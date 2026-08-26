use super::{Controls, Goal, GoalFuture, to_goal_ticks};
use crate::entity::{ai::pathfinder::NavigatorGoal, mob::Mob};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

pub struct WanderAroundGoal {
    goal_control: Controls,
    speed: f64,
    target: Option<Vector3<f64>>,
    chance: i32,
}

impl WanderAroundGoal {
    #[must_use]
    pub const fn new(speed: f64) -> Self {
        Self {
            goal_control: Controls::MOVE,
            speed,
            target: None,
            chance: to_goal_ticks(40),
        }
    }

    fn find_wander_target(mob: &dyn Mob) -> Vector3<f64> {
        let entity = &mob.get_mob_entity().living_entity.entity;
        let world = entity.world.load();
        let pos = entity.pos.load();
        let mut rng = mob.get_random();

        let horizontal_range = 8.0;

        let dx = rng.random_range(-horizontal_range..=horizontal_range);
        let dz = rng.random_range(-horizontal_range..=horizontal_range);
        let target_x = pos.x + dx;
        let target_z = pos.z + dz;

        let start_y = pos.y.floor() as i32;
        let mut best_y = pos.y;
        for dy in -3..=3 {
            let check_pos = BlockPos::new(
                target_x.floor() as i32,
                start_y + dy,
                target_z.floor() as i32,
            );
            let (_, state) = world.get_block_and_state(&check_pos);
            if !state.is_air() && !state.is_liquid() {
                let above_pos = BlockPos::new(
                    target_x.floor() as i32,
                    start_y + dy + 1,
                    target_z.floor() as i32,
                );
                let (_, state_above) = world.get_block_and_state(&above_pos);
                if state_above.is_air() {
                    best_y = f64::from(start_y + dy + 1);
                    break;
                }
            }
        }

        Vector3::new(target_x, best_y, target_z)
    }
}

impl Goal for WanderAroundGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            if mob.get_random().random_range(0..self.chance) != 0 {
                return false;
            }

            self.target = Some(Self::find_wander_target(mob));
            true
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let navigator = mob
                .get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            !navigator.is_idle()
        })
    }

    fn start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            if let Some(target) = self.target {
                let pos = mob.get_mob_entity().living_entity.entity.pos.load();
                let mut navigator = mob
                    .get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                navigator.set_progress(NavigatorGoal::new(pos, target, self.speed));
            }
        })
    }

    fn stop<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.target = None;
        })
    }

    fn controls(&self) -> Controls {
        self.goal_control
    }
}
