use std::sync::atomic::{AtomicI32, Ordering};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::entity::EntityStatus;
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::bedrock::server::actor_event::ActorEventType;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

use crate::entity::Entity;

pub(super) struct TntMinecart {
    fuse: AtomicI32,
    explosion_power: AtomicCell<f32>,
    explosion_speed_factor: AtomicCell<f32>,
}

impl TntMinecart {
    pub(super) const fn new() -> Self {
        Self {
            fuse: AtomicI32::new(-1),
            explosion_power: AtomicCell::new(4.0),
            explosion_speed_factor: AtomicCell::new(1.0),
        }
    }

    pub(super) fn prime(&self, entity: &Entity, fuse: i32) {
        if self.fuse.load(Ordering::Relaxed) >= 0
            || !entity
                .world
                .load()
                .level_info
                .load()
                .game_rules
                .tnt_explodes
        {
            return;
        }

        self.fuse.store(fuse, Ordering::Relaxed);
        let world = entity.world.load();
        world.send_entity_status(
            entity,
            EntityStatus::TntPrime,
            Some(ActorEventType::CartWithPrimeTNT),
        );
        world.play_sound(
            Sound::EntityTntPrimed,
            SoundCategory::Blocks,
            &entity.pos.load(),
        );
    }

    pub(super) async fn tick(&self, entity: &Entity) -> bool {
        let fuse = self.fuse.load(Ordering::Relaxed);
        if fuse > 0 {
            self.fuse.store(fuse - 1, Ordering::Relaxed);
            let mut smoke_pos = entity.pos.load();
            smoke_pos.y += 0.5;
            entity.world.load().spawn_particle(
                smoke_pos,
                Vector3::new(0.0, 0.0, 0.0),
                0.0,
                1,
                Particle::Smoke,
            );
        } else if fuse == 0 {
            let velocity = entity.velocity.load();
            self.explode(
                entity,
                velocity.x.mul_add(velocity.x, velocity.z * velocity.z),
            )
            .await;
            return true;
        }
        false
    }

    pub(super) async fn explode(&self, entity: &Entity, horizontal_speed_squared: f64) {
        let world = entity.world.load();
        if !world.level_info.load().game_rules.tnt_explodes {
            if self.fuse.load(Ordering::Relaxed) > -1 {
                entity.remove().await;
            }
            return;
        }

        let power = Self::explosion_strength(
            self.explosion_power.load(),
            self.explosion_speed_factor.load(),
            horizontal_speed_squared,
            rand::rng().random_range(0.0..1.0),
        );
        let pos = entity.pos.load();
        let primed = self.fuse.load(Ordering::Relaxed) > -1;
        entity.remove().await;
        if primed {
            world.explode_tnt_minecart(pos, power).await;
        } else {
            world.explode_tnt(pos, power).await;
        }
    }

    pub(super) fn set_fuse(&self, fuse: i32) {
        self.fuse.store(fuse, Ordering::Relaxed);
    }

    pub(super) fn write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_int("fuse", self.fuse.load(Ordering::Relaxed));
        let power = self.explosion_power.load();
        if power != 4.0 {
            nbt.put_float("explosion_power", power);
        }
        let speed_factor = self.explosion_speed_factor.load();
        if speed_factor != 1.0 {
            nbt.put_float("explosion_speed_factor", speed_factor);
        }
    }

    pub(super) fn read_nbt(&self, nbt: &NbtCompound) {
        self.fuse
            .store(numeric_i32(nbt, "fuse").unwrap_or(-1), Ordering::Relaxed);
        self.explosion_power.store(
            numeric_f32(nbt, "explosion_power")
                .unwrap_or(4.0)
                .clamp(0.0, 128.0),
        );
        self.explosion_speed_factor.store(
            numeric_f32(nbt, "explosion_speed_factor")
                .unwrap_or(1.0)
                .clamp(0.0, 128.0),
        );
    }

    fn explosion_strength(
        base: f32,
        speed_factor: f32,
        horizontal_speed_squared: f64,
        random: f32,
    ) -> f32 {
        let speed = horizontal_speed_squared.sqrt().min(5.0) as f32;
        base + speed_factor * random * 1.5 * speed
    }
}

