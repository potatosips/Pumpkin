use std::sync::{Arc, atomic::Ordering::Relaxed};

use pumpkin_data::entity::EntityType;

use crate::entity::{EntityBase, ai::target_predicate::TargetPredicate, mob::Mob};

use super::{Controls, Goal, GoalFuture, track_target::TrackTargetGoal};

/// Trader llama's Vanilla owner-hurt target goal. The leash holder acts as its owner.
pub struct TraderLlamaDefendGoal {
    target: Option<Arc<dyn EntityBase>>,
    trader_id: i32,
    last_attacked_time: i32,
    track_target_goal: TrackTargetGoal,
    target_predicate: TargetPredicate,
}

impl TraderLlamaDefendGoal {
    #[must_use]
    pub fn new() -> Self {
        Self {
            target: None,
            trader_id: 0,
            last_attacked_time: 0,
            // Vanilla constructs this TargetGoal with mustSee=false; its
            // acquisition TargetingConditions still performs visibility.
            track_target_goal: TrackTargetGoal::with_default(false),
            target_predicate: TargetPredicate::create_attackable(),
        }
    }
}

impl Goal for TraderLlamaDefendGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let world = mob.get_entity().world.load();
            let Some(trader) = mob.get_entity().leashed_to.lock().await.clone() else {
                return false;
            };
            if trader.get_entity().entity_type != &EntityType::WANDERING_TRADER {
                return false;
            }
            let Some(trader_living) = trader.get_living_entity() else {
                return false;
            };
            let attacked_time = trader_living.last_attacked_time.load(Relaxed);
            if attacked_time == self.last_attacked_time {
                return false;
            }
            let attacker_id = trader_living.last_attacker_id.load(Relaxed);
            let Some(attacker) = world.get_entity_by_id(attacker_id) else {
                return false;
            };
            let Some(attacker_living) = attacker.get_living_entity() else {
                return false;
            };
            if !self
                .track_target_goal
                .can_track(mob, Some(attacker_living), &self.target_predicate)
                .await
            {
                return false;
            }
            self.trader_id = trader.get_entity().entity_id;
            self.target = Some(attacker);
            true
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        self.track_target_goal.should_continue(mob)
    }

    fn start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            mob.set_mob_target(self.target.clone()).await;
            if let Some(trader) = mob
                .get_entity()
                .world
                .load()
                .get_entity_by_id(self.trader_id)
                && let Some(living) = trader.get_living_entity()
            {
                self.last_attacked_time = living.last_attacked_time.load(Relaxed);
            }
            self.track_target_goal.start(mob).await;
        })
    }

    fn stop<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.target = None;
            self.track_target_goal.stop(mob).await;
        })
    }

    fn controls(&self) -> Controls {
        Controls::TARGET
    }
}
