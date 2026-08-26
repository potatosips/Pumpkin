use std::{
    any::Any,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};

use pumpkin_data::{item_stack::ItemStack, screen::WindowType};
use pumpkin_world::inventory::Inventory;

use crate::{
    player::player_inventory::PlayerInventory,
    screen_handler::{
        InventoryPlayer, ItemStackFuture, ScreenHandler, ScreenHandlerBehaviour,
        ScreenHandlerFuture,
    },
    slot::{BoxFuture, Slot},
};

pub struct BeaconPaymentSlot {
    pub inventory: Arc<dyn Inventory>,
    pub index: usize,
    pub id: AtomicU8,
}

impl BeaconPaymentSlot {
    pub fn new(inventory: Arc<dyn Inventory>, index: usize) -> Self {
        Self {
            inventory,
            index,
            id: AtomicU8::new(0),
        }
    }
}

impl Slot for BeaconPaymentSlot {
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
        Box::pin(async move {
            stack.item == &pumpkin_data::item::Item::NETHERITE_INGOT
                || stack.item == &pumpkin_data::item::Item::EMERALD
                || stack.item == &pumpkin_data::item::Item::DIAMOND
                || stack.item == &pumpkin_data::item::Item::GOLD_INGOT
                || stack.item == &pumpkin_data::item::Item::IRON_INGOT
        })
    }

    fn get_max_item_count_for_stack<'a>(&'a self, _stack: &'a ItemStack) -> BoxFuture<'a, u8> {
        Box::pin(async move { 1 })
    }

    fn get_max_item_count(&self) -> BoxFuture<'_, u8> {
        Box::pin(async move { 1 })
    }

    fn mark_dirty(&self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            self.inventory.mark_dirty();
        })
    }
}

/// Creates a beacon container screen handler.
///
/// Beacons feature a single payment slot and a specialized UI for selecting status effects.
pub async fn create_beacon_handler(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
) -> BeaconScreenHandler {
    BeaconScreenHandler::new(sync_id, player_inventory, inventory).await
}

/// Screen handler specifically for Beacon blocks.
pub struct BeaconScreenHandler {
    /// The beacon's inventory (contains exactly 1 slot for payment).
    pub inventory: Arc<dyn Inventory>,
    /// Core screen handler behavior (slots, sync ID, listeners).
    behaviour: ScreenHandlerBehaviour,
}

impl BeaconScreenHandler {
    /// Creates a new beacon screen handler.
    async fn new(
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        inventory: Arc<dyn Inventory>,
    ) -> Self {
        let mut handler = Self {
            inventory: inventory.clone(),
            behaviour: ScreenHandlerBehaviour::new(sync_id, Some(WindowType::Beacon)),
        };

        inventory.on_open().await;

        // Add the single payment slot for the beacon (slot 0)
        handler.add_slot(Arc::new(BeaconPaymentSlot::new(
            handler.inventory.clone(),
            0,
        )));

        // Add the player's inventory slots (27 slots + 9 hotbar)
        let player_inventory_arc: Arc<dyn Inventory> = player_inventory.clone();
        handler.add_player_slots(&player_inventory_arc);

        handler
    }
}

impl ScreenHandler for BeaconScreenHandler {
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
            self.inventory.on_close().await;
        })
    }

    /// Quick move logic specifically for the beacon UI.
    ///
    /// - From beacon payment slot (0): Move to player inventory
    /// - From player inventory (1+): Move to beacon payment slot if valid payment item
    fn quick_move<'a>(
        &'a mut self,
        _player: &'a dyn InventoryPlayer,
        slot_index: i32,
    ) -> ItemStackFuture<'a> {
        Box::pin(async move {
            let mut stack_left = ItemStack::EMPTY.clone();
            let slot = self.get_behaviour().slots[slot_index as usize].clone();

            if slot.has_stack().await {
                let mut slot_stack = slot.get_stack().await;
                stack_left = slot_stack.clone();

                if slot_index == 0 {
                    // Move from the single beacon slot to the player inventory (slots 1 to end)
                    if !self
                        .insert_item(
                            &mut slot_stack,
                            1,
                            self.get_behaviour().slots.len() as i32,
                            true,
                        )
                        .await
                    {
                        return ItemStack::EMPTY.clone();
                    }
                } else {
                    // Move from player inventory into the beacon payment slot (slot 0)
                    let is_payment = slot_stack.item == &pumpkin_data::item::Item::NETHERITE_INGOT
                        || slot_stack.item == &pumpkin_data::item::Item::EMERALD
                        || slot_stack.item == &pumpkin_data::item::Item::DIAMOND
                        || slot_stack.item == &pumpkin_data::item::Item::GOLD_INGOT
                        || slot_stack.item == &pumpkin_data::item::Item::IRON_INGOT;
                    if is_payment {
                        if !self.insert_item(&mut slot_stack, 0, 1, false).await {
                            return ItemStack::EMPTY.clone();
                        }
                    } else {
                        return ItemStack::EMPTY.clone();
                    }
                }

                if slot_stack.is_empty() {
                    slot.set_stack(ItemStack::EMPTY.clone()).await;
                } else {
                    slot.set_stack(slot_stack).await;
                }
            }

            stack_left
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::item::Item;
    use pumpkin_world::inventory::SimpleInventory;

    #[tokio::test]
    async fn beacon_payment_slot_filter() {
        let inv = Arc::new(SimpleInventory::new(1));
        let slot = BeaconPaymentSlot::new(inv.clone(), 0);

        let diamond = ItemStack::new(1, &Item::DIAMOND);
        let emerald = ItemStack::new(1, &Item::EMERALD);
        let netherite = ItemStack::new(1, &Item::NETHERITE_INGOT);
        let iron = ItemStack::new(1, &Item::IRON_INGOT);
        let gold = ItemStack::new(1, &Item::GOLD_INGOT);
        let dirt = ItemStack::new(1, &Item::DIRT);

        assert!(slot.can_insert(&diamond).await);
        assert!(slot.can_insert(&emerald).await);
        assert!(slot.can_insert(&netherite).await);
        assert!(slot.can_insert(&iron).await);
        assert!(slot.can_insert(&gold).await);
        assert!(!slot.can_insert(&dirt).await);
        assert_eq!(slot.get_max_item_count().await, 1);
    }
}
