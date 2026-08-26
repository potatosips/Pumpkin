//! Player inventory screen handler.
//!
//! This module handles the player's inventory screen (opened with E key).
//! It includes:
//! - The 2x2 crafting grid (inventory crafting)
//! - Armor slots (head, chest, legs, feet)
//! - Main inventory (27 slots)
//! - Hotbar (9 slots)
//! - Offhand slot
//!
//! # Slot Layout
//!
//! The player screen handler uses the following slot indices:
//! - 0: Crafting result
//! - 1-4: Crafting grid (2x2)
//! - 5-8: Armor slots (head, chest, legs, feet)
//! - 9-35: Main inventory
//! - 36-44: Hotbar
//! - 45: Offhand

use super::player_inventory::PlayerInventory;
use crate::crafting::crafting_inventory::CraftingInventory;
use crate::crafting::crafting_screen_handler::CraftingScreenHandler;
use crate::crafting::recipes::{RecipeFinderScreenHandler, RecipeInputInventory};
use crate::screen_handler::{
    InventoryPlayer, ItemStackFuture, ScreenHandler, ScreenHandlerBehaviour, ScreenHandlerFuture,
};
use crate::slot::{ArmorSlot, NormalSlot, Slot};
use pumpkin_data::data_component_impl::{EquipmentSlot, EquipmentType, EquippableImpl};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::screen::WindowType;
use pumpkin_world::inventory::Inventory;
use std::any::Any;
use std::sync::Arc;

/// Screen handler for the player's inventory.
///
/// Manages the player's inventory UI including crafting, armor, and
/// the main inventory. This is the default screen shown when pressing E.
pub struct PlayerScreenHandler {
    /// Core screen handler behavior (slots, sync ID, listeners).
    behaviour: ScreenHandlerBehaviour,
    /// The 2x2 crafting grid inventory.
    crafting_inventory: Arc<dyn RecipeInputInventory>,
}

impl RecipeFinderScreenHandler for PlayerScreenHandler {}

impl CraftingScreenHandler<CraftingInventory> for PlayerScreenHandler {}

// TODO: Fully implement this
impl PlayerScreenHandler {
    /// Equipment slot order for armor display.
    const EQUIPMENT_SLOT_ORDER: [EquipmentSlot; 4] = [
        EquipmentSlot::HEAD,
        EquipmentSlot::CHEST,
        EquipmentSlot::LEGS,
        EquipmentSlot::FEET,
    ];

    /// Checks if a slot index is in the hotbar.
    ///
    /// Hotbar slots are 36-44 in the protocol (0-indexed 36-44).
    #[must_use]
    pub fn is_in_hotbar(slot: u8) -> bool {
        (36..=45).contains(&slot)
    }

    /// Gets a slot by its index.
    pub fn get_slot(&self, slot: usize) -> Arc<dyn Slot> {
        self.behaviour.slots[slot].clone()
    }

    /// Creates a new player screen handler.
    ///
    /// # Arguments
    /// - `player_inventory` - The player's inventory
    /// - `window_type` - The window type (usually None for player inventory)
    /// - `sync_id` - The synchronization ID
    pub async fn new(
        player_inventory: &Arc<PlayerInventory>,
        window_type: Option<WindowType>,
        sync_id: u8,
        provider: Option<Arc<dyn crate::crafting::recipe_provider::RecipeProvider>>,
    ) -> Self {
        let crafting_inventory: Arc<dyn RecipeInputInventory> =
            Arc::new(CraftingInventory::new(2, 2));

        let mut player_screen_handler = Self {
            behaviour: ScreenHandlerBehaviour::new(sync_id, window_type),
            crafting_inventory: crafting_inventory.clone(),
        };

        player_screen_handler
            .add_recipe_slots(crafting_inventory, provider)
            .await;

        // Add armor slots (head, chest, legs, feet)
        for i in 0..4 {
            player_screen_handler.add_slot(Arc::new(ArmorSlot::new(
                player_inventory.clone(),
                39 - i,
                Self::EQUIPMENT_SLOT_ORDER[i].clone(),
            )));
        }

        let player_inventory: Arc<dyn Inventory> = player_inventory.clone();

        // Add main inventory and hotbar
        player_screen_handler.add_player_slots(&player_inventory);

        // Offhand slot (index 40 in player inventory, 45 in screen handler)
        // TODO: onEquipStack callback for offhand
        player_screen_handler.add_slot(Arc::new(NormalSlot::new(player_inventory.clone(), 40)));

        player_screen_handler
    }
}

