use std::sync::Arc;

use pumpkin_data::entity::EntityType;
use pumpkin_util::{GameMode, math::boundingbox::BoundingBox};

use super::{Controls, Goal, GoalFuture, track_target::TrackTargetGoal};
use crate::entity::ai::target_predicate::TargetPredicate;
use crate::entity::passive::iron_golem::IronGolemEntity;
use crate::entity::{EntityBase, mob::Mob, passive::villager::VillagerEntity};

const SEARCH_XZ: f64 = 10.0;
const SEARCH_Y: f64 = 8.0;
const TARGET_RANGE: f64 = 64.0;
const HOSTILE_REPUTATION: i32 = -100;

fn can_defend_against_player(player_created: bool, reputation: i32, gamemode: GameMode) -> bool {
    !player_created
        && reputation <= HOSTILE_REPUTATION
        && !matches!(gamemode, GameMode::Creative | GameMode::Spectator)
}

/// Vanilla's `DefendVillageTargetGoal` for iron golems.
pub struct DefendVillageTargetGoal {
    golem: Arc<IronGolemEntity>,
    track_target_goal: TrackTargetGoal,
    target_predicate: TargetPredicate,
    target: Option<Arc<dyn EntityBase>>,
}

impl DefendVillageTargetGoal {
    #[must_use]
    pub fn new(golem: Arc<IronGolemEntity>) -> Self {
        Self {
            golem,
            // Vanilla constructs TargetGoal with checkVisibility=false and
            // checkCanNavigate=true. Pumpkin's navigation reachability probe is
            // not implemented yet, so enabling it would reject every target.
            track_target_goal: TrackTargetGoal::new(false, false),
            target_predicate: TargetPredicate::create_attackable()
                .set_base_max_distance(TARGET_RANGE),
            target: None,
        }
    }

    fn search_box(&self) -> BoundingBox {
        self.golem
            .get_entity()
            .bounding_box
            .load()
            .expand(SEARCH_XZ, SEARCH_Y, SEARCH_XZ)
    }
}

impl Goal for DefendVillageTargetGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            self.target = None;

            // IronGolem.canAttackType(PLAYER) is false for player-created golems.
            if self.golem.is_player_created() {
                return false;
            }

            let world = mob.get_entity().world.load();
            let search_box = self.search_box();
            let villagers = world
                .get_entities_at_box(&search_box)
                .into_iter()
                .filter(|entity| entity.get_entity().entity_type == &EntityType::VILLAGER)
                .collect::<Vec<_>>();
            let players = world.get_players_at_box(&search_box);

            // Mojang's nested loops retain the last qualifying player.
            for villager in villagers {
                let Some(villager) = villager.cast_any().downcast_ref::<VillagerEntity>() else {
                    continue;
                };
                for player in &players {
                    let reputation = villager
                        .reputation_for(player.get_entity().entity_uuid)
                        .await;
                    if !can_defend_against_player(
                        self.golem.is_player_created(),
                        reputation,
                        player.gamemode.load(),
                    ) {
                        continue;
                    }
                    if self
                        .target_predicate
                        .test(
                            &world,
                            Some(&mob.get_mob_entity().living_entity),
                            &player.living_entity,
                        )
                        .await
                    {
                        self.target = Some(player.clone());
                    }
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
    fn vanilla_village_defense_reputation_and_player_gates() {
        assert!(!can_defend_against_player(false, -99, GameMode::Survival));
        assert!(can_defend_against_player(false, -100, GameMode::Survival));
        assert!(can_defend_against_player(false, -101, GameMode::Adventure));
        assert!(!can_defend_against_player(false, -100, GameMode::Creative));
        assert!(!can_defend_against_player(false, -100, GameMode::Spectator));
        assert!(!can_defend_against_player(true, -1000, GameMode::Survival));
    }
}
