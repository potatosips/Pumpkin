//! Generic container screen handler.
//!
//! This module provides a generic screen handler for simple containers like:
//! - Chests (single, double, ender chest)
//! - Hoppers
//! - Dispensers/Droppers
//! - Barrels
//!
//! These containers have a simple grid layout with no special behaviors
//! (no smelting, no crafting, just item storage).

use std::{any::Any, sync::Arc};

use pumpkin_data::{item_stack::ItemStack, screen::WindowType};
use pumpkin_world::inventory::Inventory;

use crate::{
    player::player_inventory::PlayerInventory,
    screen_handler::{
        InventoryPlayer, ItemStackFuture, ScreenHandler, ScreenHandlerBehaviour,
        ScreenHandlerFuture,
    },
    slot::NormalSlot,
};

/// Creates a generic 9x3 container (single chest).
///
/// Used for single chests, ender chests, and similar containers.
pub async fn create_generic_9x3(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
) -> GenericContainerScreenHandler {
    GenericContainerScreenHandler::new(
        WindowType::Generic9x3,
        sync_id,
        player_inventory,
        inventory,
        3,
        9,
    )
    .await
}

/// Creates a generic 9x6 container (double chest).
///
/// Used for double chests and similar large containers.
pub async fn create_generic_9x6(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
) -> GenericContainerScreenHandler {
    GenericContainerScreenHandler::new(
        WindowType::Generic9x6,
        sync_id,
        player_inventory,
        inventory,
        6,
        9,
    )
    .await
}

/// Creates a generic 3x3 container.
///
/// Used for dispensers, droppers, and similar containers.
pub async fn create_generic_3x3(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
) -> GenericContainerScreenHandler {
    GenericContainerScreenHandler::new(
        WindowType::Generic3x3,
        sync_id,
        player_inventory,
        inventory,
        3,
        3,
    )
    .await
}

/// Creates a crafter container (9 slots, 3x3 layout).
pub async fn create_crafter_3x3(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
) -> GenericContainerScreenHandler {
    GenericContainerScreenHandler::new(
        WindowType::Crafter3x3,
        sync_id,
        player_inventory,
        inventory,
        3,
        3,
    )
    .await
}

/// Creates a hopper container (5 slots).
///
/// Hoppers have a single row of 5 slots.
pub async fn create_hopper(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
) -> GenericContainerScreenHandler {
    GenericContainerScreenHandler::new(
        WindowType::Hopper,
        sync_id,
        player_inventory,
        inventory,
        1,
        5,
    )
    .await
}

/// Generic container screen handler.
///
/// Handles simple grid-based containers without special behaviors.
/// The container grid is followed by the player's inventory (27 slots + 9 hotbar).
pub struct GenericContainerScreenHandler {
    /// The container's inventory.
    pub inventory: Arc<dyn Inventory>,
    /// Number of rows in the container grid.
    pub rows: u8,
    /// Number of columns in the container grid.
    pub columns: u8,
    /// Core screen handler behavior (slots, sync ID, listeners).
    behaviour: ScreenHandlerBehaviour,
}

impl GenericContainerScreenHandler {
    /// Creates a new generic container screen handler.
    ///
    /// # Arguments
    /// - `screen_type` - The window type for this container
    /// - `sync_id` - The sync ID for client-server matching
    /// - `player_inventory` - The player's inventory
    /// - `inventory` - The container's inventory
    /// - `rows` - Number of rows in the container
    /// - `columns` - Number of columns in the container
    async fn new(
        screen_type: WindowType,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        inventory: Arc<dyn Inventory>,
        rows: u8,
        columns: u8,
    ) -> Self {
        let mut handler = Self {
            inventory: inventory.clone(),
            rows,
            columns,
            behaviour: ScreenHandlerBehaviour::new(sync_id, Some(screen_type)),
        };

        // TODO: Add player entity as a parameter
        inventory.on_open().await;

        handler.add_inventory_slots();
        let player_inventory: Arc<dyn Inventory> = player_inventory.clone();
        handler.add_player_slots(&player_inventory);

        handler
    }

    /// Adds slots for the container's inventory grid.
    fn add_inventory_slots(&mut self) {
        for i in 0..self.rows {
            for j in 0..self.columns {
                self.add_slot(Arc::new(NormalSlot::new(
                    self.inventory.clone(),
                    (j + i * self.columns) as usize,
                )));
            }
        }
    }
}

impl ScreenHandler for GenericContainerScreenHandler {
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

