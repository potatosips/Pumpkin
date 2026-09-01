use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use pumpkin_data::{
    data_component_impl::EquipmentSlot,
    entity::EntityType,
    item::Item,
    item_stack::ItemStack,
    sound::{Sound, SoundCategory},
    tag::{self, Taggable},
    tracked_data,
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::java::client::play::Metadata;
use pumpkin_world::inventory::{Clearable, Inventory, InventoryFuture, SimpleInventory};

use crate::entity::{EntityBase, mob::MobEntity, player::Player};

pub struct ChestedHorseData {
    has_chest: AtomicBool,
    pub inventory: Arc<SimpleInventory>,
}

pub struct MountInventory {
    entity: Arc<dyn EntityBase>,
    cargo: Option<Arc<SimpleInventory>>,
    cargo_slots: usize,
}

#[derive(Clone, Copy)]
pub enum MountBodySlotKind {
    None,
    HorseArmor,
    LlamaDecor,
}

fn accepts_body_stack(kind: MountBodySlotKind, stack: &ItemStack) -> bool {
    stack.is_empty()
        || match kind {
            MountBodySlotKind::None => false,
            MountBodySlotKind::HorseArmor => stack.item.has_tag(&tag::Item::C_ARMORS_HORSE),
            MountBodySlotKind::LlamaDecor => stack.item.has_tag(&tag::Item::MINECRAFT_WOOL_CARPETS),
        }
}

pub async fn sanitize_body_equipment(mob: &MobEntity, kind: MountBodySlotKind) {
    let mut equipment = mob.living_entity.entity_equipment.lock().await;
    let body = equipment.get(&EquipmentSlot::BODY);
    if !accepts_body_stack(kind, &body) {
        equipment.equipment.remove(&EquipmentSlot::BODY);
    }
}

impl MountInventory {
    pub fn new(
        entity: Arc<dyn EntityBase>,
        cargo: Option<Arc<SimpleInventory>>,
        cargo_slots: usize,
    ) -> Self {
        Self {
            entity,
            cargo,
            cargo_slots: cargo_slots.min(15),
        }
    }

    async fn equipment_stack(&self, slot: &EquipmentSlot) -> ItemStack {
        match self.entity.get_living_entity() {
            Some(living) => living.entity_equipment.lock().await.get(slot),
            None => ItemStack::EMPTY.clone(),
        }
    }

    async fn set_equipment_stack(&self, slot: &EquipmentSlot, stack: ItemStack) {
        let Some(living) = self.entity.get_living_entity() else {
            return;
        };
        living
            .entity_equipment
            .lock()
            .await
            .put(slot, stack.clone());
        living.send_equipment_changes(&[(slot.clone(), stack)]);
    }
}

impl Clearable for MountInventory {
    fn clear(&self) -> InventoryFuture<'_, ()> {
        Box::pin(async move {
            if let Some(mob) = self.entity.get_mob() {
                mob.set_saddle_stack(ItemStack::EMPTY.clone()).await;
            }
            self.set_equipment_stack(&EquipmentSlot::BODY, ItemStack::EMPTY.clone())
                .await;
            if let Some(cargo) = &self.cargo {
                cargo.clear().await;
            }
        })
    }
}

impl Inventory for MountInventory {
    fn size(&self) -> usize {
        2 + self.cargo_slots
    }

