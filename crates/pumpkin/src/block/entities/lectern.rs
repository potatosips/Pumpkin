use pumpkin_data::data_component_impl::{WritableBookContentImpl, WrittenBookContentImpl};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::math::position::BlockPos;
use std::{
    any::Any,
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use tokio::sync::Mutex;

use crate::block::entities::BlockEntity;
use pumpkin_world::inventory::{Clearable, Inventory, InventoryFuture};

pub struct LecternBlockEntity {
    pub position: BlockPos,
    pub book: Arc<Mutex<ItemStack>>,
    pub page: AtomicUsize,
    pub dirty: AtomicBool,
}

impl BlockEntity for LecternBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let book_stack = nbt
            .get_compound("Book")
            .and_then(ItemStack::read_item_stack)
            .unwrap_or_else(|| ItemStack::EMPTY.clone());

        let page_count = Self::page_count_of(&book_stack);
        let page = nbt
            .get_int("Page")
            .unwrap_or(0)
            .clamp(0, page_count.saturating_sub(1).max(0)) as usize;
        let book = Arc::new(Mutex::new(book_stack));

        Self {
            position,
            book,
            page: AtomicUsize::new(page),
            dirty: AtomicBool::new(false),
        }
    }

    fn write_nbt<'a>(
        &'a self,
        nbt: &'a mut NbtCompound,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let book = self.book.lock().await;
            if !book.is_empty() {
                let mut book_nbt = NbtCompound::default();
                book.write_item_stack(&mut book_nbt);
                nbt.put_compound("Book", book_nbt);
            }
            nbt.put_int("Page", self.page.load(Ordering::Relaxed) as i32);
        })
    }

    fn get_inventory(self: Arc<Self>) -> Option<Arc<dyn Inventory>> {
        Some(self)
    }

    fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }

    fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::Relaxed);
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        if let Ok(book) = self.book.try_lock()
            && !book.is_empty()
        {
            let mut book_nbt = NbtCompound::new();
            book.write_item_stack(&mut book_nbt);
            nbt.put("Book", NbtTag::Compound(book_nbt));
        }
        nbt.put_int("Page", self.page.load(Ordering::Relaxed) as i32);
        Some(nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl LecternBlockEntity {
    pub const ID: &'static str = "minecraft:lectern";

    #[must_use]
    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            book: Arc::new(Mutex::new(ItemStack::EMPTY.clone())),
            page: AtomicUsize::new(0),
            dirty: AtomicBool::new(false),
        }
    }

    /// Number of pages in a writable or written book, `0` for anything else.
    #[must_use]
    pub fn page_count_of(stack: &ItemStack) -> i32 {
        stack
            .get_data_component::<WrittenBookContentImpl>()
            .map(|content| content.pages.len())
            .or_else(|| {
                stack
                    .get_data_component::<WritableBookContentImpl>()
                    .map(|content| content.pages.len())
            })
            .map_or(0, |pages| pages as i32)
    }

    pub async fn page_count(&self) -> i32 {
        Self::page_count_of(&*self.book.lock().await)
    }

    /// Vanilla comparator output: `floor(progress * 14) + 1`, or `0` without
    /// a book. Progress is `page / (page_count - 1)` for multi-page books and
    /// explicitly `1.0` for books with one page or fewer.
    pub async fn comparator_output(&self) -> u8 {
        let book = self.book.lock().await;
        if book.is_empty() {
            return 0;
        }

        let page = self.page.load(Ordering::Relaxed) as f32;
        let page_count = Self::page_count_of(&book);
        let progress = if page_count > 1 {
            page / (page_count - 1) as f32
        } else {
            1.0
        };
        (progress * 14.0).floor() as u8 + 1
    }
}

impl Inventory for LecternBlockEntity {
    fn size(&self) -> usize {
        1
    }

