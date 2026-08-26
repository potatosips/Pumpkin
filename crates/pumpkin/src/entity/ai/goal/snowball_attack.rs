use std::sync::Arc;

use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::math::vector3::Vector3;

use crate::entity::ai::goal::{Controls, Goal, GoalFuture};
use crate::entity::ai::pathfinder::NavigatorGoal;
use crate::entity::mob::Mob;
use crate::entity::projectile::snowball::SnowballEntity;
use crate::entity::{Entity, EntityBase};

/// Ranged snowball attack used by snow golems.
/// Matches vanilla `RangedAttackGoal` for SnowGolem: moves toward target when out of range,
/// stops and throws snowballs when in range, with smooth head tracking.
pub struct SnowballAttackGoal {
    goal_control: Controls,
    speed: f64,
    attack_interval: i32,
    squared_range: f64,
    cooldown: i32,
    see_time: i32,
}

impl SnowballAttackGoal {
    /// Vanilla snowball speed for Snow Golem shots.
    const SNOWBALL_SPEED: f64 = 1.6;

    #[must_use]
    pub fn new(speed: f64, attack_interval: i32, range: f32) -> Self {
        Self {
            goal_control: Controls::MOVE | Controls::LOOK,
            speed,
            attack_interval,
            squared_range: f64::from(range * range),
            cooldown: -1,
            see_time: 0,
        }
    }

    /// Spawns the snowball, matching vanilla `SnowGolem::performRangedAttack`.
    async fn shoot(mob: &dyn Mob, target: &Arc<dyn EntityBase>) {
        let entity = mob.get_entity();
        let world = entity.world.load();

        let mob_pos = entity.pos.load();
        let eye_height = f64::from(entity.get_eye_height());
        let eye_pos = Vector3::new(mob_pos.x, mob_pos.y + eye_height, mob_pos.z);

        let target_entity = target.get_entity();
        let target_pos = target_entity.pos.load();
        let dx = target_pos.x - mob_pos.x;
        let dz = target_pos.z - mob_pos.z;
        let horizontal_distance = dx.hypot(dz);

        // Offset spawn position forward so the projectile does not clip into self or adjacent pack mobs
        let shoot_dir = if horizontal_distance > 1e-4 {
            Vector3::new(dx / horizontal_distance, 0.0, dz / horizontal_distance)
        } else {
            Vector3::new(0.0, 0.0, 1.0)
        };
        let spawn_pos = eye_pos.add(&shoot_dir.multiply(0.5, 0.5, 0.5));

        let target_eye_y = target_pos.y + f64::from(target_entity.get_eye_height());
        let d = target_eye_y - 1.100000023841858;
        let e = target_pos.x - mob_pos.x;
        let f = d - spawn_pos.y;
        let g = target_pos.z - mob_pos.z;
        let h = e.hypot(g) * 0.20000000298023224;

        let snowball_entity = Entity::new(world.clone(), spawn_pos, &EntityType::SNOWBALL);
        let snowball = SnowballEntity::new_shot(snowball_entity, entity);
        snowball.thrown.entity.pos.store(spawn_pos);

        // Vanilla divergence for snow golem is 12.0
        snowball
            .thrown
            .set_velocity(e, f + h, g, Self::SNOWBALL_SPEED, 12.0);

        world.play_sound(
            Sound::EntitySnowGolemShoot,
            SoundCategory::Neutral,
            &mob_pos,
        );

        let snowball: Arc<dyn EntityBase> = Arc::new(snowball);
        world.spawn_entity(snowball).await;
    }
}

impl Goal for SnowballAttackGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let target = mob.get_mob_entity().target.lock().await.clone();
            let Some(target) = target else {
                return false;
            };
            target.get_entity().is_alive()
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let target = mob.get_mob_entity().target.lock().await.clone();
            let Some(target) = target else {
                return false;
            };
            target.get_entity().is_alive()
        })
    }

    fn start<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.cooldown = -1;
            self.see_time = 0;
        })
    }

    fn stop<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.cooldown = -1;
            self.see_time = 0;
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stop();
        })
    }

    fn tick<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            let target = mob.get_mob_entity().target.lock().await.clone();
            let Some(target) = target else {
                return;
            };

            let mob_pos = mob.get_entity().pos.load();
            let target_pos = target.get_entity().pos.load();
            let distance_sq = mob_pos.squared_distance_to_vec(&target_pos);

            mob.get_mob_entity()
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .look_at_entity_with_range(&target, 30.0, 30.0);

            self.see_time += 1;

            // In vanilla: pursue and chase target if distance > 4 blocks (16.0 sq) or not in direct sight
            if distance_sq > 16.0 || self.see_time < 5 {
                let mut navigator = mob
                    .get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                navigator.set_progress(NavigatorGoal {
                    current_progress: mob_pos,
                    destination: target_pos,
                    speed: self.speed,
                });
            } else {
                mob.get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .stop();
            }

            self.cooldown -= 1;
            if self.cooldown <= 0 {
                if distance_sq <= self.squared_range {
                    Self::shoot(mob, &target).await;
                    self.cooldown = self.attack_interval;
                } else {
                    self.cooldown = 10;
                }
            }
        })
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        self.goal_control
    }
}
