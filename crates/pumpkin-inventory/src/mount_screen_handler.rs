use std::{
    any::Any,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};

use pumpkin_data::item_stack::ItemStack;
use pumpkin_world::inventory::Inventory;

use crate::{
    player::player_inventory::PlayerInventory,
    screen_handler::{
        InventoryPlayer, ItemStackFuture, ScreenHandler, ScreenHandlerBehaviour,
        ScreenHandlerFuture,
    },
    slot::{BoxFuture, Slot},
};

/// The Java horse inventory uses a dedicated open packet, so it has no
/// registry-backed `WindowType`. Its first two slots are equipment and all
/// remaining slots are cargo.
pub struct MountScreenHandler {
    pub mount_inventory: Arc<dyn Inventory>,
    behaviour: ScreenHandlerBehaviour,
}

impl MountScreenHandler {
    pub async fn new(
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        mount_inventory: Arc<dyn Inventory>,
    ) -> Self {
        let mut behaviour = ScreenHandlerBehaviour::new(sync_id, None);
        behaviour.container_slots = mount_inventory.size();
        let mut handler = Self {
            mount_inventory: mount_inventory.clone(),
            behaviour,
        };

        mount_inventory.on_open().await;
        for index in 0..mount_inventory.size() {
            handler.add_slot(Arc::new(MountSlot::new(mount_inventory.clone(), index)));
        }
        let player_inventory: Arc<dyn Inventory> = player_inventory.clone();
        handler.add_player_slots(&player_inventory);
        handler
    }
}

impl ScreenHandler for MountScreenHandler {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn get_behaviour(&self) -> &ScreenHandlerBehaviour {
        &self.behaviour
    }

    fn get_behaviour_mut(&mut self) -> &mut ScreenHandlerBehaviour {
        &mut self.behaviour
    }

    fn on_closed<'a>(&'a mut self, player: &'a dyn InventoryPlayer) -> ScreenHandlerFuture<'a, ()> {
        Box::pin(async move {
            self.default_on_closed(player).await;
            self.mount_inventory.on_close().await;
        })
    }

    fn quick_move<'a>(
        &'a mut self,
        _player: &'a dyn InventoryPlayer,
        slot_index: i32,
    ) -> ItemStackFuture<'a> {
        Box::pin(async move {
            if slot_index < 0 || slot_index as usize >= self.behaviour.slots.len() {
                return ItemStack::EMPTY.clone();
            }
            let mount_slots = self.mount_inventory.size() as i32;
            let slot = self.behaviour.slots[slot_index as usize].clone();
            if !slot.has_stack().await {
                return ItemStack::EMPTY.clone();
            }

            let mut moving = slot.get_stack().await;
            let original = moving.clone();
            let moved = if slot_index < mount_slots {
                self.insert_item(
                    &mut moving,
                    mount_slots,
                    self.behaviour.slots.len() as i32,
                    true,
                )
                .await
            } else {
                let mut moved = false;
                if self.behaviour.slots[1].can_insert(&moving).await
                    && !self.behaviour.slots[1].has_stack().await
                {
                    moved = self.insert_item(&mut moving, 1, 2, false).await;
                }
                if !moved
                    && self.behaviour.slots[0].can_insert(&moving).await
                    && !self.behaviour.slots[0].has_stack().await
                {
                    moved = self.insert_item(&mut moving, 0, 1, false).await;
                }
                if !moved && mount_slots > 2 {
                    moved = self.insert_item(&mut moving, 2, mount_slots, false).await;
                }
                if !moved && slot_index < mount_slots + 27 {
                    moved = self
                        .insert_item(&mut moving, mount_slots + 27, mount_slots + 36, false)
                        .await;
                } else if !moved {
                    moved = self
                        .insert_item(&mut moving, mount_slots, mount_slots + 27, false)
                        .await;
                }
                moved
            };

            if !moved {
                return ItemStack::EMPTY.clone();
            }
            slot.set_stack(moving).await;
            original
        })
    }
}

struct MountSlot {
    inventory: Arc<dyn Inventory>,
    index: usize,
    id: AtomicU8,
}

impl MountSlot {
    fn new(inventory: Arc<dyn Inventory>, index: usize) -> Self {
        Self {
            inventory,
            index,
            id: AtomicU8::new(0),
        }
    }
}

impl Slot for MountSlot {
    fn get_inventory(&self) -> Arc<dyn Inventory> {
        self.inventory.clone()
    }

    fn get_index(&self) -> usize {
        self.index
    }

    fn set_id(&self, id: usize) {
        self.id.store(id as u8, Ordering::Relaxed);
    }

