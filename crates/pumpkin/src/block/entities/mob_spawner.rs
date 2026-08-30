use std::{
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::{entity::EntityType, world::WorldEvent};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::{
    GameMode,
    math::{
        boundingbox::{BoundingBox, EntityDimensions},
        position::BlockPos,
        vector3::Vector3,
    },
};

use crate::{block::entities::BlockEntity, entity::EntityBase, world::World};

pub struct MobSpawnerBlockEntity {
    pub position: BlockPos,
    pub delay: AtomicI32,
    pub max_delay: i32,
    pub min_delay: i32,
    pub spawn_count: i32,
    pub spawn_range: i32,
    pub max_nearby_entities: i32,
    pub required_player_range: i32,
    pub entity_type: AtomicCell<Option<&'static EntityType>>,
}

impl MobSpawnerBlockEntity {
    pub const ID: &'static str = "minecraft:mob_spawner";
    pub const DEFAULT_DELAY: i32 = 20;
    pub const DEFAULT_MAX_SPAWN_DELAY: i32 = 800;
    pub const DEFAULT_MIN_SPAWN_DELAY: i32 = 200;
    pub const DEFAULT_SPAWN_COUNT: i32 = 4;
    pub const DEFAULT_SPAWN_RANGE: i32 = 4;
    pub const DEFAULT_MAX_NEARBY_ENTITIES: i32 = 6;
    pub const DEFAULT_REQUIRED_PLAYER_RANGE: i32 = 16;

    #[must_use]
    pub const fn new(position: BlockPos, entity_type: Option<&'static EntityType>) -> Self {
        Self {
            position,
            delay: AtomicI32::new(Self::DEFAULT_DELAY),
            max_delay: Self::DEFAULT_MAX_SPAWN_DELAY,
            min_delay: Self::DEFAULT_MIN_SPAWN_DELAY,
            spawn_count: Self::DEFAULT_SPAWN_COUNT,
            spawn_range: Self::DEFAULT_SPAWN_RANGE,
            max_nearby_entities: Self::DEFAULT_MAX_NEARBY_ENTITIES,
            required_player_range: Self::DEFAULT_REQUIRED_PLAYER_RANGE,
            entity_type: AtomicCell::new(entity_type),
        }
    }

    pub fn write_nbt<'a>(&'a self, nbt: &'a mut NbtCompound) {
        // TODO: this is ugly af
        nbt.put_string("id", self.resource_location().to_string());
        let position = self.get_position();
        nbt.put_int("x", position.0.x);
        nbt.put_int("y", position.0.y);
        nbt.put_int("z", position.0.z);
        self.write_spawn_config(nbt);
        if let Some(entity_type) = self.entity_type.load() {
            let mut spawn_entry = NbtCompound::new();

            let mut entity_nbt = NbtCompound::new();
            entity_nbt.put_string("id", format!("minecraft:{}", entity_type.resource_name));

            spawn_entry.put_compound("entity", entity_nbt);

            nbt.put_compound("SpawnData", spawn_entry);
        }
    }

    fn write_spawn_config(&self, nbt: &mut NbtCompound) {
        nbt.put_short("Delay", self.delay.load(Ordering::Relaxed) as i16);
        nbt.put_short("MinSpawnDelay", self.min_delay as i16);
        nbt.put_short("MaxSpawnDelay", self.max_delay as i16);
        nbt.put_short("SpawnCount", self.spawn_count as i16);
        nbt.put_short("SpawnRange", self.spawn_range as i16);
        nbt.put_short("MaxNearbyEntities", self.max_nearby_entities as i16);
        nbt.put_short("RequiredPlayerRange", self.required_player_range as i16);
    }
}

impl MobSpawnerBlockEntity {
    async fn update_spawns(&self, world: &Arc<World>) {
        let min_delay = self.min_delay;
        let max_delay = self.max_delay;

        self.delay.store(
            if max_delay <= min_delay {
                min_delay
            } else {
                min_delay + rand::random_range(0..max_delay - min_delay)
            },
            Ordering::Relaxed,
        );
        world.add_synced_block_event(self.position, 1, 0).await;
    }

    pub fn set_entity_type(&self, entity_type: &'static EntityType) {
        self.entity_type.store(Some(entity_type));
    }
}

