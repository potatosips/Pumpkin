use std::any::Any;
use std::sync::Arc;

use crate::screen_handler::{
    InventoryPlayer, ItemStackFuture, ScreenHandler, ScreenHandlerBehaviour, ScreenHandlerFuture,
    ScreenProperty, offer_or_drop_stack,
};
use crate::slot::NormalSlot;

use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::screen::WindowType;
use pumpkin_world::block::entities::PropertyDelegate;
use pumpkin_world::inventory::Inventory;

/// Callbacks into the lectern block so page turns and book removal can drive
/// block-state changes (redstone pulse, `has_book`) that live outside this crate.
pub trait LecternController: Send + Sync {
    /// Whether the original lectern is still present, contains its book, and
    /// is within the player's valid container interaction range.
    fn can_use(&self, player: &dyn InventoryPlayer) -> bool;

    /// The page currently displayed.
    fn current_page(&self) -> i32;

    /// Clamps and persists `page`, emitting a redstone pulse when it changes.
    fn set_page(&self, page: i32) -> ScreenHandlerFuture<'_, ()>;

    /// Restores the bookless block state after the book was taken.
    fn on_book_taken(&self) -> ScreenHandlerFuture<'_, ()>;
}

/// Exposes the current page as container property 0 (see `window_property::Lectern`).
struct PageDelegate(Arc<dyn LecternController>);

impl PropertyDelegate for PageDelegate {
    fn get_property(&self, index: i32) -> i32 {
        if index == 0 { self.0.current_page() } else { 0 }
    }

    fn set_property(&self, _index: i32, _value: i32) {}

    fn get_properties_size(&self) -> i32 {
        1
    }
}

/// Vanilla `LecternScreenHandler`: a single book slot, no player slots and the
/// current page synced as property 0. Page navigation and taking the book are
/// plain button clicks sent by the client.
pub struct LecternScreenHandler {
    behaviour: ScreenHandlerBehaviour,
    inventory: Arc<dyn Inventory>,
    controller: Arc<dyn LecternController>,
}

impl LecternScreenHandler {
    const PREVIOUS_PAGE_BUTTON_ID: i32 = 1;
    const NEXT_PAGE_BUTTON_ID: i32 = 2;
    const TAKE_BOOK_BUTTON_ID: i32 = 3;
    /// Button ids at or above this jump directly to `id - JUMP_TO_PAGE_OFFSET`.
    const JUMP_TO_PAGE_OFFSET: i32 = 100;

    pub fn new(
        sync_id: u8,
        inventory: Arc<dyn Inventory>,
        controller: Arc<dyn LecternController>,
    ) -> Self {
        let mut handler = Self {
            behaviour: ScreenHandlerBehaviour::new(sync_id, Some(WindowType::Lectern)),
            inventory: inventory.clone(),
            controller: controller.clone(),
        };

        handler.add_slot(Arc::new(NormalSlot::new(inventory, 0)));
        handler.add_property(ScreenProperty::new(Arc::new(PageDelegate(controller)), 0));

        handler
    }
}

impl ScreenHandler for LecternScreenHandler {
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

    fn can_use(&self, player: &dyn InventoryPlayer) -> bool {
        self.controller.can_use(player)
    }

