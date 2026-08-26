use std::sync::Arc;

use pumpkin_data::entity::{EntityType, MobCategory};
use rand::RngExt;

use super::{Controls, Goal, GoalFuture, to_goal_ticks, track_target::TrackTargetGoal};
use crate::entity::{EntityBase, ai::target_predicate::TargetPredicate, mob::Mob};

const RECIPROCAL_CHANCE: i32 = 5;

fn is_vanilla_iron_golem_hostile_target(entity_type: &EntityType) -> bool {
    entity_type.category == &MobCategory::MONSTER && entity_type != &EntityType::CREEPER
}

/// Selects the nearest `Enemy` mob except creepers, matching Vanilla's generic
/// hostile-mob target goal for iron golems.
pub struct IronGolemHostileTargetGoal {
    track_target_goal: TrackTargetGoal,
    target_predicate: TargetPredicate,
    target: Option<Arc<dyn EntityBase>>,
    reciprocal_chance: i32,
}

impl IronGolemHostileTargetGoal {
    #[must_use]
    pub fn new() -> Self {
        Self {
            track_target_goal: TrackTargetGoal::new(false, false),
            target_predicate: TargetPredicate::create_attackable(),
            target: None,
            reciprocal_chance: to_goal_ticks(RECIPROCAL_CHANCE),
        }
    }
}

impl Goal for IronGolemHostileTargetGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            if self.reciprocal_chance > 0
                && mob.get_random().random_range(0..self.reciprocal_chance) != 0
            {
                return false;
            }

            let living = &mob.get_mob_entity().living_entity;
            let follow_range =
                living.get_attribute_value(&pumpkin_data::attributes::Attributes::FOLLOW_RANGE);
            self.target_predicate.base_max_distance = follow_range;
            let world = living.entity.world.load();
            let origin = living.entity.pos.load();

            let mut candidates = world
                .get_nearby_entities(origin, follow_range)
                .into_values()
                .filter(|candidate| {
                    is_vanilla_iron_golem_hostile_target(candidate.get_entity().entity_type)
                })
                .collect::<Vec<_>>();
            candidates.sort_by(|a, b| {
                a.get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&origin)
                    .total_cmp(&b.get_entity().pos.load().squared_distance_to_vec(&origin))
            });

            self.target = None;
            for candidate in candidates {
                let Some(candidate_living) = candidate.get_living_entity() else {
                    continue;
                };
                if self
                    .target_predicate
                    .test(&world, Some(living), candidate_living)
                    .await
                {
                    self.target = Some(candidate);
                    break;
                }
            }
            self.target.is_some()
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move { self.track_target_goal.should_continue(mob).await })
    }

    fn start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            mob.set_mob_target(self.target.clone()).await;
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
        self.track_target_goal.controls()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_iron_golem_targets_enemy_mobs_but_not_creepers() {
        for hostile in [
            &EntityType::ZOMBIE,
            &EntityType::SKELETON,
            &EntityType::SPIDER,
            &EntityType::PILLAGER,
            &EntityType::WITCH,
        ] {
            assert!(is_vanilla_iron_golem_hostile_target(hostile));
        }
        assert!(!is_vanilla_iron_golem_hostile_target(&EntityType::CREEPER));
        assert!(!is_vanilla_iron_golem_hostile_target(&EntityType::COW));
        assert!(!is_vanilla_iron_golem_hostile_target(&EntityType::VILLAGER));
    }
}
