use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::{Block, BlockStateId, item::Item, item_stack::ItemStack};
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
use pumpkin_protocol::java::client::play::Metadata;
use pumpkin_util::math::{atomic_f32::AtomicF32, position::BlockPos};
use pumpkin_util::text::{TextComponent, hover::HoverEvent};
use pumpkin_world::generation::structure::template::{BlockStateResolver, PaletteEntry};
use pumpkin_world::world::BlockFlags;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicI32, AtomicU16, Ordering},
};
use tokio::sync::Mutex;

use crate::{
    block::blocks::concrete_powder::ConcretePowderBlock,
    entity::{Entity, EntityBase, EntityBaseFuture, NBTStorage, NbtFuture, living::LivingEntity},
    server::Server,
    world::World,
};

pub struct FallingEntity {
    entity: Entity,
    block_state_id: AtomicU16,
    time: AtomicI32,
    drop_item: AtomicBool,
    hurt_entities: AtomicBool,
    fall_hurt_amount: AtomicF32,
    fall_hurt_max: AtomicI32,
    fall_distance: AtomicF32,
    cancel_drop: AtomicBool,
    block_entity_data: Mutex<Option<NbtCompound>>,
}

impl FallingEntity {
    pub fn new(entity: Entity, block_state_id: BlockStateId) -> Self {
        let block_id = block_state_id.to_block_id();
        let (hurt_entities, fall_hurt_amount) = if block_id == Block::ANVIL.id
            || block_id == Block::CHIPPED_ANVIL.id
            || block_id == Block::DAMAGED_ANVIL.id
        {
            (true, 2.0)
        } else if block_id == Block::POINTED_DRIPSTONE.id {
            (true, 6.0)
        } else {
            (false, 0.0)
        };
        Self {
            entity,
            block_state_id: AtomicU16::new(block_state_id.as_u16()),
            time: AtomicI32::new(0),
            drop_item: AtomicBool::new(true),
            hurt_entities: AtomicBool::new(hurt_entities),
            fall_hurt_amount: AtomicF32::new(fall_hurt_amount),
            fall_hurt_max: AtomicI32::new(40),
            fall_distance: AtomicF32::new(0.0),
            cancel_drop: AtomicBool::new(false),
            block_entity_data: Mutex::const_new(None),
        }
    }

    fn block_state_id(&self) -> BlockStateId {
        BlockStateId::new_or_air(self.block_state_id.load(Ordering::Relaxed))
    }

    fn block_state_to_nbt(state_id: BlockStateId) -> NbtCompound {
        let block = state_id.to_block();
        let mut state = NbtCompound::new();
        state.put_string("Name", format!("minecraft:{}", block.name));
        if let Some(properties) = block.properties(state_id) {
            let properties = properties.to_props();
            if !properties.is_empty() {
                let mut property_nbt = NbtCompound::new();
                for (name, value) in properties {
                    property_nbt.put_string(name, value.to_string());
                }
                state.put("Properties", NbtTag::Compound(property_nbt));
            }
        }
        state
    }

    fn block_state_from_nbt(nbt: &NbtCompound) -> Option<BlockStateId> {
        let state = nbt.get_compound("BlockState")?;
        let name = state.get_string("Name")?.to_string();
        let mut properties: Vec<(String, String)> =
            state
                .get_compound("Properties")
                .map_or_else(Vec::new, |properties| {
                    properties
                        .child_tags
                        .iter()
                        .filter_map(|(name, value)| match value {
                            NbtTag::String(value) => Some((name.to_string(), value.to_string())),
                            _ => None,
                        })
                        .collect()
                });
        let block_name = name.strip_prefix("minecraft:").unwrap_or(&name);
        let block = Block::from_name(&name).or_else(|| Block::from_registry_key(block_name))?;
        properties.retain(|(property_name, property_value)| {
            block.states.iter().any(|candidate| {
                block
                    .properties(candidate.id)
                    .is_some_and(|candidate_properties| {
                        candidate_properties
                            .to_props()
                            .iter()
                            .any(|(name, value)| *name == property_name && *value == property_value)
                    })
            })
        });
        BlockStateResolver::resolve_simple(&PaletteEntry::with_properties(name, properties))
            .map(|state| state.id)
    }

    fn impact_damage(&self, fall_distance: f32) -> Option<f32> {
        if !self.hurt_entities.load(Ordering::Relaxed) {
            return None;
        }
        Self::calculate_impact_damage(
            fall_distance,
            self.fall_hurt_amount.load(Ordering::Relaxed),
            self.fall_hurt_max.load(Ordering::Relaxed),
        )
    }

