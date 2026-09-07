use crate::entity::mob::Mob;

use super::{Controls, Goal, GoalFuture, revenge::RevengeGoal};

/// Llama's Vanilla hurt-by-target goal stops after the llama successfully spits.
pub struct LlamaRevengeGoal {
    inner: RevengeGoal,
}

impl LlamaRevengeGoal {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: RevengeGoal::new(true),
        }
    }
}

impl Goal for LlamaRevengeGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        self.inner.can_start(mob)
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            if mob.get_llama().is_some_and(|llama| llama.did_spit()) {
                return false;
            }
            self.inner.should_continue(mob).await
        })
    }

    fn start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        self.inner.start(mob)
    }

    fn stop<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.inner.stop(mob).await;
            if let Some(llama) = mob.get_llama() {
                llama.set_did_spit(false);
            }
        })
    }

    fn controls(&self) -> Controls {
        self.inner.controls()
    }
}