    fn can_insert<'a>(&'a self, stack: &'a ItemStack) -> BoxFuture<'a, bool> {
        Box::pin(async move { self.inventory.is_valid_slot_for(self.index, stack) })
    }

    fn get_max_item_count(&self) -> BoxFuture<'_, u8> {
        Box::pin(async move {
            if self.index < 2 {
                1
            } else {
                self.inventory.get_max_count_per_stack()
            }
        })
    }

    fn mark_dirty(&self) -> BoxFuture<'_, ()> {
        Box::pin(async move { self.inventory.mark_dirty() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity_equipment::EntityEquipment;
    use crate::screen_handler::tests::DummyPlayer;
    use pumpkin_data::data_component_impl::EquipmentSlot;
    use pumpkin_data::item::Item;
    use pumpkin_world::inventory::{Clearable, InventoryFuture, SimpleInventory};
    use std::collections::HashMap;
    use tokio::sync::Mutex;

    struct TestMountInventory {
        inner: Arc<SimpleInventory>,
        size: usize,
    }

    impl TestMountInventory {
        fn new(size: usize) -> Self {
            Self {
                inner: Arc::new(SimpleInventory::new(size)),
                size,
            }
        }
    }

    impl Clearable for TestMountInventory {
        fn clear(&self) -> InventoryFuture<'_, ()> {
            self.inner.clear()
        }
    }

    impl Inventory for TestMountInventory {
        fn size(&self) -> usize {
            self.size
        }
        fn is_empty(&self) -> InventoryFuture<'_, bool> {
            self.inner.is_empty()
        }
        fn get_stack(&self, slot: usize) -> InventoryFuture<'_, ItemStack> {
            self.inner.get_stack(slot)
        }
        fn remove_stack(&self, slot: usize) -> InventoryFuture<'_, ItemStack> {
            self.inner.remove_stack(slot)
        }
        fn remove_stack_specific(&self, slot: usize, amount: u8) -> InventoryFuture<'_, ItemStack> {
            self.inner.remove_stack_specific(slot, amount)
        }
        fn set_stack(&self, slot: usize, stack: ItemStack) -> InventoryFuture<'_, ()> {
            self.inner.set_stack(slot, stack)
        }
        fn is_valid_slot_for(&self, slot: usize, stack: &ItemStack) -> bool {
            match slot {
                0 => stack.item == &Item::SADDLE,
                1 => false,
                _ => slot < self.size,
            }
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[tokio::test]
    async fn mount_screen_has_equipment_cargo_and_player_slots() {
        let equipment = Arc::new(Mutex::new(EntityEquipment::new()));
        let mut equipment_slots = HashMap::new();
        equipment_slots.insert(40, EquipmentSlot::OFF_HAND);
        let player_inventory = Arc::new(PlayerInventory::new(equipment, Arc::new(equipment_slots)));
        let mount_inventory: Arc<dyn Inventory> = Arc::new(SimpleInventory::new(17));
        let handler = MountScreenHandler::new(7, &player_inventory, mount_inventory).await;

        assert_eq!(handler.sync_id(), 7);
        assert_eq!(handler.get_behaviour().container_slots, 17);
        assert_eq!(handler.get_behaviour().slots.len(), 17 + 36);
        assert_eq!(
            handler.get_behaviour().slots[0].get_max_item_count().await,
            1
        );
        assert_eq!(
            handler.get_behaviour().slots[1].get_max_item_count().await,
            1
        );
    }

    #[tokio::test]
    async fn quick_move_prioritizes_equipment_then_falls_back_to_player_sections() {
        let player = DummyPlayer::new(true);
        let mount_inventory: Arc<dyn Inventory> = Arc::new(TestMountInventory::new(2));
        let mut handler =
            MountScreenHandler::new(3, &player.inventory, mount_inventory.clone()).await;

        // The first player-main slot follows the two mount slots.
        handler.get_behaviour().slots[2]
            .set_stack(ItemStack::new(1, &Item::SADDLE))
            .await;
        handler.quick_move(&player, 2).await;
        assert!(mount_inventory.get_stack(0).await.item == &Item::SADDLE);

        handler.get_behaviour().slots[3]
            .set_stack(ItemStack::new(5, &Item::DIAMOND))
            .await;
        handler.quick_move(&player, 3).await;
        assert!(mount_inventory.get_stack(1).await.is_empty());
        let mut found_diamond = false;
        for slot in 0..9 {
            if player.inventory.get_stack(slot).await.item == &Item::DIAMOND {
                found_diamond = true;
                break;
            }
        }
        assert!(found_diamond);
    }
}
