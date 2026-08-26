use crate::{
    player::player_inventory::PlayerInventory,
    screen_handler::{
        InventoryPlayer, ItemStackFuture, ScreenHandler, ScreenHandlerBehaviour,
        ScreenHandlerFuture,
    },
    slot::NormalSlot,
};
use pumpkin_data::{item_stack::ItemStack, screen::WindowType};
use pumpkin_world::inventory::Inventory;
use std::{any::Any, sync::Arc};

/// Builder for custom GUIs.
pub struct GUIBuilder {
    screen_type: WindowType,
    rows: u8,
    columns: u8,
    inventory: Arc<dyn Inventory>,
    allow_grab_items: bool,
    allow_put_items: bool,
}

impl GUIBuilder {
    /// Creates a new `GUIBuilder` with a basic 9x3 layout.
    pub fn new(screen_type: WindowType, inventory: Arc<dyn Inventory>) -> Self {
        let (rows, columns) = match screen_type {
            WindowType::Generic9x1 => (1, 9),
            WindowType::Generic9x2 => (2, 9),
            WindowType::Generic9x4 => (4, 9),
            WindowType::Generic9x5 => (5, 9),
            WindowType::Generic9x6 => (6, 9),
            WindowType::Generic3x3 | WindowType::Crafter3x3 => (3, 3),
            WindowType::Hopper => (1, 5),
            _ => (3, 9), // Default to 9x3
        };

        Self {
            screen_type,
            rows,
            columns,
            inventory,
            allow_grab_items: true,
            allow_put_items: true,
        }
    }

    /// Sets whether players can grab items out of the inventory.
    #[must_use]
    pub const fn allow_grab_items(mut self, allow: bool) -> Self {
        self.allow_grab_items = allow;
        self
    }

    /// Sets whether players can put items into the inventory from their own.
    #[must_use]
    pub const fn allow_put_items(mut self, allow: bool) -> Self {
        self.allow_put_items = allow;
        self
    }

    /// Builds the `GUIScreenHandler`.
    pub async fn build(
        self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
    ) -> GUIScreenHandler {
        let mut behaviour = ScreenHandlerBehaviour::new(sync_id, Some(self.screen_type));
        behaviour.allow_grab_items = self.allow_grab_items;
        behaviour.allow_put_items = self.allow_put_items;
        behaviour.container_slots = (self.rows * self.columns) as usize;

        let mut handler = GUIScreenHandler {
            inventory: self.inventory.clone(),
            rows: self.rows,
            columns: self.columns,
            behaviour,
        };

        self.inventory.on_open().await;

        handler.add_inventory_slots();
        let player_inventory_trait: Arc<dyn Inventory> = player_inventory.clone();
        handler.add_player_slots(&player_inventory_trait);

        handler
    }
}

pub struct GUIScreenHandler {
    pub inventory: Arc<dyn Inventory>,
    pub rows: u8,
    pub columns: u8,
    behaviour: ScreenHandlerBehaviour,
}

impl GUIScreenHandler {
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

impl ScreenHandler for GUIScreenHandler {
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
                let container_slots = i32::from(self.rows * self.columns);

                if slot_index < container_slots {
                    // From container to player
                    if !self.get_behaviour().allow_grab_items {
                        return ItemStack::EMPTY.clone();
                    }
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
                } else {
                    // From player to container
                    if !self.get_behaviour().allow_put_items {
                        return ItemStack::EMPTY.clone();
                    }
                    if !self
                        .insert_item(&mut slot_stack, 0, container_slots, false)
                        .await
                    {
                        // Move from player area to inventory (start)
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
    async fn gui_builder_permissions_quick_move() {
        let player = TestPlayer::new();
        let inv = Arc::new(SimpleInventory::new(27));

        // Builder with allow_grab_items(false)
        let mut no_grab = GUIBuilder::new(WindowType::Generic9x3, inv.clone())
            .allow_grab_items(false)
            .build(1, &player.inventory)
            .await;

        let diamond = ItemStack::new(10, &Item::DIAMOND);
        no_grab.behaviour.slots[0].set_stack(diamond.clone()).await;

        // Quick moving from slot 0 should fail because grabbing is disabled
        let moved = no_grab.quick_move(&player, 0).await;
        assert!(moved.is_empty());
        assert_eq!(
            no_grab.behaviour.slots[0]
                .get_cloned_stack()
                .await
                .item_count,
            10
        );

        // Builder with allow_put_items(false)
        let mut no_put = GUIBuilder::new(WindowType::Generic9x3, inv.clone())
            .allow_put_items(false)
            .build(2, &player.inventory)
            .await;

        // Player slot 27 (first main inventory slot)
        let stone = ItemStack::new(10, &Item::STONE);
        no_put.behaviour.slots[27].set_stack(stone.clone()).await;

        // Quick moving from player slot should fail because putting is disabled
        let moved = no_put.quick_move(&player, 27).await;
        assert!(moved.is_empty());
        assert_eq!(
            no_put.behaviour.slots[27]
                .get_cloned_stack()
                .await
                .item_count,
            10
        );
    }
}
