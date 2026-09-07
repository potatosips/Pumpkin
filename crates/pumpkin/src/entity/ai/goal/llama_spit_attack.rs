use std::sync::Arc;

use pumpkin_data::{entity::EntityType, sound::Sound, sound::SoundCategory};
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::random::RandomImpl;

use crate::entity::{
    Entity, EntityBase, ai::pathfinder::NavigatorGoal, mob::Mob,
    projectile::llama_spit::LlamaSpitEntity,
};

use super::{Controls, Goal, GoalFuture};

pub struct LlamaSpitAttackGoal {
    speed: f64,
    cooldown: i32,
    see_time: i32,
}

impl LlamaSpitAttackGoal {
    #[must_use]
    pub const fn new(speed: f64) -> Self {
        Self {
            speed,
            cooldown: -1,
            see_time: 0,
        }
    }

    fn advance_cooldown(&mut self, visible: bool) -> bool {
        self.cooldown -= 1;
        if self.cooldown == 0 {
            if visible {
                self.cooldown = 40;
                return true;
            }
        } else if self.cooldown < 0 {
            // Vanilla interpolates between the configured minimum and maximum interval;
            // both are 40 ticks for llamas, so distance does not alter this value.
            self.cooldown = 40;
        }
        false
    }

    async fn shoot(mob: &dyn Mob, target: &Arc<dyn EntityBase>) {
        let Some(llama) = mob.get_llama() else {
            return;
        };
        let owner = llama.get_entity();
        let world = owner.world.load();
        let pos = owner.pos.load();
        let dimensions = owner.entity_dimension.load();
        let yaw = f64::from(owner.yaw.load()).to_radians();
        let offset = f64::from(dimensions.width + 1.0) * 0.5;
        let spawn = Vector3::new(
            pos.x - offset * yaw.sin(),
            owner.get_eye_y() - 0.1,
            pos.z + offset * yaw.cos(),
        );
        let target_entity = target.get_entity();
        let target_pos = target_entity.pos.load();
        let dx = target_pos.x - pos.x;
        let dz = target_pos.z - pos.z;
        let target_y = target_pos.y + f64::from(target_entity.entity_dimension.load().height) / 3.0;
        let dy = target_y - spawn.y;
        let arc = dx.hypot(dz) * 0.200_000_002_980_232_24;

        let spit_entity = Entity::new(world.clone(), spawn, &EntityType::LLAMA_SPIT);
        let spit = LlamaSpitEntity::new_shot(spit_entity, owner);
        spit.projectile.entity.pos.store(spawn);
        spit.projectile.set_velocity(dx, dy + arc, dz, 1.5, 10.0);

        let pitch = {
            let mut rng = mob.get_entity_random();
            1.0 + (rng.next_f32() - rng.next_f32()) * 0.2
        };
        world.play_sound_raw(
            Sound::EntityLlamaSpit as u16,
            SoundCategory::Neutral,
            &pos,
            1.0,
            pitch,
        );
        llama.set_did_spit(true);
        world.spawn_entity(Arc::new(spit)).await;
    }
}

impl Goal for LlamaSpitAttackGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            mob.get_mob_entity()
                .target
                .lock()
                .await
                .as_ref()
                .is_some_and(|target| target.get_entity().is_alive())
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let target_alive = mob
                .get_mob_entity()
                .target
                .lock()
                .await
                .as_ref()
                .is_some_and(|target| target.get_entity().is_alive());
            target_alive
                || !mob
                    .get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .is_idle()
        })
    }

    fn start<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.cooldown = -1;
            self.see_time = 0;
        })
    }

    fn stop<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.cooldown = -1;
            self.see_time = 0;
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stop();
        })
    }

    fn tick<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            let Some(target) = mob.get_mob_entity().target.lock().await.clone() else {
                return;
            };
            let pos = mob.get_entity().pos.load();
            let target_pos = target.get_entity().pos.load();
            let distance_sq = pos.squared_distance_to_vec(&target_pos);
            let world = mob.get_entity().world.load();
            let visible = world
                .raycast(
                    mob.get_entity().get_eye_pos(),
                    target.get_entity().get_eye_pos(),
                    async |block_pos, world| world.get_block_state(block_pos).is_solid(),
                )
                .await
                .is_none();
            mob.get_mob_entity()
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .look_at_entity_with_range(&target, 30.0, 30.0);

            if visible {
                self.see_time += 1;
            } else {
                self.see_time = 0;
            }
            if distance_sq <= 400.0 && self.see_time >= 5 {
                mob.get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .stop();
            } else {
                mob.get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .set_progress(NavigatorGoal::new(pos, target_pos, self.speed));
            }

            if self.advance_cooldown(visible) {
                Self::shoot(mob, &target).await;
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

#[cfg(test)]
mod tests {
    use super::LlamaSpitAttackGoal;

    #[test]
    fn vanilla_llama_spit_waits_forty_visible_ticks() {
        let mut goal = LlamaSpitAttackGoal::new(1.25);
        assert!(!goal.advance_cooldown(true));
        assert_eq!(goal.cooldown, 40);
        for _ in 0..39 {
            assert!(!goal.advance_cooldown(true));
        }
        assert!(goal.advance_cooldown(true));
        assert_eq!(goal.cooldown, 40);
    }

    #[test]
    fn invisible_target_defers_spit_and_restarts_interval() {
        let mut goal = LlamaSpitAttackGoal::new(1.25);
        goal.cooldown = 1;
        assert!(!goal.advance_cooldown(false));
        assert_eq!(goal.cooldown, 0);
        assert!(!goal.advance_cooldown(true));
        assert_eq!(goal.cooldown, 40);
    }
}