    fn calculate_impact_damage(
        fall_distance: f32,
        fall_hurt_amount: f32,
        fall_hurt_max: i32,
    ) -> Option<f32> {
        let distance_steps = Self::fall_damage_steps(fall_distance);
        if distance_steps <= 0 {
            return None;
        }
        let damage = ((distance_steps as f32) * fall_hurt_amount)
            .floor()
            .min(fall_hurt_max as f32);
        (damage > 0.0).then_some(damage)
    }

    fn fall_damage_steps(fall_distance: f32) -> i32 {
        (fall_distance - 1.0).ceil() as i32
    }

    fn should_time_out(time: i32, y: i32, bottom_y: i32, top_y: i32) -> bool {
        time > 600 || (time > 100 && (y <= bottom_y || y > top_y))
    }

    fn is_anvil(state_id: BlockStateId) -> bool {
        matches!(
            state_id.to_block().id,
            id if id == Block::ANVIL.id
                || id == Block::CHIPPED_ANVIL.id
                || id == Block::DAMAGED_ANVIL.id
        )
    }

    /// Returns the next damaged anvil state, preserving shared properties. A
    /// damaged anvil has no successor and is destroyed instead.
    fn damaged_anvil_state(state_id: BlockStateId) -> Option<BlockStateId> {
        let block = state_id.to_block();
        let successor = if block.id == Block::ANVIL.id {
            &Block::CHIPPED_ANVIL
        } else if block.id == Block::CHIPPED_ANVIL.id {
            &Block::DAMAGED_ANVIL
        } else {
            return None;
        };
        let properties = block
            .properties(state_id)
            .map(|properties| {
                properties
                    .to_props()
                    .into_iter()
                    .map(|(name, value)| (name.to_string(), value.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        BlockStateResolver::resolve_simple(&PaletteEntry::with_properties(
            format!("minecraft:{}", successor.name),
            properties,
        ))
        .map(|state| state.id)
    }

    /// Replaced the current Block and Spawns a new Falling one
    pub async fn replace_spawn(world: &Arc<World>, position: BlockPos, block_state: BlockStateId) {
        // Replace the original block, TODO: use fluid state
        world
            .set_block_state(
                &position,
                Block::AIR.default_state.id,
                BlockFlags::NOTIFY_ALL,
            )
            .await;

        let position = position.0.to_f64().add_raw(0.5, 0.0, 0.5);
        let entity = Entity::new(world.clone(), position, &EntityType::FALLING_BLOCK);
        entity
            .data
            .store(i32::from(block_state.as_u16()), Ordering::Relaxed);
        let entity = Arc::new(Self::new(entity, block_state));
        world.spawn_entity(entity).await;
    }
}

impl NBTStorage for FallingEntity {
    fn write_nbt<'a>(
        &'a self,
        nbt: &'a mut pumpkin_nbt::compound::NbtCompound,
    ) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.entity.write_nbt(nbt).await;
            nbt.put(
                "BlockState",
                NbtTag::Compound(Self::block_state_to_nbt(self.block_state_id())),
            );
            nbt.put_int("Time", self.time.load(Ordering::Relaxed));
            nbt.put_bool("DropItem", self.drop_item.load(Ordering::Relaxed));
            nbt.put_bool("HurtEntities", self.hurt_entities.load(Ordering::Relaxed));
            nbt.put_float(
                "FallHurtAmount",
                self.fall_hurt_amount.load(Ordering::Relaxed),
            );
            nbt.put_int("FallHurtMax", self.fall_hurt_max.load(Ordering::Relaxed));
            nbt.put_float("FallDistance", self.fall_distance.load(Ordering::Relaxed));
            nbt.put_bool("CancelDrop", self.cancel_drop.load(Ordering::Relaxed));
            if let Some(block_entity_data) = self.block_entity_data.lock().await.as_ref() {
                nbt.put(
                    "TileEntityData",
                    NbtTag::Compound(block_entity_data.clone()),
                );
            } else {
                nbt.child_tags.remove("TileEntityData");
            }
        })
    }

