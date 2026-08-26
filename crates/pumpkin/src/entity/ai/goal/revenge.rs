use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

use super::{Controls, Goal};
use crate::entity::EntityBase;
use crate::entity::ai::goal::GoalFuture;
use crate::entity::ai::goal::track_target::TrackTargetGoal;
use crate::entity::ai::target_predicate::TargetPredicate;
use crate::entity::mob::Mob;

pub struct RevengeGoal {
    track_target_goal: TrackTargetGoal,
    target: Option<Arc<dyn EntityBase>>,
    last_attacked_time: i32,
    target_predicate: TargetPredicate,
    pub alert_same_type: bool,
}

impl RevengeGoal {
    #[must_use]
    pub fn new(check_visibility: bool) -> Self {
        let target_predicate = TargetPredicate::create_attackable()
            .ignore_visibility()
            .ignore_distance_scaling_factor();
        Self {
            track_target_goal: TrackTargetGoal::with_default(check_visibility),
            target: None,
            last_attacked_time: 0,
            target_predicate,
            alert_same_type: false,
        }
    }

    #[must_use]
    pub fn set_alert_others(mut self) -> Self {
        self.alert_same_type = true;
        self
    }
}

impl Goal for RevengeGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let mob_entity = mob.get_mob_entity();
            let living = &mob_entity.living_entity;

            let attacked_time = living.last_attacked_time.load(Relaxed);
            if attacked_time == self.last_attacked_time {
                return false;
            }

            let attacker_id = living.last_attacker_id.load(Relaxed);
            if attacker_id == 0 {
                return false;
            }

            let world = living.entity.world.load();
            let Some(attacker) = world.get_entity_by_id(attacker_id) else {
                return false;
            };

            let Some(attacker_living) = attacker.get_living_entity() else {
                return false;
            };

            if !self
                .target_predicate
                .test(&world, Some(&mob_entity.living_entity), attacker_living)
                .await
            {
                return false;
            }

            self.target = Some(attacker);
            true
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async { self.track_target_goal.should_continue(mob).await })
    }

    fn start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async {
            mob.set_mob_target(self.target.clone()).await;

            let mob_entity = mob.get_mob_entity();
            self.last_attacked_time = mob_entity.living_entity.last_attacked_time.load(Relaxed);
            self.track_target_goal.max_time_without_visibility = 300;

            self.track_target_goal.start(mob).await;

            if self.alert_same_type
                && let Some(ref target_ent) = self.target
            {
                let this_ent = &mob_entity.living_entity.entity;
                let world = this_ent.world.load();
                let my_type = this_ent.entity_type;
                let my_pos = this_ent.pos.load();
                let nearby = world.get_nearby_entities(my_pos, 32.0);
                for other in nearby.values() {
                    let other_ent = other.get_entity();
                    if other_ent.entity_id != this_ent.entity_id && other_ent.entity_type == my_type
                    {
                        if let Some(other_mob) = other.cast_any().downcast_ref::<crate::entity::mob::zombified_piglin::ZombifiedPiglinEntity>() {
                            let has_target = other_mob.mob_entity.target.lock().await.is_some();
                            if !has_target {
                                other_mob.mob_entity.set_target(Some(target_ent.clone())).await;
                            }
                        }
                    }
                }
            }
        })
    }

    fn stop<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async {
            self.target = None;
            self.track_target_goal.stop(mob).await;
        })
    }

    fn controls(&self) -> Controls {
        self.track_target_goal.controls()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_revenge_goal_alert_flag_and_memory() {
        let goal_default = RevengeGoal::new(true);
        assert!(!goal_default.alert_same_type);

        let goal_pack = RevengeGoal::new(true).set_alert_others();
        assert!(goal_pack.alert_same_type);
    }
}
