//! Furnace-like screen handler.
//!
//! This module implements the screen handler for furnace-like blocks:
//! - Furnace
//! - Smoker
//! - Blast Furnace
//!
//! All three share the same 3-slot layout:
//! - Slot 0: Input (item to smelt/cook)
//! - Slot 1: Fuel (coal, charcoal, etc.)
//! - Slot 2: Output (smelted result)
//!
//! The screen handler tracks 4 properties:
//! - Property 0: Fire icon animation (fuel burn time remaining)
//! - Property 1: Maximum fuel burn time
//! - Property 2: Progress arrow (cooking/smelt time)
//! - Property 3: Maximum progress (typically 200 ticks for furnace)

use std::{any::Any, pin::Pin, sync::Arc};

use pumpkin_data::{fuels::is_fuel, item_stack::ItemStack, screen::WindowType};
use pumpkin_world::{
    block::entities::{ExperienceContainer, PropertyDelegate},
    inventory::Inventory,
};

use crate::{
    player::player_inventory::PlayerInventory,
    screen_handler::{
        InventoryPlayer, ItemStackFuture, ScreenHandler, ScreenHandlerBehaviour,
        ScreenHandlerFuture, ScreenHandlerListener, ScreenProperty,
    },
};
use tracing::debug;

use super::furnace_like_slot::{FurnaceLikeSlot, FurnaceLikeSlotType, FurnaceOutputSlot};

/// Screen handler for furnace-like containers.
///
/// Handles the UI for furnaces, smokers, and blast furnaces.
/// These all share the same slot layout and quick-move behavior.
pub struct FurnaceLikeScreenHandler {
    /// The furnace's inventory (3 slots: 0 input, 1 fuel, 2 output).
    pub inventory: Arc<dyn Inventory>,
    /// Container that tracks accumulated smelting experience.
    ///
    /// Experience is awarded to the player when they take items from the output slot.
    experience_container: Arc<dyn ExperienceContainer>,
    /// Core screen handler behavior (slots, sync ID, properties, listeners).
    behaviour: ScreenHandlerBehaviour,
}

impl FurnaceLikeScreenHandler {
    /// Creates a new furnace-like screen handler.
    ///
    /// # Arguments
    /// - `sync_id` - The sync ID for client-server matching
    /// - `player_inventory` - The player's inventory
    /// - `inventory` - The furnace's inventory (3 slots)
    /// - `property_delegate` - Delegate for accessing furnace properties
    /// - `experience_container` - Container that tracks smelting experience
    /// - `window_type` - The window type (Furnace, Smoker, or `BlastFurnace`)
    pub async fn new(
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        inventory: Arc<dyn Inventory>,
        property_delegate: Arc<dyn PropertyDelegate>,
        experience_container: Arc<dyn ExperienceContainer>,
        window_type: WindowType,
    ) -> Self {
        struct FurnaceLikeScreenListener;
        impl ScreenHandlerListener for FurnaceLikeScreenListener {
            fn on_property_update<'a>(
                &'a self,
                screen_handler: &'a ScreenHandlerBehaviour,
                property: u8,
                value: i32,
            ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
                Box::pin(async move {
                    if let Some(sync_handler) = screen_handler.sync_handler.as_ref() {
                        sync_handler
                            .update_property(screen_handler, i32::from(property), value)
                            .await;
                    }
                })
            }
        }
        let mut handler = Self {
            inventory,
            experience_container,
            behaviour: ScreenHandlerBehaviour::new(sync_id, Some(window_type)),
        };

        // 0: Fire icon (fuel left) counting from fuel burn time down to 0 (in-game ticks)
        // 1: Maximum fuel burn time fuel burn time or 0 (in-game ticks)
        // 2: Progress arrow counting from 0 to maximum progress (in-game ticks)
        // 3: Maximum progress always 200 on the vanilla server
        for i in 0..4 {
            handler.add_property(ScreenProperty::new(property_delegate.clone(), i));
        }

        handler
            .add_listener(Arc::new(FurnaceLikeScreenListener))
            .await;
        handler.add_inventory_slots();
        let player_inventory: Arc<dyn Inventory> = player_inventory.clone();
        handler.add_player_slots(&player_inventory);

        handler
    }

    /// Adds the 3 furnace inventory slots.
    ///
    /// - Slot 0: Input (top)
    /// - Slot 1: Fuel (bottom)
    /// - Slot 2: Output
    fn add_inventory_slots(&mut self) {
        self.add_slot(Arc::new(FurnaceLikeSlot::new(
            self.inventory.clone(),
            FurnaceLikeSlotType::Top,
        )));
        self.add_slot(Arc::new(FurnaceLikeSlot::new(
            self.inventory.clone(),
            FurnaceLikeSlotType::Bottom,
        )));
        // Output slot awards experience when items are taken
        self.add_slot(Arc::new(FurnaceOutputSlot::new(
            self.inventory.clone(),
            self.experience_container.clone(),
        )));
    }
}