    fn read_nbt_non_mut<'a>(
        &'a self,
        nbt: &'a pumpkin_nbt::compound::NbtCompound,
    ) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.entity.read_nbt_non_mut(nbt).await;
            let state_id = Self::block_state_from_nbt(nbt).unwrap_or(Block::SAND.default_state.id);
            self.block_state_id
                .store(state_id.as_u16(), Ordering::Relaxed);
            self.entity
                .data
                .store(i32::from(state_id.as_u16()), Ordering::Relaxed);
            self.time
                .store(nbt.get_int("Time").unwrap_or(0), Ordering::Relaxed);
            self.drop_item
                .store(nbt.get_bool("DropItem").unwrap_or(true), Ordering::Relaxed);
            self.hurt_entities.store(
                nbt.get_bool("HurtEntities").unwrap_or(false),
                Ordering::Relaxed,
            );
            self.fall_hurt_amount.store(
                nbt.get_float("FallHurtAmount").unwrap_or(0.0),
                Ordering::Relaxed,
            );
            self.fall_hurt_max
                .store(nbt.get_int("FallHurtMax").unwrap_or(40), Ordering::Relaxed);
            self.fall_distance.store(
                nbt.get_float("FallDistance")
                    .or_else(|| nbt.get_float("fall_distance"))
                    .unwrap_or(0.0),
                Ordering::Relaxed,
            );
            self.cancel_drop.store(
                nbt.get_bool("CancelDrop").unwrap_or(false),
                Ordering::Relaxed,
            );
            *self.block_entity_data.lock().await = nbt.get_compound("TileEntityData").cloned();
        })
    }
}

impl EntityBase for FallingEntity {
    fn tick<'a>(
        &'a self,
        caller: &'a Arc<dyn EntityBase>,
        server: &'a Server,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            let entity = &self.entity;
            entity.tick(caller, server).await;
            let time = self.time.fetch_add(1, Ordering::Relaxed) + 1;
            let world = entity.world.load();
            let position = entity.block_pos.load();
            let mut state_id = self.block_state_id();
            if let Some(concrete) = ConcretePowderBlock::concrete_for_powder(state_id.to_block().id)
            {
                let curr_state = world.get_block_state(&position);
                if ConcretePowderBlock::is_water(curr_state) {
                    state_id = concrete.default_state.id;
                    self.block_state_id
                        .store(state_id.as_u16(), Ordering::Relaxed);
                    self.entity
                        .data
                        .store(i32::from(state_id.as_u16()), Ordering::Relaxed);
                }
            }
            if Self::should_time_out(time, position.0.y, world.get_bottom_y(), world.get_top_y()) {
                let state_id = self.block_state_id();
                if self.drop_item.load(Ordering::Relaxed)
                    && world.level_info.load().game_rules.entity_drops
                    && let Some(item) = Item::from_id(state_id.to_block().item_id)
                {
                    world.drop_stack(&position, ItemStack::new(1, item)).await;
                }
                entity.remove().await;
                return;
            }

            let original_velo = entity.velocity.load();
            let mut velo = original_velo;
            if !entity.has_no_gravity() {
                velo.y -= self.get_gravity();
            }

            entity.velocity.store(velo);

