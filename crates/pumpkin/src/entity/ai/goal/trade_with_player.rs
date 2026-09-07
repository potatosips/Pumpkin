use std::sync::Arc;

use super::{Controls, Goal, GoalFuture};
use crate::entity::{EntityBase, mob::Mob, player::Player};

pub struct TradeWithPlayerGoal {
    player: Option<Arc<Player>>,
}

impl TradeWithPlayerGoal {
    #[must_use]
    pub const fn new(_speed: f64) -> Self {
        Self { player: None }
    }

    fn trading_player_in_range(mob: &dyn Mob) -> Option<Arc<Player>> {
        let player = mob.get_trading_player()?;
        let entity = &mob.get_mob_entity().living_entity.entity;
        (entity.is_alive()
            && player.get_entity().is_alive()
            && !entity
                .touching_water
                .load(std::sync::atomic::Ordering::Relaxed)
            && entity.on_ground.load(std::sync::atomic::Ordering::Relaxed)
            && entity
                .pos
                .load()
                .squared_distance_to_vec(&player.get_entity().pos.load())
                <= 16.0)
            .then_some(player)
    }
}

impl Goal for TradeWithPlayerGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            self.player = Self::trading_player_in_range(mob);
            self.player.is_some()
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let Some(current) = Self::trading_player_in_range(mob) else {
                return false;
            };
            self.player.as_ref().is_some_and(|player| {
                player.get_entity().entity_uuid == current.get_entity().entity_uuid
            })
        })
    }

    fn start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stop();
        })
    }

    fn stop<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.player = None;
            mob.stop_trading();
        })
    }

    fn tick<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {})
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::JUMP
    }
}
