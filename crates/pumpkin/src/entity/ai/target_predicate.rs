use pumpkin_data::{data_component_impl::EquipmentSlot, entity::EntityType, item::Item};
use pumpkin_util::Difficulty;

use crate::entity::living::LivingEntity;
use crate::world::World;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

const MIN_DISTANCE: f64 = 2.0;

const fn attackable_target_allowed(
    can_take_damage: bool,
    is_player: bool,
    peaceful: bool,
    has_tester: bool,
) -> bool {
    can_take_damage && !(peaceful && (!has_tester || is_player))
}

fn visibility_scale(
    sneaking: bool,
    invisible: bool,
    armor_coverage: f64,
    matching_disguise: bool,
) -> f64 {
    let mut scale = if sneaking { 0.8 } else { 1.0 };
    if invisible {
        scale *= 0.7 * armor_coverage.max(0.1);
    }
    if matching_disguise {
        scale *= 0.5;
    }
    scale
}

async fn target_visibility_scale(target: &LivingEntity, observer: &LivingEntity) -> f64 {
    let equipment = target.entity_equipment.lock().await;
    let armor_coverage = [
        EquipmentSlot::HEAD,
        EquipmentSlot::CHEST,
        EquipmentSlot::LEGS,
        EquipmentSlot::FEET,
    ]
    .iter()
    .filter(|slot| !equipment.get(slot).is_empty())
    .count() as f64
        / 4.0;
    let head = equipment.get(&EquipmentSlot::HEAD).item;
    let matching_disguise = match observer.entity.entity_type {
        kind if kind == &EntityType::SKELETON => head == &Item::SKELETON_SKULL,
        kind if kind == &EntityType::ZOMBIE => head == &Item::ZOMBIE_HEAD,
        kind if kind == &EntityType::CREEPER => head == &Item::CREEPER_HEAD,
        kind if kind == &EntityType::PIGLIN || kind == &EntityType::PIGLIN_BRUTE => {
            head == &Item::PIGLIN_HEAD
        }
        _ => false,
    };
    drop(equipment);
    visibility_scale(
        target.entity.is_sneaking(),
        target
            .entity
            .invisible
            .load(std::sync::atomic::Ordering::Relaxed),
        armor_coverage,
        matching_disguise,
    )
}

pub type PredicateFn =
    dyn Fn(i32, Arc<World>) -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync;

pub struct TargetPredicate {
    pub attackable: bool,
    pub base_max_distance: f64,
    pub respects_visibility: bool,
    pub use_distance_scaling_factor: bool,
    pub predicate: Option<Arc<PredicateFn>>,
}

impl Default for TargetPredicate {
    fn default() -> Self {
        Self {
            attackable: true,
            base_max_distance: -1.0,
            respects_visibility: true,
            use_distance_scaling_factor: true,
            predicate: None,
        }
    }
}

impl TargetPredicate {
    fn new(attackable: bool) -> Self {
        Self {
            attackable,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn create_attackable() -> Self {
        Self::new(true)
    }

    #[must_use]
    pub fn create_non_attackable() -> Self {
        Self::new(false)
    }

    #[must_use]
    pub fn copy(&self) -> Self {
        Self {
            attackable: self.attackable,
            base_max_distance: self.base_max_distance,
            respects_visibility: self.respects_visibility,
            use_distance_scaling_factor: self.use_distance_scaling_factor,
            predicate: self.predicate.clone(),
        }
    }

    #[must_use]
    pub const fn set_base_max_distance(mut self, base_max_distance: f64) -> Self {
        self.base_max_distance = base_max_distance;
        self
    }

    #[must_use]
    pub const fn ignore_visibility(mut self) -> Self {
        self.respects_visibility = false;
        self
    }

    #[must_use]
    pub const fn ignore_distance_scaling_factor(mut self) -> Self {
        self.use_distance_scaling_factor = false;
        self
    }

    pub fn set_predicate<F, Fut>(&mut self, predicate: F)
    where
        F: Fn(i32, Arc<World>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = bool> + Send + 'static,
    {
        self.predicate = Some(Arc::new(move |entity_id: i32, world: Arc<World>| {
            Box::pin(predicate(entity_id, world))
        }));
    }

    pub async fn test(
        &self,
        world: &World,
        tester: Option<&LivingEntity>,
        target: &LivingEntity,
    ) -> bool {
        if tester.is_some_and(|t| std::ptr::eq(t, target)) {
            return false;
        }

        if !target.is_part_of_game() {
            return false;
        }

        // Vanilla evaluates the optional selector before attackability,
        // distance, and line-of-sight checks.
        if let Some(predicate) = &self.predicate
            && !predicate(target.entity.entity_id, target.entity.world.load_full()).await
        {
            return false;
        }

        if self.attackable
            && !attackable_target_allowed(
                target.can_take_damage(),
                target.entity.entity_type == &EntityType::PLAYER,
                world.level_info.load().difficulty == Difficulty::Peaceful,
                tester.is_some(),
            )
        {
            return false;
        }

        if let Some(tester_ent) = tester
            && self.base_max_distance > 0.0
        {
            let scale = if self.use_distance_scaling_factor {
                target_visibility_scale(target, tester_ent).await
            } else {
                1.0
            };
            let max_dist = (self.base_max_distance * scale).max(MIN_DISTANCE);
            let dist_sq = tester_ent
                .entity
                .pos
                .load()
                .squared_distance_to_vec(&target.entity.pos.load());

            if dist_sq > max_dist * max_dist {
                return false;
            }
        }

        if self.respects_visibility
            && let Some(tester_ent) = tester
            && tester_ent
                .entity
                .world
                .load_full()
                .raycast(
                    tester_ent.entity.get_eye_pos(),
                    target.entity.get_eye_pos(),
                    async |block_pos, world| world.get_block_state(block_pos).is_solid(),
                )
                .await
                .is_some()
        {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::{attackable_target_allowed, visibility_scale};

    #[test]
    fn peaceful_only_blocks_player_targets() {
        assert!(!attackable_target_allowed(true, true, true, true));
        assert!(attackable_target_allowed(true, false, true, true));
        assert!(attackable_target_allowed(true, true, false, true));
        assert!(!attackable_target_allowed(false, false, false, true));
        assert!(!attackable_target_allowed(true, false, true, false));
        assert!(attackable_target_allowed(true, false, false, false));
    }

    #[test]
    fn visibility_distance_scale_matches_vanilla_formula() {
        assert_eq!(visibility_scale(false, false, 0.0, false), 1.0);
        assert_eq!(visibility_scale(true, false, 0.0, false), 0.8);
        assert!((visibility_scale(false, true, 0.0, false) - 0.07).abs() < f64::EPSILON);
        assert!((visibility_scale(false, true, 0.5, false) - 0.35).abs() < f64::EPSILON);
        assert_eq!(visibility_scale(false, false, 0.0, true), 0.5);
        assert!((visibility_scale(true, true, 1.0, true) - 0.28).abs() < f64::EPSILON);
    }
}
