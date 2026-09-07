use std::sync::Arc;

use pumpkin_util::random::RandomImpl;

use super::{Controls, Goal, GoalFuture, to_goal_ticks};
use crate::entity::{EntityBase, mob::Mob, player::Player};

pub struct LookAtTradingPlayerGoal {
    player: Option<Arc<Player>>,
    look_time: i32,
}

impl LookAtTradingPlayerGoal {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            player: None,
            look_time: 0,
        }
    }

    fn look_at_player(&self, mob: &dyn Mob) {
        let Some(player) = &self.player else { return };
        let entity = player.get_entity();
        let position = entity.pos.load();
        mob.get_mob_entity()
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at(mob, position.x, entity.get_eye_y(), position.z);
    }
}

impl Goal for LookAtTradingPlayerGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            self.player = mob.get_trading_player();
            self.player.is_some()
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let Some(player) = &self.player else {
                return false;
            };
            player.get_entity().is_alive()
                && mob
                    .get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&player.get_entity().pos.load())
                    <= 64.0
                && self.look_time > 0
        })
    }

    fn start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.look_time = to_goal_ticks(40 + mob.get_entity_random().next_bounded_i32(40));
        })
    }

    fn stop<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.player = None;
            self.look_time = 0;
        })
    }

    fn tick<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.look_at_player(mob);
            self.look_time -= 1;
        })
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::LOOK
    }
}
