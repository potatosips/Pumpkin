use crate::{
    entity::{Entity, EntityBase, EntityBaseFuture, NBTStorage, projectile::ThrownItemEntity},
    server::Server,
    world::World,
};
use pumpkin_data::damage::DamageType;
use pumpkin_data::data_component_impl::FireworksImpl;
use pumpkin_data::entity::EntityStatus;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_protocol::bedrock::server::actor_event::ActorEventType;
use pumpkin_protocol::codec::item_stack_seralizer::ItemStackSerializer;
use pumpkin_protocol::{codec::optional_int::OptionalInt, java::client::play::Metadata};
use pumpkin_util::{
    math::vector3::Vector3,
    random::{RandomGenerator, RandomImpl, get_seed, xoroshiro128::Xoroshiro},
};
use std::sync::atomic::AtomicBool;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
use tokio::sync::RwLock;

const GRAVITY: f64 = 0.0;

pub struct FireworkRocketEntity {
    entity: ThrownItemEntity,
    life: AtomicU32,
    life_time: AtomicU32,
    item_stack: RwLock<ItemStack>,
    shot_at_angle: AtomicBool,
}

impl FireworkRocketEntity {
    pub fn new(entity: Entity) -> Self {
        Self::new_with_item(entity, &ItemStack::new(1, &Item::FIREWORK_ROCKET))
    }

    fn life_time(flight_duration: i32, first_random: u32, second_random: u32) -> u32 {
        10 * (1 + flight_duration.max(0) as u32) + first_random + second_random
    }

    pub fn new_with_item(entity: Entity, item_stack: &ItemStack) -> Self {
        let mut random = RandomGenerator::Xoroshiro(Xoroshiro::from_seed(get_seed()));
        let flight_duration = item_stack
            .get_data_component::<FireworksImpl>()
            .map_or(1, |fireworks| fireworks.flight_duration);
        entity.set_velocity(Vector3::new(
            random.next_triangular(0.0, 0.002_297),
            0.05,
            random.next_triangular(0.0, 0.002_297),
        ));
        Self {
            entity: ThrownItemEntity {
                entity,
                owner_id: None,
                collides_with_projectiles: false,
                has_hit: AtomicBool::new(false),
                gravity: GRAVITY,
            },
            life: 0.into(),
            life_time: Self::life_time(
                flight_duration,
                random.next_bounded_i32(6) as u32,
                random.next_bounded_i32(7) as u32,
            )
            .into(),
            item_stack: RwLock::new(item_stack.copy_with_count(1)),
            shot_at_angle: AtomicBool::new(false),
        }
    }

    pub fn new_shot(entity: Entity, shooter: &Entity, item_stack: &ItemStack) -> Self {
        let mut random = RandomGenerator::Xoroshiro(Xoroshiro::from_seed(get_seed()));
        let flight_duration = item_stack
            .get_data_component::<FireworksImpl>()
            .map_or(1, |fireworks| fireworks.flight_duration);

        // Set random initial velocity
        // Set on the inner entity after constructing ThrownItemEntity
        let thrown = ThrownItemEntity::new(entity, shooter, GRAVITY);
        thrown.entity.set_velocity(Vector3::new(
            random.next_triangular(0.0, 0.002_297),
            0.05,
            random.next_triangular(0.0, 0.002_297),
        ));

        // Set random life
        let rocket = Self {
            entity: thrown,
            life: 0.into(),
            life_time: Self::life_time(
                flight_duration,
                random.next_bounded_i32(6) as u32,
                random.next_bounded_i32(7) as u32,
            )
            .into(),
            item_stack: RwLock::new(item_stack.copy_with_count(1)),
            shot_at_angle: AtomicBool::new(false),
        };

        // Set shooter metadata
        rocket.entity.entity.send_meta_data(
            &[Metadata::new(
                pumpkin_data::tracked_data::firework_rocket::ATTACHED_TO_TARGET,
                OptionalInt(Some(shooter.entity_id)),
            )],
            None,
        );

        rocket
    }

    pub fn new_crossbow_shot(entity: Entity, shooter: &Entity, item_stack: &ItemStack) -> Self {
        let mut random = RandomGenerator::Xoroshiro(Xoroshiro::from_seed(get_seed()));
        let flight_duration = item_stack
            .get_data_component::<FireworksImpl>()
            .map_or(1, |fireworks| fireworks.flight_duration);
        Self {
            entity: ThrownItemEntity::new(entity, shooter, GRAVITY),
            life: 0.into(),
            life_time: Self::life_time(
                flight_duration,
                random.next_bounded_i32(6) as u32,
                random.next_bounded_i32(7) as u32,
            )
            .into(),
            item_stack: RwLock::new(item_stack.copy_with_count(1)),
            shot_at_angle: AtomicBool::new(true),
        }
    }

