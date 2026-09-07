use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::entity::EntityType;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{EntityBase, ai::pathfinder::NavigatorGoal, mob::Mob};

use super::{Controls, Goal, GoalFuture, to_goal_ticks};

const SEARCH_RADIUS: f64 = 9.0;
const MIN_JOIN_DISTANCE_SQ: f64 = 4.0;
const MAX_DISTANCE_SQ: f64 = 676.0;
const MAX_CHAIN_DEPTH: usize = 8;

/// Vanilla's `LlamaFollowCaravanGoal`.
pub struct LlamaFollowCaravanGoal {
    speed: AtomicCell<f64>,
    distance_check_cooldown: AtomicI32,
}

impl LlamaFollowCaravanGoal {
    #[must_use]
    pub fn new(speed: f64) -> Self {
        Self {
            speed: AtomicCell::new(speed),
            distance_check_cooldown: AtomicI32::new(0),
        }
    }

    fn llama(entity: &Arc<dyn EntityBase>) -> Option<&crate::entity::passive::llama::LlamaEntity> {
        entity.get_mob().and_then(Mob::get_llama)
    }

    async fn chain_reaches_leashed_llama(
        start: &crate::entity::passive::llama::LlamaEntity,
        initial_depth: usize,
    ) -> bool {
        let world = start.get_entity().world.load();
        let mut current_id = start.caravan_head_id();
        // Java rejects only when the recursive depth is greater than eight.
        // The current node's head is still checked at depth eight.
        for _ in initial_depth..=MAX_CHAIN_DEPTH {
            let Some(entity) = world.get_entity_by_id(current_id) else {
                return false;
            };
            let Some(llama) = Self::llama(&entity) else {
                return false;
            };
            if entity.get_entity().is_leashed().await {
                return true;
            }
            if !llama.in_caravan() {
                return false;
            }
            current_id = llama.caravan_head_id();
        }
        false
    }

    async fn nearest_head(mob: &dyn Mob) -> Option<Arc<dyn EntityBase>> {
        let llama = mob.get_llama()?;
        let entity = llama.get_entity();
        if entity.is_leashed().await || llama.in_caravan() {
            return None;
        }

        let origin = entity.pos.load();
        let search_box = entity
            .bounding_box
            .load()
            .expand(SEARCH_RADIUS, 4.0, SEARCH_RADIUS);
        let nearby = entity.world.load().get_entities_at_box(&search_box);
        let mut caravan_head: Option<(f64, Arc<dyn EntityBase>)> = None;
        let mut leashed_head: Option<(f64, Arc<dyn EntityBase>)> = None;

        for candidate in &nearby {
            let candidate_type = candidate.get_entity().entity_type;
            if candidate_type != &EntityType::LLAMA && candidate_type != &EntityType::TRADER_LLAMA {
                continue;
            }
            let Some(other) = Self::llama(candidate) else {
                continue;
            };
            if other.has_caravan_tail() {
                continue;
            }
            let candidate_pos = candidate.get_entity().pos.load();
            let distance = origin.squared_distance_to_vec(&candidate_pos);
            if other.in_caravan()
                && caravan_head
                    .as_ref()
                    .is_none_or(|(best, _)| distance <= *best)
            {
                caravan_head = Some((distance, candidate.clone()));
            } else if candidate.get_entity().is_leashed().await
                && leashed_head
                    .as_ref()
                    .is_none_or(|(best, _)| distance <= *best)
            {
                leashed_head = Some((distance, candidate.clone()));
            }
        }

        let (distance, head) = caravan_head.or(leashed_head)?;
        if distance < MIN_JOIN_DISTANCE_SQ {
            return None;
        }
        let head_llama = Self::llama(&head)?;
        if !head.get_entity().is_leashed().await
            && !Self::chain_reaches_leashed_llama(head_llama, 1).await
        {
            return None;
        }
        Some(head)
    }

    fn follow_target(pos: Vector3<f64>, head_pos: Vector3<f64>) -> Option<Vector3<f64>> {
        let distance_sq = pos.squared_distance_to_vec(&head_pos);
        let distance = distance_sq.sqrt();
        if distance <= f64::EPSILON {
            return None;
        }
        let scale = (distance - 2.0).max(0.0) / distance;
        Some(pos + (head_pos - pos) * scale)
    }
}

impl Goal for LlamaFollowCaravanGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let Some(head) = Self::nearest_head(mob).await else {
                return false;
            };
            let Some(llama) = mob.get_llama() else {
                return false;
            };
            llama.join_caravan(head.get_entity().entity_id);
            true
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let Some(llama) = mob.get_llama() else {
                return false;
            };
            if !llama.in_caravan() || !Self::chain_reaches_leashed_llama(llama, 0).await {
                return false;
            }
            let world = llama.get_entity().world.load();
            let Some(head) = world.get_entity_by_id(llama.caravan_head_id()) else {
                return false;
            };
            if !head.get_entity().is_alive() {
                return false;
            }

            let distance_sq = llama
                .get_entity()
                .pos
                .load()
                .squared_distance_to_vec(&head.get_entity().pos.load());
            if distance_sq > MAX_DISTANCE_SQ {
                let speed = self.speed.load();
                if speed <= 3.0 {
                    self.speed.store(speed * 1.2);
                    self.distance_check_cooldown
                        .store(to_goal_ticks(40), Ordering::Relaxed);
                    return true;
                }
                if self.distance_check_cooldown.load(Ordering::Relaxed) == 0 {
                    return false;
                }
            }
            if self.distance_check_cooldown.load(Ordering::Relaxed) > 0 {
                self.distance_check_cooldown.fetch_sub(1, Ordering::Relaxed);
            }
            true
        })
    }

    fn tick<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            let Some(llama) = mob.get_llama() else {
                return;
            };
            let entity = llama.get_entity();
            if entity
                .leashed_to
                .lock()
                .await
                .as_ref()
                .is_some_and(|holder| holder.get_entity().entity_type == &EntityType::LEASH_KNOT)
            {
                return;
            }
            let Some(head) = entity
                .world
                .load()
                .get_entity_by_id(llama.caravan_head_id())
            else {
                return;
            };
            let pos = entity.pos.load();
            let head_pos = head.get_entity().pos.load();
            let Some(target) = Self::follow_target(pos, head_pos) else {
                return;
            };
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_progress(NavigatorGoal::new(pos, target, self.speed.load()));
        })
    }

    fn stop<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            if let Some(llama) = mob.get_llama() {
                llama.leave_caravan();
            }
            self.speed.store(2.1);
        })
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

#[cfg(test)]
mod tests {
    use pumpkin_util::math::vector3::Vector3;

    use super::LlamaFollowCaravanGoal;

    #[test]
    fn caravan_target_preserves_vanilla_two_block_spacing() {
        let target = LlamaFollowCaravanGoal::follow_target(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
        )
        .unwrap();
        assert!((target.x - 8.0).abs() < f64::EPSILON);
        assert_eq!(target.y, 0.0);
        assert_eq!(target.z, 0.0);

        let close = LlamaFollowCaravanGoal::follow_target(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
        )
        .unwrap();
        assert_eq!(close, Vector3::new(0.0, 0.0, 0.0));
    }
}