    /// Quick move logic for generic containers.
    ///
    /// - From container: Move to player inventory (end first)
    /// - From player inventory: Move to container (start first)
    fn quick_move<'a>(
        &'a mut self,
        _player: &'a dyn InventoryPlayer,
        slot_index: i32,
    ) -> ItemStackFuture<'a> {
        Box::pin(async move {
            let mut stack_left = ItemStack::EMPTY.clone();
            let container_slots: i32 = i32::from(self.rows) * i32::from(self.columns);
            let slot = self.get_behaviour().slots[slot_index as usize].clone();

            if slot.has_stack().await {
                let mut slot_stack = slot.get_stack().await;
                stack_left = slot_stack.clone();

                if slot_index < container_slots {
                    // Move from inventory to player area (end)
                    if !self
                        .insert_item(
                            &mut slot_stack,
                            container_slots,
                            self.get_behaviour().slots.len() as i32,
                            true,
                        )
                        .await
                    {
                        return ItemStack::EMPTY.clone();
                    }
                } else if !self
                    .insert_item(&mut slot_stack, 0, container_slots, false)
                    .await
                {
                    // Move from player area to inventory (start)
                    return ItemStack::EMPTY.clone();
                }

                // Check the resulting state of the slot stack after insert_item
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
    use crate::entity_equipment::EntityEquipment;
    use crate::screen_handler::PlayerFuture;
    use pumpkin_data::data_component_impl::EquipmentSlot;
    use pumpkin_data::item::Item;
    use pumpkin_data::statistic::StatisticCategory;
    use pumpkin_protocol::java::client::play::{
        CSetContainerContent, CSetContainerProperty, CSetContainerSlot, CSetCursorItem,
        CSetPlayerInventory, CSetSelectedSlot,
    };
    use pumpkin_world::inventory::SimpleInventory;
    use std::collections::HashMap;
    use tokio::sync::Mutex;

    struct TestPlayer {
        inventory: Arc<PlayerInventory>,
    }

    impl TestPlayer {
        fn new() -> Self {
            let equipment = Arc::new(Mutex::new(EntityEquipment::new()));
            let mut equipment_slots = HashMap::new();
            equipment_slots.insert(40, EquipmentSlot::OFF_HAND);
            let inventory = Arc::new(PlayerInventory::new(equipment, Arc::new(equipment_slots)));
            Self { inventory }
        }
    }

    impl InventoryPlayer for TestPlayer {
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn drop_item(&self, _item: ItemStack, _retain_ownership: bool) -> PlayerFuture<'_, ()> {
            Box::pin(async {})
        }
        fn get_inventory(&self) -> Arc<PlayerInventory> {
            self.inventory.clone()
        }
        fn has_infinite_materials(&self) -> bool {
            false
        }
        fn is_creative(&self) -> bool {
            false
        }
        fn experience_level(&self) -> i32 {
            0
        }
        fn add_experience_levels(&self, _levels: i32) -> PlayerFuture<'_, ()> {
            Box::pin(async {})
        }
        fn enchantment_seed(&self) -> i32 {
            0
        }
        fn set_enchantment_seed(&self, _seed: i32) -> PlayerFuture<'_, ()> {
            Box::pin(async {})
        }
        fn enqueue_inventory_packet<'a>(
            &'a self,
            _packet: &'a CSetContainerContent,
            _window_type: Option<WindowType>,
        ) -> PlayerFuture<'a, ()> {
            Box::pin(async {})
        }
        fn enqueue_slot_packet<'a>(
            &'a self,
            _packet: &'a CSetContainerSlot,
            _window_type: Option<WindowType>,
            _total_slots: usize,
        ) -> PlayerFuture<'a, ()> {
            Box::pin(async {})
        }
        fn enqueue_cursor_packet<'a>(
            &'a self,
            _packet: &'a CSetCursorItem,
        ) -> PlayerFuture<'a, ()> {
            Box::pin(async {})
        }
        fn enqueue_property_packet<'a>(
            &'a self,
            _packet: &'a CSetContainerProperty,
        ) -> PlayerFuture<'a, ()> {
            Box::pin(async {})
        }
        fn enqueue_slot_set_packet<'a>(
            &'a self,
            _packet: &'a CSetPlayerInventory,
        ) -> PlayerFuture<'a, ()> {
            Box::pin(async {})
        }
        fn enqueue_set_held_item_packet<'a>(
            &'a self,
            _packet: &'a CSetSelectedSlot,
        ) -> PlayerFuture<'a, ()> {
            Box::pin(async {})
        }
        fn enqueue_equipment_change<'a>(
            &'a self,
            _slot: &'a EquipmentSlot,
            _stack: &'a ItemStack,
        ) -> PlayerFuture<'a, ()> {
            Box::pin(async {})
        }
        fn award_experience(&self, _amount: i32) -> PlayerFuture<'_, ()> {
            Box::pin(async {})
        }
        fn increment_stat(
            &self,
            _category: StatisticCategory,
            _stat_id: i32,
            _amount: i32,
        ) -> PlayerFuture<'_, ()> {
            Box::pin(async {})
        }
    }

    #[tokio::test]
    async fn generic_chest_9x3_quick_move() {
        let player = TestPlayer::new();
        let inv = Arc::new(SimpleInventory::new(27));
        let mut handler = create_generic_9x3(1, &player.inventory, inv.clone()).await;

        let diamond = ItemStack::new(64, &Item::DIAMOND);
        handler.behaviour.slots[0].set_stack(diamond.clone()).await;

        // Shift click from chest slot 0 to player inventory
        let moved = handler.quick_move(&player, 0).await;
        assert_eq!(moved.item_count, 64);
        assert!(
            handler.behaviour.slots[0]
                .get_cloned_stack()
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn generic_hopper_quick_move_bounds() {
        let player = TestPlayer::new();
        let inv = Arc::new(SimpleInventory::new(5));
        let mut handler = create_hopper(1, &player.inventory, inv.clone()).await;

        // Hopper has 5 slots (0..5), player slots start at 5
        let stone = ItemStack::new(16, &Item::STONE);
        // Put in first player slot (slot index 5)
        handler.behaviour.slots[5].set_stack(stone.clone()).await;

        // Quick move from player slot into hopper
        let moved = handler.quick_move(&player, 5).await;
        assert_eq!(moved.item_count, 16);
        assert!(
            handler.behaviour.slots[5]
                .get_cloned_stack()
                .await
                .is_empty()
        );
        assert_eq!(
            handler.behaviour.slots[0]
                .get_cloned_stack()
                .await
                .item_count,
            16
        );
    }
}
