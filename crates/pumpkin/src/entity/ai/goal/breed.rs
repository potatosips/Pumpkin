use std::sync::Arc;

use crate::entity::{
    EntityBase, ai::pathfinder::NavigatorGoal, experience_orb::ExperienceOrbEntity, mob::Mob,
};
use pumpkin_data::entity::EntityStatus;
use pumpkin_protocol::bedrock::server::actor_event::ActorEventType;

use super::{Controls, Goal, GoalFuture};

pub struct BreedGoal {
    speed: f64,
    mate: Option<Arc<dyn EntityBase>>,
    timer: i32,
}

impl BreedGoal {
    #[must_use]
    pub fn new(speed: f64) -> Box<Self> {
        Box::new(Self {
            speed,
            mate: None,
            timer: 0,
        })
    }

    fn find_mate(mob: &dyn Mob) -> Option<Arc<dyn EntityBase>> {
        let mob_entity = mob.get_mob_entity();
        if !mob_entity.is_in_love() || mob.is_sitting() || !mob.can_breed_now() {
            return None;
        }

        let entity = mob.get_entity();
        let pos = entity.pos.load();
        let world = entity.world.load();
        let my_uuid = entity.entity_uuid;

        let nearby = world.get_nearby_entities(pos, 8.0);
        let mut closest: Option<(f64, Arc<dyn EntityBase>)> = None;

        for candidate in nearby.values() {
            let c_entity = candidate.get_entity();
            if c_entity.entity_uuid == my_uuid {
                continue;
            }
            if !mob.can_mate_with(candidate.as_ref()) {
                continue;
            }
            if !candidate.is_in_love()
                || !candidate.is_breeding_ready()
                || candidate.is_panicking()
                || candidate
                    .get_mob()
                    .is_some_and(|mob| mob.is_sitting() || !mob.can_breed_now())
            {
                continue;
            }

            let dist = pos.squared_distance_to_vec(&c_entity.pos.load());
            match &closest {
                Some((best_dist, _)) if dist >= *best_dist => {}
                _ => closest = Some((dist, candidate.clone())),
            }
        }

        closest.map(|(_, e)| e)
    }

    async fn breed(mob: &dyn Mob, mate: &dyn EntityBase) {
        let mob_entity = mob.get_mob_entity();
        let entity = mob.get_entity();
        let world = entity.world.load();

        if let Some(player) = mob_entity
            .breeder
            .load()
            .and_then(|uuid| world.get_player_by_uuid(uuid))
        {
            player
                .increment_stat(
                    pumpkin_data::statistic::StatisticCategory::Custom,
                    pumpkin_data::statistic::CustomStatistic::AnimalsBred as i32,
                    1,
                )
                .await;

            player
                .trigger_advancement(
                    crate::entity::player::advancement::trigger::AdvancementTrigger::BredAnimal {
                        parent_type: format!("minecraft:{}", entity.entity_type.resource_name),
                    },
                )
                .await;
        }

        let parent_pos = entity.pos.load();
        mob.spawn_breeding_offspring(mate).await;

        mob_entity.reset_love_ticks();
        entity.set_age(6000);
        mob_entity
            .breeding_cooldown
            .store(6000, std::sync::atomic::Ordering::Relaxed);
        mate.reset_love();
        mate.get_entity().set_age(6000);
        mate.set_breeding_cooldown(6000);

        world.send_entity_status(
            entity,
            EntityStatus::InLoveHearts,
            Some(ActorEventType::InLoveHearts),
        );
        if world.level_info.load().game_rules.mob_drops {
            ExperienceOrbEntity::spawn(&world, parent_pos, rand::random_range(1..=7)).await;
        }
    }
}

impl Goal for BreedGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async {
            let mob_entity = mob.get_mob_entity();
            if !mob_entity.is_breeding_ready()
                || !mob_entity.is_in_love()
                || mob.is_sitting()
                || !mob.can_breed_now()
            {
                return false;
            }

            self.mate = Self::find_mate(mob);
            self.mate.is_some()
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async {
            let Some(mate) = &self.mate else {
                return false;
            };

            if mob.is_sitting()
                || !mob.can_breed_now()
                || !mate.get_entity().is_alive()
                || mate.is_panicking()
                || mate
                    .get_mob()
                    .is_some_and(|mob| mob.is_sitting() || !mob.can_breed_now())
            {
                return false;
            }

            mate.is_in_love() && self.timer < 60
        })
    }

    fn start<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async {
            self.timer = 0;
        })
    }

    fn stop<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async {
            self.mate = None;
            self.timer = 0;
            let mut navigator = mob
                .get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            navigator.stop();
        })
    }

    fn tick<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async {
            let Some(mate) = &self.mate else {
                return;
            };

            let mob_entity = mob.get_mob_entity();
            let mate_pos = mate.get_entity().pos.load();

            {
                let mut look_control = mob_entity
                    .look_control
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                look_control.look_at_entity(mob, mate);
            };

            let my_pos = mob.get_entity().pos.load();
            let dist_sq = my_pos.squared_distance_to_vec(&mate_pos);

            {
                let mut navigator = mob_entity
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                navigator.set_progress(NavigatorGoal::new(my_pos, mate_pos, self.speed));
            };

            self.timer += 1;

            if self.timer >= 60 && dist_sq < 9.0 {
                Self::breed(mob, mate.as_ref()).await;
            }
        })
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}