pub(super) fn should_explode_on_horizontal_collision(
    horizontal_collision: bool,
    post_collision_speed_squared: f64,
) -> bool {
    horizontal_collision && post_collision_speed_squared >= 0.01
}

pub(super) fn retained_horizontal_speed_squared(
    requested_x: f64,
    requested_z: f64,
    actual_x: f64,
    actual_z: f64,
    retained_x: f64,
    retained_z: f64,
) -> f64 {
    const EPSILON: f64 = 1.0e-7;
    let x = if (requested_x - actual_x).abs() > EPSILON {
        0.0
    } else {
        retained_x
    };
    let z = if (requested_z - actual_z).abs() > EPSILON {
        0.0
    } else {
        retained_z
    };
    x.mul_add(x, z * z)
}

fn numeric_i32(nbt: &NbtCompound, key: &str) -> Option<i32> {
    nbt.get_byte(key)
        .map(i32::from)
        .or_else(|| nbt.get_short(key).map(i32::from))
        .or_else(|| nbt.get_int(key))
        .or_else(|| nbt.get_long(key).map(|value| value as i32))
        .or_else(|| nbt.get_float(key).map(|value| value as i32))
        .or_else(|| nbt.get_double(key).map(|value| value as i32))
}

fn numeric_f32(nbt: &NbtCompound, key: &str) -> Option<f32> {
    nbt.get_byte(key)
        .map(f32::from)
        .or_else(|| nbt.get_short(key).map(f32::from))
        .or_else(|| nbt.get_int(key).map(|value| value as f32))
        .or_else(|| nbt.get_long(key).map(|value| value as f32))
        .or_else(|| nbt.get_float(key))
        .or_else(|| nbt.get_double(key).map(|value| value as f32))
}

#[cfg(test)]
mod tests {
    use super::{
        TntMinecart, numeric_f32, numeric_i32, retained_horizontal_speed_squared,
        should_explode_on_horizontal_collision,
    };
    use pumpkin_nbt::compound::NbtCompound;

    #[test]
    fn tnt_minecart_explosion_bonus_is_speed_capped() {
        assert_eq!(TntMinecart::explosion_strength(4.0, 1.0, 100.0, 1.0), 11.5);
        assert_eq!(TntMinecart::explosion_strength(4.0, 1.0, 100.0, 0.0), 4.0);
    }

    #[test]
    fn tnt_minecart_accepts_vanilla_numeric_nbt_types() {
        let mut nbt = NbtCompound::new();
        nbt.put_short("fuse", 7);
        nbt.put_double("explosion_power", 6.5);
        nbt.put_byte("explosion_speed_factor", 2);

        assert_eq!(numeric_i32(&nbt, "fuse"), Some(7));
        assert_eq!(numeric_f32(&nbt, "explosion_power"), Some(6.5));
        assert_eq!(numeric_f32(&nbt, "explosion_speed_factor"), Some(2.0));
    }

    #[test]
    fn collision_explosion_uses_post_collision_horizontal_speed() {
        assert!(!should_explode_on_horizontal_collision(false, 1.0));
        assert!(!should_explode_on_horizontal_collision(true, 0.009_999));
        assert!(should_explode_on_horizontal_collision(true, 0.01));

        assert_eq!(
            retained_horizontal_speed_squared(1.0, 0.0, 0.31, 0.0, 0.2945, 0.0),
            0.0
        );
        assert_eq!(
            retained_horizontal_speed_squared(1.0, 0.4, 0.31, 0.4, 0.2945, 0.38),
            0.38_f64.powi(2)
        );
    }
}