impl BlockEntity for MobSpawnerBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn tick<'a>(&'a self, world: &'a Arc<World>) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let Some(entity_type) = &self.entity_type.load() {
                let center = self.position.to_centered_f64();
                let player_range_sq = f64::from(self.required_player_range).powi(2);
                if !world.players.load().iter().any(|player| {
                    player.gamemode.load() != GameMode::Spectator
                        && !player.living_entity.dead.load(Ordering::Relaxed)
                        && player.living_entity.health.load() > 0.0
                        && player
                            .get_entity()
                            .pos
                            .load()
                            .squared_distance_to_vec(&center)
                            <= player_range_sq
                }) {
                    return;
                }

                if self.delay.load(Ordering::Relaxed) < 0 {
                    self.update_spawns(world).await;
                    return;
                }
                if self.delay.load(Ordering::Relaxed) > 0 {
                    self.delay.fetch_sub(1, Ordering::Relaxed);
                    return;
                }

                let center = self.position.to_centered_f64();
                let nearby_range = f64::from(self.spawn_range);
                let mut nearby_count = world
                    .entities
                    .load()
                    .iter()
                    .filter(|entity| {
                        let entity = entity.get_entity();
                        if entity.entity_type.id != entity_type.id {
                            return false;
                        }
                        let position = entity.pos.load();
                        (position.x - center.x).abs() <= nearby_range
                            && (position.z - center.z).abs() <= nearby_range
                            && (position.y - center.y).abs() <= nearby_range
                    })
                    .count();
                if nearby_count as i32 >= self.max_nearby_entities {
                    self.update_spawns(world).await;
                    return;
                }

                let spawn_range = self.spawn_range;
                let mut spawned_any = false;
                for _ in 0..self.spawn_count {
                    // Vanilla rechecks the cap for every candidate, so a
                    // single cycle cannot overshoot MaxNearbyEntities.
                    if nearby_count as i32 >= self.max_nearby_entities {
                        self.update_spawns(world).await;
                        return;
                    }
                    let pos = self.position.0;

                    let spawn_pos = Vector3::new(
                        pos.x as f64
                            + (rand::random::<f64>() - rand::random::<f64>()) * spawn_range as f64
                            + 0.5,
                        (pos.y + rand::random_range(0..3) - 1) as f64,
                        pos.z as f64
                            + (rand::random::<f64>() - rand::random::<f64>()) * spawn_range as f64
                            + 0.5,
                    );
                    // TODO: we should use getSpawnBox, but this is only modified for slimes and magma slimes
                    if !world.is_space_empty(BoundingBox::new_from_pos(
                        spawn_pos.x,
                        spawn_pos.y,
                        spawn_pos.z,
                        &EntityDimensions {
                            width: entity_type.dimension[0],
                            height: entity_type.dimension[1],
                            eye_height: entity_type.eye_height,
                        },
                    )) {
                        continue;
                    }
                    let entity = crate::entity::r#type::from_type(
                        entity_type,
                        spawn_pos,
                        world,
                        uuid::Uuid::new_v4(),
                    );
                    entity
                        .get_entity()
                        .set_rotation(rand::random::<f32>() * 360.0, 0.0);
                    world.spawn_entity(entity).await;
                    world.sync_world_event(WorldEvent::ParticlesMobblockSpawn, self.position, 0);
                    nearby_count += 1;
                    spawned_any = true;
                }
                if spawned_any {
                    self.update_spawns(world).await;
                }
            }
        })
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let get_number = |name: &str| {
            nbt.get_short(name)
                .map(i32::from)
                .or_else(|| nbt.get_int(name))
                .or_else(|| nbt.get_byte(name).map(i32::from))
        };
        let delay = get_number("Delay").unwrap_or(Self::DEFAULT_DELAY);
        let min_delay = get_number("MinSpawnDelay").unwrap_or(Self::DEFAULT_MIN_SPAWN_DELAY);
        let max_delay = get_number("MaxSpawnDelay").unwrap_or(Self::DEFAULT_MAX_SPAWN_DELAY);
        let spawn_count = get_number("SpawnCount").unwrap_or(Self::DEFAULT_SPAWN_COUNT);
        let spawn_range = get_number("SpawnRange").unwrap_or(Self::DEFAULT_SPAWN_RANGE);
        let max_nearby_entities =
            get_number("MaxNearbyEntities").unwrap_or(Self::DEFAULT_MAX_NEARBY_ENTITIES);
        let required_player_range =
            get_number("RequiredPlayerRange").unwrap_or(Self::DEFAULT_REQUIRED_PLAYER_RANGE);

        let entity_type = nbt
            .get_compound("SpawnData")
            .and_then(|data| {
                data.get_compound("entity")
                    .and_then(|entity| entity.get_string("id"))
                    .or_else(|| data.get_string("id"))
            })
            .or_else(|| {
                nbt.get_list("SpawnPotentials")
                    .and_then(|list| list.first())
                    .and_then(|tag| tag.extract_compound())
                    .and_then(|entry| {
                        entry
                            .get_compound("data")
                            .and_then(|data| {
                                data.get_compound("entity")
                                    .and_then(|entity| entity.get_string("id"))
                                    .or_else(|| data.get_string("id"))
                            })
                            .or_else(|| {
                                entry
                                    .get_compound("entity")
                                    .and_then(|entity| entity.get_string("id"))
                            })
                    })
            })
            .or_else(|| nbt.get_string("EntityId"))
            .and_then(EntityType::from_name);

        Self {
            position,
            delay: AtomicI32::new(delay),
            max_delay,
            min_delay,
            spawn_count,
            spawn_range,
            max_nearby_entities,
            required_player_range,
            entity_type: AtomicCell::new(entity_type),
        }
    }

    fn write_nbt<'a>(
        &'a self,
        nbt: &'a mut NbtCompound,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            self.write_nbt(nbt);
        })
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut final_nbt = NbtCompound::new();
        self.write_spawn_config(&mut final_nbt);
        if let Some(entity_type) = self.entity_type.load() {
            let mut spawn_entry = NbtCompound::new();

            let mut entity_nbt = NbtCompound::new();
            entity_nbt.put_string("id", format!("minecraft:{}", entity_type.resource_name));

            spawn_entry.put_compound("entity", entity_nbt);

            final_nbt.put_compound("SpawnData", spawn_entry);
        }
        Some(final_nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_spawner_defaults_are_preserved() {
        let spawner = MobSpawnerBlockEntity::new(BlockPos::new(1, 2, 3), None);
        assert_eq!(spawner.delay.load(Ordering::Relaxed), 20);
        assert_eq!(spawner.min_delay, 200);
        assert_eq!(spawner.max_delay, 800);
        assert_eq!(spawner.spawn_count, 4);
        assert_eq!(spawner.spawn_range, 4);
        assert_eq!(spawner.max_nearby_entities, 6);
        assert_eq!(spawner.required_player_range, 16);
    }

    #[test]
    fn spawner_reads_vanilla_numeric_nbt_widths_and_entity_id() {
        let mut nbt = NbtCompound::new();
        nbt.put_short("Delay", 37);
        nbt.put_int("MinSpawnDelay", 101);
        nbt.put_byte("MaxSpawnDelay", 99);
        nbt.put_short("SpawnCount", 3);
        nbt.put_int("SpawnRange", 7);
        nbt.put_short("MaxNearbyEntities", 9);
        nbt.put_byte("RequiredPlayerRange", 12);
        let mut entity = NbtCompound::new();
        entity.put_string("id", "minecraft:zombie".to_string());
        let mut spawn_data = NbtCompound::new();
        spawn_data.put_compound("entity", entity);
        nbt.put_compound("SpawnData", spawn_data);

        let spawner =
            <MobSpawnerBlockEntity as BlockEntity>::from_nbt(&nbt, BlockPos::new(0, 64, 0));
        assert_eq!(spawner.delay.load(Ordering::Relaxed), 37);
        assert_eq!(spawner.min_delay, 101);
        assert_eq!(spawner.max_delay, 99);
        assert_eq!(spawner.spawn_count, 3);
        assert_eq!(spawner.spawn_range, 7);
        assert_eq!(spawner.max_nearby_entities, 9);
        assert_eq!(spawner.required_player_range, 12);
        assert_eq!(spawner.entity_type.load(), Some(&EntityType::ZOMBIE));
    }

    #[test]
    fn spawner_chunk_nbt_exposes_client_animation_configuration() {
        let spawner =
            MobSpawnerBlockEntity::new(BlockPos::new(0, 64, 0), Some(&EntityType::SKELETON));
        let nbt = spawner.chunk_data_nbt().expect("spawner chunk NBT");
        assert_eq!(nbt.get_short("Delay"), Some(20));
        assert_eq!(nbt.get_short("MinSpawnDelay"), Some(200));
        assert_eq!(nbt.get_short("MaxSpawnDelay"), Some(800));
        assert_eq!(nbt.get_short("SpawnCount"), Some(4));
        assert_eq!(nbt.get_short("SpawnRange"), Some(4));
        assert_eq!(nbt.get_short("MaxNearbyEntities"), Some(6));
        assert_eq!(nbt.get_short("RequiredPlayerRange"), Some(16));
        assert_eq!(
            nbt.get_compound("SpawnData")
                .and_then(|data| data.get_compound("entity"))
                .and_then(|entity| entity.get_string("id")),
            Some("minecraft:skeleton")
        );
    }
}
