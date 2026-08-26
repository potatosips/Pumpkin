use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::{Controls, Goal};
use crate::entity::EntityBase;
use crate::entity::ai::goal::GoalFuture;
use crate::entity::mob::Mob;
use crate::entity::mob::creeper::CreeperEntity;

pub struct CreeperIgniteGoal {
    goal_control: Controls,
    creeper: Arc<CreeperEntity>,
    target: Option<Arc<dyn EntityBase>>,
}

impl CreeperIgniteGoal {
    #[must_use]
    pub fn new(creeper: Arc<CreeperEntity>) -> Self {
        Self {
            goal_control: Controls::MOVE,
            creeper,
            target: None,
        }
    }
}

fn should_swell(fuse_speed: i32, target_distance_squared: Option<f64>) -> bool {
    fuse_speed > 0 || target_distance_squared.is_some_and(|distance| distance < 9.0)
}

impl Goal for CreeperIgniteGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let creeper = mob.get_mob_entity();
            let target_lock = creeper.target.lock().await;

            let target_distance_squared = target_lock.as_ref().map(|target| {
                mob.get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&target.get_entity().pos.load())
            });
            should_swell(
                self.creeper.fuse_speed.load(Ordering::Relaxed),
                target_distance_squared,
            )
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let target_lock = mob.get_mob_entity().target.lock().await;
            let target_distance_squared = target_lock.as_ref().map(|target| {
                mob.get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&target.get_entity().pos.load())
            });
            should_swell(
                self.creeper.fuse_speed.load(Ordering::Relaxed),
                target_distance_squared,
            )
        })
    }

    fn start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            {
                let mut navigator = mob
                    .get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                navigator.stop();
            }
            self.target = mob.get_mob_entity().target.lock().await.clone();
        })
    }

    fn stop<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.target = None;
        })
    }

    fn tick<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            if self.creeper.ignited.load(Ordering::Relaxed) {
                self.creeper.set_fuse_speed(1);
                return;
            }
            let Some(target) = self.target.as_ref() else {
                self.creeper.set_fuse_speed(-1);
                return;
            };

            let dist_sq = mob
                .get_entity()
                .pos
                .load()
                .squared_distance_to_vec(&target.get_entity().pos.load());

            let world = mob.get_entity().world.load();
            let has_line_of_sight = world
                .raycast(
                    mob.get_entity().get_eye_pos(),
                    target.get_entity().get_eye_pos(),
                    async |block_pos, world| world.get_block_state(block_pos).is_solid(),
                )
                .await
                .is_none();

            if dist_sq > 49.0 || !has_line_of_sight {
                self.creeper.set_fuse_speed(-1);
            } else {
                self.creeper.set_fuse_speed(1);
            }
        })
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        self.goal_control
    }
}

#[cfg(test)]
mod tests {
    use super::should_swell;

    #[test]
    fn vanilla_creeper_swell_start_and_continue_conditions() {
        assert!(should_swell(1, None));
        assert!(should_swell(-1, Some(8.999)));
        assert!(!should_swell(-1, Some(9.0)));
        assert!(!should_swell(-1, None));
    }
}