            let before_y = entity.pos.load().y;
            entity.move_entity(caller, velo).await;
            let fallen = (before_y - entity.pos.load().y).max(0.0) as f32;
            entity.tick_block_collisions(caller, server).await;
            if entity.on_ground.load(Ordering::Relaxed) {
                let position = entity.block_pos.load();
                let fall_distance = self.fall_distance.load(Ordering::Relaxed) + fallen;
                self.fall_distance.store(0.0, Ordering::Relaxed);
                if let Some(damage) = self.impact_damage(fall_distance) {
                    let state_id = self.block_state_id();
                    let is_anvil = Self::is_anvil(state_id);
                    let damage_type = if is_anvil {
                        DamageType::FALLING_ANVIL
                    } else {
                        DamageType::FALLING_BLOCK
                    };
                    let victims = world.get_all_at_box(&entity.bounding_box.load());
                    for victim in victims {
                        if victim.get_entity().entity_id == entity.entity_id {
                            continue;
                        }
                        let _ = victim
                            .damage_with_context(
                                victim.as_ref(),
                                damage,
                                damage_type,
                                None,
                                Some(caller.as_ref()),
                                None,
                            )
                            .await;
                    }
                    if is_anvil {
                        let damage_steps = Self::fall_damage_steps(fall_distance);
                        let damage_chance = 0.05 + damage_steps as f32 * 0.05;
                        if rand::random::<f32>() < damage_chance {
                            if let Some(damaged_state) = Self::damaged_anvil_state(state_id) {
                                self.block_state_id
                                    .store(damaged_state.as_u16(), Ordering::Relaxed);
                                self.entity
                                    .data
                                    .store(i32::from(damaged_state.as_u16()), Ordering::Relaxed);
                            } else {
                                self.cancel_drop.store(true, Ordering::Relaxed);
                            }
                        }
                    }
                }
                entity.velocity.store(velo.multiply(0.7, -0.5, 0.7));
                let mut state_id = self.block_state_id();
                if let Some(concrete) =
                    ConcretePowderBlock::concrete_for_powder(state_id.to_block().id)
                {
                    if ConcretePowderBlock::should_harden(world.as_ref(), &position) {
                        state_id = concrete.default_state.id;
                        self.block_state_id
                            .store(state_id.as_u16(), Ordering::Relaxed);
                        self.entity
                            .data
                            .store(i32::from(state_id.as_u16()), Ordering::Relaxed);
                    }
                }
                let mut position = entity.block_pos.load();
                let mut replacing_state = world.get_block_state_async(&position).await;
                if !replacing_state.replaceable() {
                    if let Some(supporting) = entity.supporting_block_pos.load() {
                        if supporting.0.y + 1 > position.0.y {
                            let candidate =
                                BlockPos::new(position.0.x, supporting.0.y + 1, position.0.z);
                            let cand_state = world.get_block_state_async(&candidate).await;
                            if cand_state.replaceable() {
                                position = candidate;
                                replacing_state = cand_state;
                            }
                        }
                    } else {
                        let up_pos = position.up();
                        let up_state = world.get_block_state_async(&up_pos).await;
                        if up_state.replaceable() {
                            position = up_pos;
                            replacing_state = up_state;
                        }
                    }
                }
                let state = state_id.to_state();
                let block = state_id.to_block();
                let can_survive = world.block_registry.can_place_at(
                    Some(server),
                    Some(world.as_ref()),
                    world.as_ref(),
                    None,
                    block,
                    state,
                    &position,
                    None,
                    None,
                );
                if replacing_state.replaceable()
                    && can_survive
                    && !self.cancel_drop.load(Ordering::Relaxed)
                {
                    world
                        .set_block_state(&position, state_id, BlockFlags::NOTIFY_ALL)
                        .await;
                    if let Some(block_entity_data) =
                        self.block_entity_data.lock().await.as_ref().cloned()
                    {
                        world
                            .merge_block_entity_data(position, &block_entity_data)
                            .await;
                    }
                } else if self.drop_item.load(Ordering::Relaxed)
                    && !self.cancel_drop.load(Ordering::Relaxed)
                    && let Some(item) = Item::from_id(state_id.to_block().item_id)
                {
                    world.drop_stack(&position, ItemStack::new(1, item)).await;
                }
                entity.remove().await;
            } else if fallen > 0.0 {
                let distance = self.fall_distance.load(Ordering::Relaxed) + fallen;
                self.fall_distance.store(distance, Ordering::Relaxed);
            }

            entity.velocity.store(velo.multiply(0.98, 0.98, 0.98));

