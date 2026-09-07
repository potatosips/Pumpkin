use super::{Goal, GoalFuture};
use crate::entity::mob::Mob;
use pumpkin_util::random::RandomImpl;

/// Vanilla's `RandomStandGoal` for horse-family mobs.
pub struct AmbientStandGoal {
    cooldown: i32,
    interval: i32,
}

impl AmbientStandGoal {
    #[must_use]
    pub const fn new(interval: i32) -> Self {
        Self {
            cooldown: -interval,
            interval,
        }
    }

    fn evaluate_attempt(&mut self, interval_roll: i32, stand_roll: i32, can_stand: bool) -> bool {
        self.cooldown += 1;
        if self.cooldown > 0 && interval_roll < self.cooldown {
            self.cooldown = -self.interval;
            return can_stand && stand_roll == 0;
        }
        false
    }
}

impl Goal for AmbientStandGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let interval_roll = mob.get_entity_random().next_bounded_i32(1000);
            let next_cooldown = self.cooldown + 1;
            let gate_open = next_cooldown > 0 && interval_roll < next_cooldown;
            let can_stand = gate_open && mob.can_ambient_stand().await;
            // Vanilla only consumes this second random value after both earlier gates pass.
            let stand_roll = if gate_open && can_stand {
                mob.get_entity_random().next_bounded_i32(10)
            } else {
                1
            };
            self.evaluate_attempt(interval_roll, stand_roll, can_stand)
        })
    }

    fn start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            mob.start_ambient_stand();
        })
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::AmbientStandGoal;

    #[test]
    fn vanilla_random_stand_cooldown_and_roll_boundaries() {
        let mut goal = AmbientStandGoal::new(400);
        for _ in 0..400 {
            assert!(!goal.evaluate_attempt(0, 0, true));
        }
        assert!(goal.evaluate_attempt(0, 0, true));
        assert_eq!(goal.cooldown, -400);

        let mut blocked = AmbientStandGoal::new(0);
        assert!(!blocked.evaluate_attempt(0, 0, false));
        assert_eq!(blocked.cooldown, 0);
        assert!(!blocked.evaluate_attempt(0, 1, true));
        assert_eq!(blocked.cooldown, 0);
    }
}
