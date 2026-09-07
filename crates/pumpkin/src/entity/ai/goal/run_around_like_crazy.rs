use pumpkin_util::math::vector3::Vector3;

use crate::entity::{ai::pathfinder::NavigatorGoal, mob::Mob};

use super::{Controls, Goal, GoalFuture};

/// Vanilla's untamed-horse riding goal. Taming rolls and rider ejection are
/// handled by the shared horse-family tick; this goal owns the required random
/// 1.2-speed navigation while an untamed horse has a passenger.
pub struct RunAroundLikeCrazyGoal {
    target: Option<Vector3<f64>>,
}

impl RunAroundLikeCrazyGoal {
    pub const fn new() -> Self {
        Self { target: None }
    }

    fn random_target(mob: &dyn Mob) -> Option<Vector3<f64>> {
        crate::entity::ai::random_position::get_pos(mob, 5, 4)
    }
}

impl Goal for RunAroundLikeCrazyGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            if mob.is_tame()
                || !mob
                    .get_mob_entity()
                    .living_entity
                    .entity
                    .has_passengers()
                    .await
            {
                return false;
            }
            self.target = Self::random_target(mob);
            self.target.is_some()
        })
    }

    fn start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            if let Some(target) = self.target {
                let entity = &mob.get_mob_entity().living_entity.entity;
                mob.get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .set_progress(NavigatorGoal::new(entity.pos.load(), target, 1.2));
            }
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            if mob.is_tame()
                || !mob
                    .get_mob_entity()
                    .living_entity
                    .entity
                    .has_passengers()
                    .await
            {
                return false;
            }
            !mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_idle()
        })
    }

    fn stop<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.target = None;
        })
    }

    fn tick<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            mob.tick_untamed_riding().await;
        })
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}
