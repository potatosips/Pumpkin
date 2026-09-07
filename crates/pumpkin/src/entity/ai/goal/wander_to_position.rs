use pumpkin_util::math::{position::BlockPos, vector3::Vector3};

use crate::entity::{ai::pathfinder::NavigatorGoal, mob::Mob};

use super::{Controls, Goal, GoalFuture};

/// Wandering trader's goal for returning to its spawner-assigned meeting target.
pub struct WanderToPositionGoal {
    controls: Controls,
    stop_distance: f64,
    speed: f64,
}

impl WanderToPositionGoal {
    pub const fn new(stop_distance: f64, speed: f64) -> Self {
        Self {
            controls: Controls::MOVE,
            stop_distance,
            speed,
        }
    }

    fn is_farther_than(entity_pos: Vector3<f64>, target: BlockPos, distance: f64) -> bool {
        entity_pos.squared_distance_to_vec(&target.to_centered_f64()) > distance * distance
    }
}

impl Goal for WanderToPositionGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let Some(trader) = mob.get_wandering_trader() else {
                return false;
            };
            trader.wander_target().is_some_and(|target| {
                Self::is_farther_than(mob.get_entity().pos.load(), target, self.stop_distance)
            })
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let Some(trader) = mob.get_wandering_trader() else {
                return false;
            };
            trader.wander_target().is_some_and(|target| {
                Self::is_farther_than(mob.get_entity().pos.load(), target, self.stop_distance)
            })
        })
    }

    fn stop<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            if let Some(trader) = mob.get_wandering_trader() {
                trader.set_wander_target(None);
            }
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stop();
        })
    }

    fn tick<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            let Some(target) = mob
                .get_wandering_trader()
                .and_then(|trader| trader.wander_target())
            else {
                return;
            };
            let pos = mob.get_entity().pos.load();
            // Vanilla steers toward the BlockPos low corner here (its distance
            // predicate above deliberately uses the block center).
            let destination = Vector3::new(
                f64::from(target.0.x),
                f64::from(target.0.y),
                f64::from(target.0.z),
            );
            let mut navigator = mob
                .get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !navigator.is_idle() {
                return;
            }
            let destination = if Self::is_farther_than(pos, target, 10.0) {
                pos + (destination - pos).normalize() * 10.0
            } else {
                destination
            };
            navigator.set_progress(NavigatorGoal::new(pos, destination, self.speed));
        })
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        self.controls
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_stop_distance_is_strictly_greater_than_two() {
        let target = BlockPos::new(2, 0, 0);
        let center = target.to_centered_f64();
        assert!(!WanderToPositionGoal::is_farther_than(
            center + Vector3::new(2.0, 0.0, 0.0),
            target,
            2.0
        ));
        assert!(WanderToPositionGoal::is_farther_than(
            center + Vector3::new(2.01, 0.0, 0.0),
            target,
            2.0
        ));
    }
}