    fn on_button_click<'a>(
        &'a mut self,
        player: &'a dyn InventoryPlayer,
        id: i32,
    ) -> ScreenHandlerFuture<'a, bool> {
        Box::pin(async move {
            match id {
                Self::PREVIOUS_PAGE_BUTTON_ID => {
                    self.controller
                        .set_page(self.controller.current_page() - 1)
                        .await;
                    true
                }
                Self::NEXT_PAGE_BUTTON_ID => {
                    self.controller
                        .set_page(self.controller.current_page() + 1)
                        .await;
                    true
                }
                Self::TAKE_BOOK_BUTTON_ID => {
                    // Vanilla LecternMenu checks Player.mayBuild() before it
                    // mutates the lectern inventory (Survival/Creative true,
                    // Adventure/Spectator false).
                    if !player.may_build() {
                        return false;
                    }
                    let stack = self.inventory.remove_stack(0).await;
                    if stack.is_empty() {
                        return false;
                    }
                    self.inventory.mark_dirty();
                    self.controller.on_book_taken().await;
                    offer_or_drop_stack(player, stack).await;
                    self.send_content_updates().await;
                    true
                }
                _ if id >= Self::JUMP_TO_PAGE_OFFSET => {
                    self.controller
                        .set_page(id - Self::JUMP_TO_PAGE_OFFSET)
                        .await;
                    true
                }
                _ => false,
            }
        })
    }

    fn quick_move<'a>(
        &'a mut self,
        _player: &'a dyn InventoryPlayer,
        _slot_index: i32,
    ) -> ItemStackFuture<'a> {
        // The lectern screen has no player slots, so nothing can be shift-clicked.
        Box::pin(async move { ItemStack::EMPTY.clone() })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering},
        },
    };

    use pumpkin_data::{
        data_component_impl::EquipmentSlot, item::Item, statistic::StatisticCategory,
    };
    use pumpkin_protocol::java::client::play::{
        CSetContainerContent, CSetContainerProperty, CSetContainerSlot, CSetCursorItem,
        CSetPlayerInventory, CSetSelectedSlot,
    };
    use pumpkin_util::math::position::BlockPos;
    use pumpkin_world::inventory::SimpleInventory;
    use tokio::sync::Mutex;

    use crate::{
        entity_equipment::EntityEquipment, player::player_inventory::PlayerInventory,
        screen_handler::PlayerFuture,
    };

    use super::*;

    struct TestController {
        page: AtomicI32,
        books_taken: AtomicUsize,
        valid: AtomicBool,
    }

    impl TestController {
        fn new() -> Self {
            Self {
                page: AtomicI32::new(0),
                books_taken: AtomicUsize::new(0),
                valid: AtomicBool::new(true),
            }
        }
    }

    impl LecternController for TestController {
        fn can_use(&self, player: &dyn InventoryPlayer) -> bool {
            self.valid.load(Ordering::Relaxed)
                && player.can_interact_with_block_at(&BlockPos::new(0, 0, 0), 4.0)
        }

        fn current_page(&self) -> i32 {
            self.page.load(Ordering::Relaxed)
        }

        fn set_page(&self, page: i32) -> ScreenHandlerFuture<'_, ()> {
            Box::pin(async move { self.page.store(page, Ordering::Relaxed) })
        }

        fn on_book_taken(&self) -> ScreenHandlerFuture<'_, ()> {
            Box::pin(async move {
                self.books_taken.fetch_add(1, Ordering::Relaxed);
            })
        }
    }

    struct TestPlayer {
        inventory: Arc<PlayerInventory>,
        may_build: bool,
        within_range: bool,
    }

    impl TestPlayer {
        fn new(may_build: bool) -> Self {
            Self {
                inventory: Arc::new(PlayerInventory::new(
                    Arc::new(Mutex::new(EntityEquipment::new())),
                    Arc::new(HashMap::new()),
                )),
                may_build,
                within_range: true,
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
        fn may_build(&self) -> bool {
            self.may_build
        }
        fn can_interact_with_block_at(&self, _position: &BlockPos, _additional_range: f64) -> bool {
            self.within_range
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

    async fn handler_with_book() -> (
        LecternScreenHandler,
        Arc<SimpleInventory>,
        Arc<TestController>,
    ) {
        let inventory = Arc::new(SimpleInventory::new(1));
        inventory
            .set_stack(0, ItemStack::new(1, &Item::WRITABLE_BOOK))
            .await;
        let controller = Arc::new(TestController::new());
        let handler = LecternScreenHandler::new(1, inventory.clone(), controller.clone());
        (handler, inventory, controller)
    }

    #[tokio::test]
    async fn take_book_is_denied_without_may_build_and_does_not_mutate() {
        let (mut handler, inventory, controller) = handler_with_book().await;
        let player = TestPlayer::new(false);

        assert!(!handler.on_button_click(&player, 3).await);
        assert_eq!(inventory.get_stack(0).await.item_count, 1);
        assert_eq!(controller.books_taken.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn take_book_is_allowed_with_may_build() {
        let (mut handler, inventory, controller) = handler_with_book().await;
        let player = TestPlayer::new(true);

        assert!(handler.on_button_click(&player, 3).await);
        assert!(inventory.get_stack(0).await.is_empty());
        assert_eq!(controller.books_taken.load(Ordering::Relaxed), 1);
        assert_eq!(player.inventory.get_stack(0).await.item_count, 1);
    }

    #[tokio::test]
    async fn menu_is_invalid_when_original_lectern_is_gone() {
        let (handler, _inventory, controller) = handler_with_book().await;
        let player = TestPlayer::new(true);
        controller.valid.store(false, Ordering::Relaxed);

        assert!(!handler.can_use(&player));
    }

    #[tokio::test]
    async fn menu_is_invalid_outside_interaction_range() {
        let (handler, _inventory, _controller) = handler_with_book().await;
        let mut player = TestPlayer::new(true);
        player.within_range = false;

        assert!(!handler.can_use(&player));
    }
}