impl ScreenHandler for PlayerScreenHandler {
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
            if !self.behaviour.slots.is_empty() {
                self.behaviour.slots[0]
                    .set_stack(ItemStack::EMPTY.clone())
                    .await;
            }
            self.drop_inventory(player, self.crafting_inventory.clone())
                .await;
        })
    }

    /// Performs quick move (shift-click) for the given slot.
    ///
    /// The quick move logic depends on the source slot:
    /// - Crafting result (0) -> Player inventory (from end)
    /// - Crafting grid (1-4) -> Player inventory (from start)
    /// - Armor slots (5-8) -> Player inventory, unequips
    /// - Armor items -> Armor slots if empty
    /// - Offhand items -> Offhand slot if empty
    /// - Main inventory (9-35) -> Hotbar
    /// - Hotbar (36-44) -> Main inventory
    fn quick_move<'a>(
        &'a mut self,
        player: &'a dyn InventoryPlayer,
        slot_index: i32,
    ) -> ItemStackFuture<'a> {
        Box::pin(async move {
            let slot = self.get_behaviour().slots[slot_index as usize].clone();

            // TODO: Equippable component

            if slot.has_stack().await {
                let mut slot_stack = slot.get_stack().await;
                let stack_prev = slot_stack.clone();

                let equipment_slot = slot_stack
                    .get_data_component::<EquippableImpl>()
                    .map_or(&EquipmentSlot::MAIN_HAND, |equippable| equippable.slot);

                // Quick move logic based on source slot
                let success = if slot_index == 0 {
                    // From crafting result slot (0) -> Player Inventory (9-45, from end)
                    self.insert_item(&mut slot_stack, 9, 45, true).await
                } else if (1..5).contains(&slot_index) {
                    // From craft ingredient slots (1-4) -> Player Inventory (9-45, from start)
                    self.insert_item(&mut slot_stack, 9, 45, false).await
                } else if (5..9).contains(&slot_index) {
                    // From armour slots (5-8) -> Player Inventory (9-45, from start)
                    let result = self.insert_item(&mut slot_stack, 9, 45, false).await;

                    if result {
                        player
                            .enqueue_equipment_change(equipment_slot, ItemStack::EMPTY)
                            .await;
                    }
                    result
                } else if equipment_slot.slot_type() == EquipmentType::HumanoidArmor
                    && self
                        .get_slot((8 - equipment_slot.get_entity_slot_id()) as usize)
                        .get_cloned_stack()
                        .await
                        .is_empty()
                {
                    // Into empty armour slots (5-8)
                    let index = 8 - equipment_slot.get_entity_slot_id();
                    let result = self
                        .insert_item(&mut slot_stack, index, index + 1, false)
                        .await;

                    if result {
                        player
                            .enqueue_equipment_change(equipment_slot, &stack_prev)
                            .await;
                    }
                    result
                } else if matches!(equipment_slot, EquipmentSlot::OffHand(_))
                    && slot_index != 45
                    && self.get_slot(45).get_cloned_stack().await.is_empty()
                {
                    // Into empty offhand slot (45)
                    let index = 45;
                    self.insert_item(&mut slot_stack, index, index + 1, false)
                        .await
                } else if (9..36).contains(&slot_index) {
                    // From main inventory (9-35) -> Hotbar (36-44)
                    self.insert_item(&mut slot_stack, 36, 45, false).await
                } else if (36..45).contains(&slot_index) {
                    // From hotbar (36-44) -> Main inventory (9-35)
                    self.insert_item(&mut slot_stack, 9, 36, false).await
                } else {
                    // Fallback to moving into the player inventory area
                    self.insert_item(&mut slot_stack, 9, 45, false).await
                };

                if !success {
                    return ItemStack::EMPTY.clone();
                }

                let stack = slot_stack.clone();

                if stack.is_empty() {
                    slot.set_stack_prev(ItemStack::EMPTY.clone(), stack_prev.clone())
                        .await;
                } else {
                    slot.set_stack(stack.clone()).await;
                }

                if stack.item_count == stack_prev.item_count {
                    return ItemStack::EMPTY.clone();
                }

                let mut taken_stack = stack_prev.clone();
                taken_stack.set_count(stack_prev.item_count - stack.item_count);
                slot.on_take_item(player, &taken_stack).await;

                if slot_index == 0 {
                    // From crafting result slot (0)
                    // Notify the result slot to refill
                    slot.on_quick_move_crafted(stack.clone(), stack_prev.clone())
                        .await;
                    // For crafting result slot, drop any remaining items
                    if !stack.is_empty() {
                        player.drop_item(stack, false).await;
                    }
                }

                return stack_prev;
            }

            // Nothing changed
            ItemStack::EMPTY.clone()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity_equipment::EntityEquipment;
    use crate::screen_handler::PlayerFuture;
    use pumpkin_data::item::Item;
    use pumpkin_data::statistic::StatisticCategory;
    use pumpkin_protocol::java::client::play::{
        CSetContainerContent, CSetContainerProperty, CSetContainerSlot, CSetCursorItem,
        CSetPlayerInventory, CSetSelectedSlot,
    };
    use std::collections::HashMap;
    use tokio::sync::Mutex;

    struct TestPlayer {
        inventory: Arc<PlayerInventory>,
        dropped: Arc<Mutex<Vec<ItemStack>>>,
    }

    impl TestPlayer {
        fn new() -> Self {
            let equipment = Arc::new(Mutex::new(EntityEquipment::new()));
            let mut equipment_slots = HashMap::new();
            equipment_slots.insert(40, EquipmentSlot::OFF_HAND);
            equipment_slots.insert(39, EquipmentSlot::HEAD);
            equipment_slots.insert(38, EquipmentSlot::CHEST);
            equipment_slots.insert(37, EquipmentSlot::LEGS);
            equipment_slots.insert(36, EquipmentSlot::FEET);
            let inventory = Arc::new(PlayerInventory::new(equipment, Arc::new(equipment_slots)));
            Self {
                inventory,
                dropped: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl InventoryPlayer for TestPlayer {
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn drop_item(&self, item: ItemStack, _retain_ownership: bool) -> PlayerFuture<'_, ()> {
            let dropped = self.dropped.clone();
            Box::pin(async move {
                dropped.lock().await.push(item);
            })
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
    async fn player_screen_handler_close_drops_crafting_items() {
        let player = TestPlayer::new();
        let mut handler = PlayerScreenHandler::new(&player.inventory, None, 0, None).await;

        // Place oak planks into 2x2 crafting slot (slot 1)
        let planks = ItemStack::new(4, &Item::OAK_PLANKS);
        handler
            .crafting_inventory
            .set_stack(0, planks.clone())
            .await;

        // Closing player screen drops the crafting items to player
        handler.on_closed(&player).await;
        assert!(handler.crafting_inventory.get_stack(0).await.is_empty());
        assert!(
            handler.behaviour.slots[0]
                .get_cloned_stack()
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn player_screen_handler_quick_move_hotbar_to_main() {
        let player = TestPlayer::new();
        let mut handler = PlayerScreenHandler::new(&player.inventory, None, 0, None).await;

        // Hotbar slot 36 (first hotbar slot)
        let stone = ItemStack::new(16, &Item::STONE);
        handler.behaviour.slots[36].set_stack(stone.clone()).await;

        // Quick move from hotbar to main inventory
        let moved = handler.quick_move(&player, 36).await;
        assert!(!moved.is_empty());

        // Slot 36 should now be empty and slot 9 (first main inv slot) should have stone
        assert!(
            handler.behaviour.slots[36]
                .get_cloned_stack()
                .await
                .is_empty()
        );
        assert_eq!(
            handler.behaviour.slots[9]
                .get_cloned_stack()
                .await
                .item_count,
            16
        );
    }
}
