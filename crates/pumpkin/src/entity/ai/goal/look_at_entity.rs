use super::{Controls, Goal};
use crate::entity::ai::goal::GoalFuture;
use crate::entity::ai::target_predicate::TargetPredicate;
use crate::entity::mob::Mob;
use crate::entity::predicate::EntityPredicate;
use crate::entity::{EntityBase, player::Player};
use pumpkin_data::entity::EntityType;
use pumpkin_util::random::RandomImpl;
use std::sync::{Arc, Weak};

pub struct LookAtEntityGoal {
    goal_control: Controls,
    target: Option<Arc<dyn EntityBase>>,
    range: f32,
    look_time: i32,
    chance: f32,
    look_forward: bool,
    target_type: &'static EntityType,
    any_mob: bool,
    target_predicate: TargetPredicate,
}

impl LookAtEntityGoal {
    #[must_use]
    pub fn new(
        mob_weak: Weak<dyn Mob>,
        target_type: &'static EntityType,
        range: f32,
        chance: f32,
        look_forward: bool,
    ) -> Self {
        let target_predicate = Self::create_target_predicate(mob_weak, target_type, range);
        Self {
            goal_control: Controls::LOOK,
            target: None,
            range,
            look_time: 0,
            chance,
            look_forward,
            target_type,
            any_mob: false,
            target_predicate,
        }
    }

    #[must_use]
    pub fn with_default(
        mob_weak: Weak<dyn Mob>,
        target_type: &'static EntityType,
        range: f32,
    ) -> Box<Self> {
        Box::new(Self::new(mob_weak, target_type, range, 0.02, false))
    }

    /// Java's class-based `LookAtPlayerGoal` can target the broad `Mob` class.
    #[must_use]
    pub fn for_any_mob(mob_weak: Weak<dyn Mob>, range: f32) -> Box<Self> {
        let mut goal = Self::new(mob_weak, &EntityType::PLAYER, range, 0.02, false);
        goal.any_mob = true;
        Box::new(goal)
    }

    fn create_target_predicate(
        mob_weak: Weak<dyn Mob>,
        target_type: &'static EntityType,
        range: f32,
    ) -> TargetPredicate {
        let mut target_predicate = TargetPredicate::create_non_attackable();
        target_predicate.base_max_distance = range as f64; // TODO
        if target_type == &EntityType::PLAYER {
            target_predicate.set_predicate(move |entity_id, world| {
                let mob_weak = mob_weak.clone();
                async move {
                    if let (Some(mob_arc), Some(target)) =
                        (mob_weak.upgrade(), world.get_entity_by_id(entity_id))
                    {
                        let predicate = EntityPredicate::Rides(mob_arc.get_entity());
                        predicate.test(target.get_entity()).await
                    } else {
                        // MobEntity is destroyed
                        false
                    }
                }
            });
        }
        target_predicate
    }
}

impl Goal for LookAtEntityGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async {
            let mob_entity = mob.get_mob_entity();
            if mob_entity.target.lock().await.is_some() {
                return false;
            }

            if mob.get_entity_random().next_f32() >= self.chance {
                return false;
            }

            let world = mob_entity.living_entity.entity.world.load();
            let mob_pos = mob_entity.living_entity.entity.pos.load();

            let mut candidates: Vec<Arc<dyn EntityBase>> = if self.any_mob {
                let self_id = mob_entity.living_entity.entity.entity_id;
                let mut candidates = Vec::new();
                world.extend_entities_in_box_where(
                    &mut candidates,
                    usize::MAX,
                    mob_entity.living_entity.entity.bounding_box.load().expand(
                        self.range.into(),
                        3.0,
                        self.range.into(),
                    ),
                    |entity| {
                        entity.get_entity().entity_id != self_id
                            && entity.get_entity().is_alive()
                            && entity.get_mob().is_some()
                    },
                );
                candidates
            } else if *self.target_type == EntityType::PLAYER {
                world
                    .get_nearby_players(mob_pos, self.range.into())
                    .into_iter()
                    .map(|player: Arc<Player>| player as Arc<dyn EntityBase>)
                    .collect()
            } else {
                let mut candidates = Vec::new();
                world.extend_entities_in_box_where(
                    &mut candidates,
                    usize::MAX,
                    mob_entity.living_entity.entity.bounding_box.load().expand(
                        self.range.into(),
                        3.0,
                        self.range.into(),
                    ),
                    |entity| entity.get_entity().entity_type == self.target_type,
                );
                candidates
            };
            candidates.sort_by(|a, b| {
                a.get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&mob_pos)
                    .total_cmp(&b.get_entity().pos.load().squared_distance_to_vec(&mob_pos))
            });
            self.target = None;
            for candidate in candidates {
                let Some(living) = candidate.get_living_entity() else {
                    continue;
                };
                if self
                    .target_predicate
                    .test(&world, Some(&mob_entity.living_entity), living)
                    .await
                {
                    self.target = Some(candidate);
                    break;
                }
            }

            self.target.is_some()
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async {
            let mob_entity = mob.get_mob_entity();
            if mob_entity.target.lock().await.is_some() {
                return false;
            }
            if let Some(target) = &self.target {
                if !target.get_entity().is_alive() {
                    return false;
                }
                let mob_pos = mob_entity.living_entity.entity.pos.load();
                let target_pos = target.get_entity().pos.load();
                if mob_pos.squared_distance_to_vec(&target_pos) as f32 > (self.range * self.range) {
                    return false;
                }
                return self.look_time > 0;
            }
            false
        })
    }

    fn start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async {
            self.look_time = self.get_tick_count(40 + mob.get_entity_random().next_bounded_i32(40));
        })
    }

    fn stop<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async {
            self.target = None;
        })
    }

    fn tick<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async {
            let mob_entity = mob.get_mob_entity();
            if let Some(target) = &self.target
                && target.get_entity().is_alive()
            {
                let target_entity = target.get_entity();
                let target_pos = target_entity.pos.load();
                let look_y = if self.look_forward {
                    mob_entity.living_entity.entity.get_eye_y()
                } else {
                    target_entity.get_eye_y()
                };
                mob_entity
                    .look_control
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .look_at(mob, target_pos.x, look_y, target_pos.z);
                self.look_time -= 1;
            }
        })
    }

    fn controls(&self) -> Controls {
        self.goal_control
    }
}

#[cfg(test)]
mod tests {
    use super::LookAtEntityGoal;

    #[test]
    fn any_mob_constructor_uses_the_class_wide_mode() {
        let weak = std::sync::Weak::<crate::entity::passive::cow::CowEntity>::new();
        let weak: std::sync::Weak<dyn crate::entity::mob::Mob> = weak;
        let goal = LookAtEntityGoal::for_any_mob(weak, 8.0);
        assert!(goal.any_mob);
        assert_eq!(goal.range, 8.0);
    }
}