impl ScreenHandler for FurnaceLikeScreenHandler {
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
            // TODO: self.inventory.on_closed(player).await;
        })
    }

    /// Quick move logic for furnace-like containers.
    ///
    /// - From furnace slots (0-2): Move to player inventory
    /// - Fuel items: Move to fuel slot (1)
    /// - Other items: Move to input slot (0)
    fn quick_move<'a>(
        &'a mut self,
        player: &'a dyn InventoryPlayer,
        slot_index: i32,
    ) -> ItemStackFuture<'a> {
        Box::pin(async move {
            const FUEL_SLOT: i32 = 1; // Note: Slots 0, 1, 2 are Furnace slots.
            const OUTPUT_SLOT: i32 = 2;

            debug!("FurnaceLikeScreenHandler::quick_move slot_index={slot_index}");

            let mut stack_left = ItemStack::EMPTY.clone();

            let slot = self.get_behaviour().slots[slot_index as usize].clone();

            if !slot.has_stack().await {
                return stack_left;
            }

            let mut stack = slot.get_stack().await;
            stack_left = stack.clone();

            let success = if slot_index < 3 {
                // If clicked slot is one of the Furnace slots (0, 1, 2):
                // Try to move to player inventory (slots 3 onwards, starting from the end)
                self.insert_item(&mut stack, 3, self.get_behaviour().slots.len() as i32, true)
                    .await
            } else if is_fuel(stack.item.id) {
                // If clicked slot is in the player inventory (3+) and contains fuel:
                // Try to move to the Furnace's Fuel slot (slot 1)
                self.insert_item(&mut stack, FUEL_SLOT, 3, false).await
            } else {
                // If clicked slot is in the player inventory (3+) and NOT fuel (must be a smeltable item):
                // Try to move to the Furnace's Input/Smelting slot (slot 0)
                self.insert_item(&mut stack, 0, 3, false).await
            };

            if !success {
                return ItemStack::EMPTY.clone();
            }

            if stack.is_empty() {
                slot.set_stack(ItemStack::EMPTY.clone()).await;
            } else {
                slot.set_stack(stack).await;
            }

            // Award XP when taking from output slot (slot 2)
            if slot_index == OUTPUT_SLOT {
                debug!("quick_move: taking from output slot, calling on_take_item");
                slot.on_take_item(player, &stack_left).await;
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
    use std::sync::atomic::AtomicI32;
    use tokio::sync::Mutex;

    struct TestPlayer {
        inventory: Arc<PlayerInventory>,
        awarded_xp: Arc<AtomicI32>,
    }

    impl TestPlayer {
        fn new() -> Self {
            let equipment = Arc::new(Mutex::new(EntityEquipment::new()));
            let mut equipment_slots = HashMap::new();
            equipment_slots.insert(40, EquipmentSlot::OFF_HAND);
            let inventory = Arc::new(PlayerInventory::new(equipment, Arc::new(equipment_slots)));
            Self {
                inventory,
                awarded_xp: Arc::new(AtomicI32::new(0)),
            }
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
        fn award_experience(&self, amount: i32) -> PlayerFuture<'_, ()> {
            let xp = self.awarded_xp.clone();
            Box::pin(async move {
                xp.fetch_add(amount, std::sync::atomic::Ordering::Relaxed);
            })
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

    struct MockExperienceContainer {
        xp: AtomicI32,
    }

    impl ExperienceContainer for MockExperienceContainer {
        fn extract_experience(&self) -> i32 {
            self.xp.swap(0, std::sync::atomic::Ordering::Relaxed)
        }
    }

    struct MockPropertyDelegate;
    impl PropertyDelegate for MockPropertyDelegate {
        fn get_property(&self, _index: i32) -> i32 {
            0
        }
        fn set_property(&self, _index: i32, _value: i32) {}
        fn get_properties_size(&self) -> i32 {
            4
        }
    }

    #[tokio::test]
    async fn furnace_slots_filtering_and_quick_move() {
        let player = TestPlayer::new();
        let inv = Arc::new(SimpleInventory::new(3));
        let xp_container = Arc::new(MockExperienceContainer {
            xp: AtomicI32::new(10),
        });
        let prop_delegate = Arc::new(MockPropertyDelegate);

        let mut handler = FurnaceLikeScreenHandler::new(
            1,
            &player.inventory,
            inv.clone(),
            prop_delegate,
            xp_container,
            WindowType::Furnace,
        )
        .await;

        // Slot 0 (top/input) accepts raw iron
        let raw_iron = ItemStack::new(10, &Item::RAW_IRON);
        assert!(handler.behaviour.slots[0].can_insert(&raw_iron).await);

        // Slot 1 (fuel) accepts coal, rejects raw iron
        let coal = ItemStack::new(10, &Item::COAL);
        assert!(handler.behaviour.slots[1].can_insert(&coal).await);
        assert!(!handler.behaviour.slots[1].can_insert(&raw_iron).await);

        // Slot 2 (output) rejects manual insertion
        let iron_ingot = ItemStack::new(10, &Item::IRON_INGOT);
        assert!(!handler.behaviour.slots[2].can_insert(&iron_ingot).await);

        // Taking from output slot awards XP
        handler.behaviour.slots[2]
            .set_stack(iron_ingot.clone())
            .await;
        let taken = handler.quick_move(&player, 2).await;
        assert_eq!(taken.item_count, 10);
        assert_eq!(
            player.awarded_xp.load(std::sync::atomic::Ordering::Relaxed),
            10
        );
    }
}