    fn is_empty(&self) -> InventoryFuture<'_, bool> {
        Box::pin(async move { self.book.lock().await.is_empty() })
    }

    fn get_stack(&self, _slot: usize) -> InventoryFuture<'_, ItemStack> {
        Box::pin(async move { self.book.lock().await.clone() })
    }

    fn remove_stack(&self, _slot: usize) -> InventoryFuture<'_, ItemStack> {
        Box::pin(async move {
            let mut removed = ItemStack::EMPTY.clone();
            let mut guard = self.book.lock().await;
            std::mem::swap(&mut removed, &mut *guard);
            self.mark_dirty();
            removed
        })
    }

    fn remove_stack_specific(&self, _slot: usize, amount: u8) -> InventoryFuture<'_, ItemStack> {
        Box::pin(async move {
            let mut stack = self.book.lock().await;
            if stack.is_empty() {
                return ItemStack::EMPTY.clone();
            }
            let res = stack.split(amount);
            self.mark_dirty();
            res
        })
    }

    fn set_stack(&self, _slot: usize, stack: ItemStack) -> InventoryFuture<'_, ()> {
        Box::pin(async move {
            *self.book.lock().await = stack;
            // A freshly placed book always opens on its first page.
            self.page.store(0, Ordering::Relaxed);
            self.mark_dirty();
        })
    }

    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
    }

    /// Vanilla does not expose the lectern's internal book container to
    /// hopper automation. The inventory remains available to its menu, but
    /// automated insertion and extraction must both be rejected.
    fn is_valid_slot_for(&self, _slot: usize, _stack: &ItemStack) -> bool {
        false
    }

    fn can_transfer_to(
        &self,
        _hopper_inventory: &dyn Inventory,
        _slot: usize,
        _stack: &ItemStack,
    ) -> bool {
        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clearable for LecternBlockEntity {
    fn clear(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            *self.book.lock().await = ItemStack::EMPTY.clone();
            self.mark_dirty();
        })
    }
}

#[cfg(test)]
mod tests {
    use pumpkin_data::{
        data_component::DataComponent,
        data_component_impl::{DataComponentImpl, WritableBookContentImpl},
        item::Item,
    };

    use super::*;

    #[test]
    fn hopper_insertion_and_extraction_are_rejected() {
        let lectern = LecternBlockEntity::new(BlockPos::new(0, 0, 0));
        let book = ItemStack::new(1, &Item::WRITABLE_BOOK);

        assert!(!lectern.is_valid_slot_for(0, &book));
        assert!(!lectern.can_transfer_to(&lectern, 0, &book));
    }

    #[tokio::test]
    async fn menu_inventory_can_still_store_and_remove_a_book() {
        let lectern = LecternBlockEntity::new(BlockPos::new(0, 0, 0));
        let book = ItemStack::new(1, &Item::WRITABLE_BOOK);

        lectern.set_stack(0, book).await;
        assert_eq!(lectern.get_stack(0).await.item_count, 1);
        assert_eq!(lectern.remove_stack(0).await.item_count, 1);
        assert!(lectern.is_empty().await);
    }

    fn writable_book_with_pages(pages: usize) -> ItemStack {
        let mut book = ItemStack::new(1, &Item::WRITABLE_BOOK);
        book.patch.push((
            DataComponent::WritableBookContent,
            Some(
                WritableBookContentImpl {
                    pages: (0..pages).map(|page| format!("page {page}")).collect(),
                    filtered_pages: vec![None; pages],
                }
                .to_dyn(),
            ),
        ));
        book
    }

    #[tokio::test]
    async fn comparator_is_zero_without_a_book_and_fifteen_for_one_page() {
        let lectern = LecternBlockEntity::new(BlockPos::new(0, 0, 0));
        assert_eq!(lectern.comparator_output().await, 0);

        lectern.set_stack(0, writable_book_with_pages(1)).await;
        assert_eq!(lectern.comparator_output().await, 15);
    }

    #[tokio::test]
    async fn comparator_scales_from_one_to_fifteen_across_pages() {
        let lectern = LecternBlockEntity::new(BlockPos::new(0, 0, 0));
        lectern.set_stack(0, writable_book_with_pages(3)).await;

        assert_eq!(lectern.comparator_output().await, 1);
        lectern.page.store(1, Ordering::Relaxed);
        assert_eq!(lectern.comparator_output().await, 8);
        lectern.page.store(2, Ordering::Relaxed);
        assert_eq!(lectern.comparator_output().await, 15);
    }
}