    pub fn set_velocity_from_rotation(&self, pitch: f32, yaw: f32, speed: f32, divergence: f32) {
        self.entity
            .set_velocity_from(&self.entity.entity, pitch, yaw, 0.0, speed, divergence);
    }

    const fn explosion_base_damage(explosion_count: usize) -> f32 {
        if explosion_count == 0 {
            0.0
        } else {
            5.0 + 2.0 * explosion_count as f32
        }
    }

    fn distance_scaled_damage(base_damage: f32, distance: f64) -> f32 {
        if base_damage <= 0.0 || distance >= 5.0 {
            0.0
        } else {
            base_damage * ((5.0 - distance) / 5.0).sqrt() as f32
        }
    }

    pub async fn explode_and_remove(
        &self,
        world: &Arc<World>,
        direct_hit: Option<&Arc<dyn EntityBase>>,
    ) {
        let entity = self.get_entity();
        world.send_entity_status(
            entity,
            EntityStatus::FireworksExplode,
            Some(ActorEventType::FireworksExplode),
        );

        let explosion_count = self
            .item_stack
            .read()
            .await
            .get_data_component::<FireworksImpl>()
            .map_or(0, |fireworks| fireworks.explosions.len());
        let base_damage = Self::explosion_base_damage(explosion_count);
        if base_damage > 0.0 {
            if let Some(target) = direct_hit
                && target.get_living_entity().is_some()
            {
                target
                    .damage_with_context(
                        target.as_ref(),
                        base_damage,
                        DamageType::FIREWORKS,
                        Some(entity.pos.load()),
                        None,
                        Some(self),
                    )
                    .await;
            }

            let origin = entity.pos.load();
            let nearby =
                world.get_entities_at_box(&entity.bounding_box.load().expand(5.0, 5.0, 5.0));
            for target in nearby {
                if direct_hit.is_some_and(|direct| {
                    direct.get_entity().entity_id == target.get_entity().entity_id
                }) || target.get_living_entity().is_none()
                {
                    continue;
                }
                let target_entity = target.get_entity();
                if !self.shot_at_angle.load(Ordering::Relaxed)
                    && self.entity.owner_id == Some(target_entity.entity_id)
                {
                    continue;
                }
                let distance = origin
                    .squared_distance_to_vec(&target_entity.pos.load())
                    .sqrt();
                let damage = Self::distance_scaled_damage(base_damage, distance);
                if damage <= 0.0 {
                    continue;
                }
                let target_pos = target_entity.pos.load();
                let half_height = f64::from(target_entity.entity_dimension.load().height) * 0.5;
                let mut visible = false;
                for y_offset in [0.0, half_height] {
                    let sample = target_pos.add_raw(0.0, y_offset, 0.0);
                    if world
                        .raycast(origin, sample, async |block_pos, world| {
                            world.get_block_state(block_pos).is_solid()
                        })
                        .await
                        .is_none()
                    {
                        visible = true;
                        break;
                    }
                }
                if visible {
                    target
                        .damage_with_context(
                            target.as_ref(),
                            damage,
                            DamageType::FIREWORKS,
                            Some(origin),
                            None,
                            Some(self),
                        )
                        .await;
                }
            }
        }

        entity.remove().await;
    }
}

impl NBTStorage for FireworkRocketEntity {
    fn write_nbt<'a>(
        &'a self,
        nbt: &'a mut pumpkin_nbt::compound::NbtCompound,
    ) -> crate::entity::NbtFuture<'a, ()> {
        Box::pin(async move {
            self.entity.entity.write_nbt(nbt).await;
            nbt.put_int("Life", self.life.load(Ordering::Relaxed) as i32);
            nbt.put_int("LifeTime", self.life_time.load(Ordering::Relaxed) as i32);
            nbt.put_byte(
                "ShotAtAngle",
                i8::from(self.shot_at_angle.load(Ordering::Relaxed)),
            );
            let mut item = pumpkin_nbt::compound::NbtCompound::new();
            self.item_stack
                .read()
                .await
                .copy_with_count(1)
                .write_item_stack(&mut item);
            nbt.put_compound("FireworksItem", item);
        })
    }

    fn read_nbt_non_mut<'a>(
        &'a self,
        nbt: &'a pumpkin_nbt::compound::NbtCompound,
    ) -> crate::entity::NbtFuture<'a, ()> {
        Box::pin(async move {
            self.entity.entity.read_nbt_non_mut(nbt).await;
            if let Some(life) = nbt.get_int("Life") {
                self.life.store(life.max(0) as u32, Ordering::Relaxed);
            }
            if let Some(life_time) = nbt.get_int("LifeTime") {
                self.life_time
                    .store(life_time.max(0) as u32, Ordering::Relaxed);
            }
            if let Some(shot_at_angle) = nbt.get_byte("ShotAtAngle") {
                self.shot_at_angle
                    .store(shot_at_angle != 0, Ordering::Relaxed);
            }
            if let Some(item) = nbt
                .get_compound("FireworksItem")
                .and_then(ItemStack::read_item_stack)
            {
                *self.item_stack.write().await = item.copy_with_count(1);
            }
        })
    }
}