            if entity.velocity_dirty.swap(false, Ordering::SeqCst) {
                entity.send_pos_rot();
                entity.send_velocity();
            }
        })
    }

    fn init_data_tracker(&self) -> EntityBaseFuture<'_, ()> {
        Box::pin(async move {
            self.entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::falling_block::START_POS,
                    self.entity.block_pos.load(),
                )],
                None,
            );
        })
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn get_name(&self) -> TextComponent {
        self.entity
            .custom_name
            .load()
            .as_ref()
            .clone()
            .unwrap_or_else(|| {
                let block = self.block_state_id().to_block();
                TextComponent::translate(
                    pumpkin_data::translation::java::ENTITY_MINECRAFT_FALLING_BLOCK_TYPE,
                    [TextComponent::translate(
                        format!("block.minecraft.{}", block.name),
                        [],
                    )],
                )
            })
    }

    fn get_display_name(&self) -> EntityBaseFuture<'_, TextComponent> {
        Box::pin(async move {
            let entity = &self.entity;
            let name = self.get_name();
            name.clone()
                .hover_event(HoverEvent::show_entity(
                    entity.entity_uuid.to_string(),
                    entity.entity_type.resource_name.into(),
                    Some(name),
                ))
                .insertion(entity.entity_uuid.to_string())
        })
    }

    fn as_nbt_storage(&self) -> &dyn NBTStorage {
        self
    }

    fn damage<'a>(
        &'a self,
        _caller: &'a dyn EntityBase,
        _amount: f32,
        _damage_type: DamageType,
    ) -> EntityBaseFuture<'a, bool> {
        Box::pin(async move { false })
    }

    fn get_gravity(&self) -> f64 {
        0.04
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_state_nbt_round_trips_default_state() {
        let state_id = Block::STONE.default_state.id;
        let mut root = NbtCompound::new();
        root.put(
            "BlockState",
            NbtTag::Compound(FallingEntity::block_state_to_nbt(state_id)),
        );

        assert_eq!(FallingEntity::block_state_from_nbt(&root), Some(state_id));
        assert_eq!(
            root.get_compound("BlockState")
                .and_then(|state| state.get_string("Name")),
            Some("minecraft:stone")
        );
    }

    #[test]
    fn block_state_nbt_round_trips_properties() {
        let entry = PaletteEntry::with_properties(
            "minecraft:oak_log".to_string(),
            vec![("axis".to_string(), "x".to_string())],
        );
        let state_id = BlockStateResolver::resolve_simple(&entry).unwrap().id;
        let mut root = NbtCompound::new();
        root.put(
            "BlockState",
            NbtTag::Compound(FallingEntity::block_state_to_nbt(state_id)),
        );

        assert_eq!(FallingEntity::block_state_from_nbt(&root), Some(state_id));
        assert_eq!(
            root.get_compound("BlockState")
                .and_then(|state| state.get_compound("Properties"))
                .and_then(|properties| properties.get_string("axis")),
            Some("x")
        );
    }

    #[test]
    fn missing_block_state_is_not_silently_interpreted_as_air() {
        assert_eq!(
            FallingEntity::block_state_from_nbt(&NbtCompound::new()),
            None
        );
    }

    #[test]
    fn malformed_block_state_property_uses_the_named_blocks_default() {
        let mut properties = NbtCompound::new();
        properties.put_string("axis", "invalid".to_string());
        let mut state = NbtCompound::new();
        state.put_string("Name", "minecraft:oak_log".to_string());
        state.put("Properties", NbtTag::Compound(properties));
        let mut root = NbtCompound::new();
        root.put("BlockState", NbtTag::Compound(state));

        assert_eq!(
            FallingEntity::block_state_from_nbt(&root),
            Some(Block::OAK_LOG.default_state.id)
        );
    }

    #[test]
    fn impact_damage_uses_vanilla_rounding_and_cap() {
        assert_eq!(FallingEntity::calculate_impact_damage(1.0, 2.0, 40), None);
        assert_eq!(
            FallingEntity::calculate_impact_damage(1.01, 2.0, 40),
            Some(2.0)
        );
        assert_eq!(
            FallingEntity::calculate_impact_damage(4.2, 2.5, 40),
            Some(10.0)
        );
        assert_eq!(
            FallingEntity::calculate_impact_damage(100.0, 2.0, 23),
            Some(23.0)
        );
    }

    #[test]
    fn timeout_uses_vanilla_age_and_dimension_boundaries() {
        let bottom = -64;
        let top = 319;
        assert!(!FallingEntity::should_time_out(100, bottom, bottom, top));
        assert!(FallingEntity::should_time_out(101, bottom, bottom, top));
        assert!(!FallingEntity::should_time_out(
            101,
            bottom + 1,
            bottom,
            top
        ));
        assert!(!FallingEntity::should_time_out(101, top, bottom, top));
        assert!(FallingEntity::should_time_out(101, top + 1, bottom, top));
        assert!(!FallingEntity::should_time_out(600, 100, bottom, top));
        assert!(FallingEntity::should_time_out(601, 100, bottom, top));
    }

    #[test]
    fn anvil_damage_progression_preserves_facing() {
        for (source, successor, facing) in [
            ("minecraft:anvil", "minecraft:chipped_anvil", "north"),
            ("minecraft:chipped_anvil", "minecraft:damaged_anvil", "east"),
        ] {
            let source = BlockStateResolver::resolve_simple(&PaletteEntry::with_properties(
                source.to_string(),
                vec![("facing".to_string(), facing.to_string())],
            ))
            .unwrap()
            .id;
            let expected = BlockStateResolver::resolve_simple(&PaletteEntry::with_properties(
                successor.to_string(),
                vec![("facing".to_string(), facing.to_string())],
            ))
            .unwrap()
            .id;

            assert_eq!(FallingEntity::damaged_anvil_state(source), Some(expected));
        }
        assert_eq!(
            FallingEntity::damaged_anvil_state(Block::DAMAGED_ANVIL.default_state.id),
            None
        );
        assert!(!FallingEntity::is_anvil(Block::SAND.default_state.id));
    }
}
