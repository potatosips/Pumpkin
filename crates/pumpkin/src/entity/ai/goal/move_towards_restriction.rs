use pumpkin_util::math::{position::BlockPos, vector3::Vector3};

use super::{Controls, Goal, GoalFuture};
use crate::entity::{ai::pathfinder::NavigatorGoal, mob::Mob};

pub struct MoveTowardsRestrictionGoal {
    speed: f64,
    target: Option<Vector3<f64>>,
}

impl MoveTowardsRestrictionGoal {
    #[must_use]
    pub const fn new(speed: f64) -> Self {
        Self {
            speed,
            target: None,
        }
    }

    fn find_target(mob: &dyn Mob) -> Option<Vector3<f64>> {
        let mob_entity = mob.get_mob_entity();
        if mob_entity.is_in_position_target_range() {
            return None;
        }
        let center = mob_entity.position_target.load();
        let center = restriction_bottom_center(center);
        crate::entity::ai::random_position::get_pos_towards(
            mob,
            16,
            7,
            center,
            std::f64::consts::FRAC_PI_2,
        )
    }
}

fn restriction_bottom_center(center: BlockPos) -> Vector3<f64> {
    Vector3::new(
        center.0.x as f64 + 0.5,
        center.0.y as f64,
        center.0.z as f64 + 0.5,
    )
}

impl Goal for MoveTowardsRestrictionGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            self.target = Self::find_target(mob);
            self.target.is_some()
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            self.target.is_some()
                && !mob
                    .get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .is_idle()
        })
    }

    fn start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            if let Some(target) = self.target {
                let start = mob.get_mob_entity().living_entity.entity.pos.load();
                mob.get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .set_progress(NavigatorGoal::new(start, target, self.speed));
            }
        })
    }

    fn stop<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move { self.target = None })
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

#[cfg(test)]
mod tests {
    use super::restriction_bottom_center;
    use pumpkin_util::math::{position::BlockPos, vector3::Vector3};

    #[test]
    fn restriction_direction_uses_vanilla_bottom_center() {
        assert_eq!(
            restriction_bottom_center(BlockPos::new(-3, 64, 8)),
            Vector3::new(-2.5, 64.0, 8.5)
        );
    }
}