impl EntityBase for FireworkRocketEntity {
    fn tick<'a>(
        &'a self,
        caller: &'a Arc<dyn EntityBase>,
        server: &'a Server,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            self.entity.process_tick(caller, server).await;

            let entity = self.get_entity();
            if entity.is_removed() {
                return;
            }
            let world = entity.world.load();
            let mut velocity = entity.velocity.load();

            if let Some(shooter_id) = self.entity.owner_id {
                // Check if the player who fired this rocket still exists in the world
                if let Some(shooter) = world.get_entity_by_id(shooter_id) {
                    let shooter = shooter.get_entity();

                    // Logic for boosting Elytra flight
                    if shooter.is_fall_flying() {
                        let rotation = shooter.rotation().to_f64();
                        let shooter_vel = shooter.velocity.load();

                        let new_shooter_vel =
                            shooter_vel + (rotation * 0.1 + (rotation * 1.5 - shooter_vel) * 0.5);

                        shooter.set_velocity(new_shooter_vel);

                        entity.set_pos(shooter.pos.load());
                        entity.set_velocity(new_shooter_vel);
                    }
                }
            } else {
                // Standard firework rocket flight logic
                velocity.x *= 1.15;
                velocity.z *= 1.15;
                velocity.y += 0.04;
                entity.set_velocity(velocity);
            }

            // Increment life and check for explosion
            let current_life = self.life.fetch_add(1, Ordering::Relaxed);
            if current_life > self.life_time.load(Ordering::Relaxed) {
                self.explode_and_remove(&world, None).await;
            }
        })
    }

    fn get_entity(&self) -> &crate::entity::Entity {
        &self.entity.entity
    }

    fn init_data_tracker(&self) -> EntityBaseFuture<'_, ()> {
        Box::pin(async {
            self.entity.entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::firework_rocket::ID_FIREWORKS_ITEM,
                    &ItemStackSerializer::from(self.item_stack.read().await.clone()),
                )],
                None,
            );
            self.entity.entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::firework_rocket::SHOT_AT_ANGLE,
                    self.shot_at_angle.load(Ordering::Relaxed),
                )],
                None,
            );
        })
    }

    fn on_hit(&self, hit: crate::entity::projectile::ProjectileHit) -> EntityBaseFuture<'_, ()> {
        Box::pin(async move {
            let world = self.entity.entity.world.load();
            let direct_hit = match &hit {
                crate::entity::projectile::ProjectileHit::Entity { entity, .. } => Some(entity),
                crate::entity::projectile::ProjectileHit::Block { .. } => None,
            };
            self.explode_and_remove(&world, direct_hit).await;
        })
    }

    fn get_living_entity(&self) -> Option<&crate::entity::living::LivingEntity> {
        None
    }

    fn as_nbt_storage(&self) -> &dyn crate::entity::NBTStorage {
        self
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::FireworkRocketEntity;

    #[test]
    fn lifetime_scales_with_vanilla_flight_duration() {
        assert_eq!(FireworkRocketEntity::life_time(0, 0, 0), 10);
        assert_eq!(FireworkRocketEntity::life_time(1, 0, 0), 20);
        assert_eq!(FireworkRocketEntity::life_time(3, 5, 6), 51);
    }

    #[test]
    fn explosion_damage_matches_vanilla_count_and_distance_scaling() {
        assert_eq!(FireworkRocketEntity::explosion_base_damage(0), 0.0);
        assert_eq!(FireworkRocketEntity::explosion_base_damage(1), 7.0);
        assert_eq!(FireworkRocketEntity::explosion_base_damage(3), 11.0);
        assert_eq!(FireworkRocketEntity::distance_scaled_damage(7.0, 0.0), 7.0);
        assert_eq!(FireworkRocketEntity::distance_scaled_damage(7.0, 5.0), 0.0);
    }
}