    fn is_empty(&self) -> InventoryFuture<'_, bool> {
        Box::pin(async move {
            self.get_stack(0).await.is_empty()
                && self.get_stack(1).await.is_empty()
                && match &self.cargo {
                    Some(cargo) => cargo.is_empty().await,
                    None => true,
                }
        })
    }

    fn get_stack(&self, slot: usize) -> InventoryFuture<'_, ItemStack> {
        Box::pin(async move {
            match slot {
                0 => match self.entity.get_mob() {
                    Some(mob) => mob.saddle_stack().await,
                    None => ItemStack::EMPTY.clone(),
                },
                1 => self.equipment_stack(&EquipmentSlot::BODY).await,
                cargo_slot if cargo_slot < self.size() => match &self.cargo {
                    Some(cargo) => cargo.get_stack(cargo_slot - 2).await,
                    None => ItemStack::EMPTY.clone(),
                },
                _ => ItemStack::EMPTY.clone(),
            }
        })
    }

    fn remove_stack(&self, slot: usize) -> InventoryFuture<'_, ItemStack> {
        Box::pin(async move {
            let stack = self.get_stack(slot).await;
            self.set_stack(slot, ItemStack::EMPTY.clone()).await;
            stack
        })
    }

    fn remove_stack_specific(&self, slot: usize, amount: u8) -> InventoryFuture<'_, ItemStack> {
        Box::pin(async move {
            let mut stack = self.get_stack(slot).await;
            let removed = stack.split(amount);
            self.set_stack(slot, stack).await;
            removed
        })
    }

    fn set_stack(&self, slot: usize, stack: ItemStack) -> InventoryFuture<'_, ()> {
        Box::pin(async move {
            if !stack.is_empty() && !self.is_valid_slot_for(slot, &stack) {
                return;
            }
            match slot {
                0 => {
                    if let Some(mob) = self.entity.get_mob() {
                        mob.set_saddle_stack(stack).await;
                    }
                }
                1 => self.set_equipment_stack(&EquipmentSlot::BODY, stack).await,
                cargo_slot if cargo_slot < self.size() => {
                    if let Some(cargo) = &self.cargo {
                        cargo.set_stack(cargo_slot - 2, stack).await;
                    }
                }
                _ => {}
            }
        })
    }

    fn is_valid_slot_for(&self, slot: usize, stack: &ItemStack) -> bool {
        match slot {
            0 => {
                self.entity.get_entity().entity_type != &EntityType::LLAMA
                    && stack.item == &Item::SADDLE
            }
            1 if self.entity.get_entity().entity_type == &EntityType::HORSE => {
                stack.item.has_tag(&tag::Item::C_ARMORS_HORSE)
            }
            1 if self.entity.get_entity().entity_type == &EntityType::LLAMA => {
                stack.item.has_tag(&tag::Item::MINECRAFT_WOOL_CARPETS)
            }
            1 => false,
            cargo_slot => cargo_slot < self.size() && cargo_slot >= 2,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Default for ChestedHorseData {
    fn default() -> Self {
        Self {
            has_chest: AtomicBool::new(false),
            // Donkeys and mules expose all 15 slots. Llamas expose only the
            // first strength * 3 slots, but keeping the Vanilla maximum here
            // lets strength changes preserve the backing inventory.
            inventory: Arc::new(SimpleInventory::new(15)),
        }
    }
}

impl ChestedHorseData {
    pub fn has_chest(&self) -> bool {
        self.has_chest.load(Ordering::Relaxed)
    }

    pub fn set_has_chest(&self, entity: &dyn EntityBase, has_chest: bool) {
        self.has_chest.store(has_chest, Ordering::Relaxed);
        entity.get_entity().send_meta_data(
            &[Metadata::new(
                tracked_data::abstract_chested_horse::DATA_ID_CHEST,
                has_chest,
            )],
            None,
        );
    }

    pub async fn write_nbt(&self, nbt: &mut NbtCompound) {
        let has_chest = self.has_chest();
        nbt.put_bool("ChestedHorse", has_chest);
        if has_chest {
            self.inventory.write_inventory_nbt(nbt, false).await;
        }
    }

    pub async fn read_nbt(&self, entity: &dyn EntityBase, nbt: &NbtCompound) {
        let has_chest = nbt.get_bool("ChestedHorse").unwrap_or(false);
        self.read_inventory_nbt(nbt, has_chest).await;
        self.set_has_chest(entity, has_chest);
    }

    async fn read_inventory_nbt(&self, nbt: &NbtCompound, has_chest: bool) {
        self.inventory.clear().await;
        if has_chest {
            let mut stacks = self.inventory.stacks.write().await;
            self.inventory.read_data(nbt, &mut stacks);
        }
    }

    pub async fn try_attach(
        &self,
        entity: &dyn EntityBase,
        player: &Player,
        stack: &mut ItemStack,
        sound: Sound,
    ) -> bool {
        if self.has_chest() || stack.item != &Item::CHEST {
            return false;
        }

        self.set_has_chest(entity, true);
        stack.decrement_unless_creative(player.gamemode.load(), 1);
        let entity = entity.get_entity();
        entity
            .world
            .load()
            .play_sound(sound, SoundCategory::Neutral, &entity.pos.load());
        true
    }

    async fn take_chest_and_cargo(&self) -> Vec<ItemStack> {
        let mut drops = Vec::new();
        if self.has_chest.swap(false, Ordering::Relaxed) {
            drops.push(ItemStack::new(1, &Item::CHEST));
        }
        let mut cargo = self.inventory.stacks.write().await;
        drops.extend(
            cargo
                .iter_mut()
                .map(|stack| std::mem::replace(stack, ItemStack::EMPTY.clone()))
                .filter(|stack| !stack.is_empty()),
        );
        drops
    }
}

pub async fn drop_mount_inventory_on_death(mob: &MobEntity, chested: Option<&ChestedHorseData>) {
    let world = mob.living_entity.entity.world.load();
    let should_drop = world.level_info.load().game_rules.mob_drops;
    let position = mob.living_entity.entity.block_pos.load();

    let equipment = {
        let mut equipment = mob.living_entity.entity_equipment.lock().await;
        [EquipmentSlot::SADDLE, EquipmentSlot::BODY]
            .into_iter()
            .filter_map(|slot| equipment.equipment.remove(&slot))
            .filter(|stack| !stack.is_empty())
            .collect::<Vec<_>>()
    };

    let mut drops = equipment;
    if let Some(chested) = chested {
        drops.extend(chested.take_chest_and_cargo().await);
    }

    if should_drop {
        for stack in drops {
            world.drop_stack(&position, stack).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn chested_horse_inventory_nbt_round_trip() {
        let source = ChestedHorseData::default();
        source.has_chest.store(true, Ordering::Relaxed);
        source
            .inventory
            .set_stack(14, ItemStack::new(7, &Item::DIAMOND))
            .await;

        let mut nbt = NbtCompound::new();
        source.write_nbt(&mut nbt).await;

        let restored = ChestedHorseData::default();
        restored.read_inventory_nbt(&nbt, true).await;
        let stack = restored.inventory.get_stack(14).await;
        assert_eq!(restored.inventory.size(), 15);
        assert!(stack.item == &Item::DIAMOND);
        assert_eq!(stack.item_count, 7);
    }

    #[tokio::test]
    async fn unchested_state_ignores_stale_items() {
        let data = ChestedHorseData::default();
        data.inventory
            .set_stack(0, ItemStack::new(1, &Item::DIAMOND))
            .await;
        let mut nbt = NbtCompound::new();
        nbt.put_bool("ChestedHorse", false);
        data.read_inventory_nbt(&nbt, false).await;
        assert!(data.inventory.is_empty().await);
    }

    #[tokio::test]
    async fn death_drain_returns_chest_and_cargo_once() {
        let data = ChestedHorseData::default();
        data.has_chest.store(true, Ordering::Relaxed);
        data.inventory
            .set_stack(3, ItemStack::new(4, &Item::DIAMOND))
            .await;

        let drops = data.take_chest_and_cargo().await;
        assert_eq!(drops.len(), 2);
        assert!(drops.iter().any(|stack| stack.item == &Item::CHEST));
        assert!(
            drops
                .iter()
                .any(|stack| stack.item == &Item::DIAMOND && stack.item_count == 4)
        );
        assert!(data.take_chest_and_cargo().await.is_empty());
        assert!(data.inventory.is_empty().await);
    }

    #[test]
    fn restored_body_equipment_is_species_restricted() {
        let horse_armor = ItemStack::new(1, &Item::IRON_HORSE_ARMOR);
        let carpet = ItemStack::new(1, &Item::RED_CARPET);
        let diamond = ItemStack::new(1, &Item::DIAMOND);

        assert!(accepts_body_stack(
            MountBodySlotKind::HorseArmor,
            &horse_armor
        ));
        assert!(!accepts_body_stack(MountBodySlotKind::HorseArmor, &carpet));
        assert!(accepts_body_stack(MountBodySlotKind::LlamaDecor, &carpet));
        assert!(!accepts_body_stack(
            MountBodySlotKind::LlamaDecor,
            &horse_armor
        ));
        assert!(!accepts_body_stack(MountBodySlotKind::None, &diamond));
        assert!(accepts_body_stack(
            MountBodySlotKind::None,
            &ItemStack::EMPTY
        ));
    }
}
