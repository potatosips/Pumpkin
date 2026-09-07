use pumpkin_util::math::{position::BlockPos, vector3::Vector3};
use pumpkin_util::random::RandomImpl;

use super::pathfinder::pathfinding_context::PathfindingContext;
use crate::entity::mob::Mob;

const ATTEMPTS: usize = 10;

/// Shared implementation of Vanilla's `DefaultRandomPos` candidate pipeline.
pub fn get_pos(mob: &dyn Mob, horizontal: i32, vertical: i32) -> Option<Vector3<f64>> {
    choose_best(mob, horizontal, || {
        let mut random = mob.get_entity_random();
        Some(Vector3::new(
            random.next_inbetween_i32(-horizontal, horizontal),
            random.next_inbetween_i32(-vertical, vertical),
            random.next_inbetween_i32(-horizontal, horizontal),
        ))
    })
}

pub fn get_pos_towards(
    mob: &dyn Mob,
    horizontal: i32,
    vertical: i32,
    target: Vector3<f64>,
    max_angle: f64,
) -> Option<Vector3<f64>> {
    let origin = mob.get_entity().pos.load();
    let direction = target - origin;
    choose_best(mob, horizontal, || {
        random_direction_within_radians(mob, horizontal, vertical, direction, max_angle)
    })
}

fn choose_best(
    mob: &dyn Mob,
    horizontal: i32,
    mut direction: impl FnMut() -> Option<Vector3<i32>>,
) -> Option<Vector3<f64>> {
    let mob_entity = mob.get_mob_entity();
    let entity = &mob_entity.living_entity.entity;
    let origin = entity.pos.load();
    let world = entity.world.load();
    let restricted = is_restriction_relevant(mob, horizontal);
    let mut best = None;
    let mut best_score = f32::NEG_INFINITY;

    for _ in 0..ATTEMPTS {
        let Some(mut offset) = direction() else {
            continue;
        };
        if restricted && horizontal > 1 {
            let center = mob_entity.position_target.load().0;
            let mut random = mob.get_entity_random();
            if origin.x > f64::from(center.x) {
                offset.x -= random.next_bounded_i32(horizontal / 2);
            } else {
                offset.x += random.next_bounded_i32(horizontal / 2);
            }
            if origin.z > f64::from(center.z) {
                offset.z -= random.next_bounded_i32(horizontal / 2);
            } else {
                offset.z += random.next_bounded_i32(horizontal / 2);
            }
        }

        let candidate = BlockPos::new(
            (origin.x + f64::from(offset.x)).floor() as i32,
            (origin.y + f64::from(offset.y)).floor() as i32,
            (origin.z + f64::from(offset.z)).floor() as i32,
        );
        if !world.is_in_height_limit(candidate.0.y)
            || (restricted && !mob_entity.is_in_position_target_range_pos(&candidate))
            || world.get_block_state(&candidate).is_solid()
            || !world.get_block_state(&candidate.down()).is_solid()
        {
            continue;
        }

        let mut context = PathfindingContext::new(entity.block_pos.load().0, world.clone());
        let path_type = context.get_land_node_type(candidate.0);
        if mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_pathfinding_malus(path_type)
            != 0.0
        {
            continue;
        }

        let score = mob.get_walk_target_value(candidate);
        if score > best_score {
            best_score = score;
            best = Some(Vector3::new(
                f64::from(candidate.0.x) + 0.5,
                f64::from(candidate.0.y),
                f64::from(candidate.0.z) + 0.5,
            ));
        }
    }
    best
}

fn is_restriction_relevant(mob: &dyn Mob, horizontal: i32) -> bool {
    let mob_entity = mob.get_mob_entity();
    let radius = mob_entity
        .position_target_range
        .load(std::sync::atomic::Ordering::Relaxed);
    if radius < 0 {
        return false;
    }
    let center = mob_entity.position_target.load().0;
    let position = mob.get_entity().pos.load();
    let dx = f64::from(center.x) + 0.5 - position.x;
    let dy = f64::from(center.y) + 0.5 - position.y;
    let dz = f64::from(center.z) + 0.5 - position.z;
    let limit = f64::from(radius + horizontal + 1);
    dx * dx + dy * dy + dz * dz < limit * limit
}

fn random_direction_within_radians(
    mob: &dyn Mob,
    horizontal: i32,
    vertical: i32,
    direction: Vector3<f64>,
    max_angle: f64,
) -> Option<Vector3<i32>> {
    let mut random = mob.get_entity_random();
    let base = direction.z.atan2(direction.x) - std::f64::consts::FRAC_PI_2;
    let angle = base + (2.0 * f64::from(random.next_f32()) - 1.0) * max_angle;
    let distance = random.next_f64().sqrt() * std::f64::consts::SQRT_2 * f64::from(horizontal);
    let x = -distance * angle.sin();
    let z = distance * angle.cos();
    if x.abs() > f64::from(horizontal) || z.abs() > f64::from(horizontal) {
        return None;
    }
    Some(Vector3::new(
        x.floor() as i32,
        random.next_inbetween_i32(-vertical, vertical),
        z.floor() as i32,
    ))
}

#[cfg(test)]
mod tests {
    use super::ATTEMPTS;

    #[test]
    fn vanilla_random_position_uses_ten_candidates() {
        assert_eq!(ATTEMPTS, 10);
    }
}
