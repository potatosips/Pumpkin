use super::BlockEntity;
use pumpkin_data::{
    Block,
    block_properties::{BlockProperties, CampfireLikeProperties},
    item::Item,
    item_stack::ItemStack,
    recipes::{CookingRecipeKind, get_cooking_recipe_with_ingredient},
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::math::position::BlockPos;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct CampfireBlockEntity {
    pub position: BlockPos,
    pub items: [Arc<Mutex<ItemStack>>; 4],
    pub cooking_times: [tokio::sync::Mutex<i32>; 4],
    pub cooking_total_times: [tokio::sync::Mutex<i32>; 4],
}

impl BlockEntity for CampfireBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let mut items = std::array::from_fn(|_| Arc::new(Mutex::new(ItemStack::EMPTY.clone())));
        if let Some(list) = nbt.get_list("Items") {
            for tag in list {
                if let Some(compound) = tag.extract_compound() {
                    let slot = compound.get_byte("Slot").unwrap_or(0) as usize;
                    if slot < 4
                        && let Some(stack) = ItemStack::read_item_stack(compound)
                    {
                        items[slot] = Arc::new(Mutex::new(stack));
                    }
                }
            }
        }
        let mut cooking_times = [Mutex::new(0), Mutex::new(0), Mutex::new(0), Mutex::new(0)];
        if let Some(arr) = nbt.get_int_array("CookingTimes") {
            for (i, &val) in arr.iter().enumerate().take(4) {
                cooking_times[i] = Mutex::new(val);
            }
        }
        let mut cooking_total_times = [Mutex::new(0), Mutex::new(0), Mutex::new(0), Mutex::new(0)];
        if let Some(arr) = nbt.get_int_array("CookingTotalTimes") {
            for (i, &val) in arr.iter().enumerate().take(4) {
                cooking_total_times[i] = Mutex::new(val);
            }
        }

        Self {
            position,
            items,
            cooking_times,
            cooking_total_times,
        }
    }

    fn write_nbt<'a>(
        &'a self,
        nbt: &'a mut NbtCompound,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let mut list = Vec::new();
            for (i, item_mutex) in self.items.iter().enumerate() {
                let stack = item_mutex.lock().await;
                if !stack.is_empty() {
                    let mut item_nbt = NbtCompound::new();
                    item_nbt.put_byte("Slot", i as i8);
                    stack.write_item_stack(&mut item_nbt);
                    list.push(NbtTag::Compound(item_nbt));
                }
            }
            nbt.put_list("Items", list);

            let mut times = Vec::new();
            for m in &self.cooking_times {
                times.push(*m.lock().await);
            }
            nbt.put("CookingTimes", NbtTag::IntArray(times));

            let mut total_times = Vec::new();
            for m in &self.cooking_total_times {
                total_times.push(*m.lock().await);
            }
            nbt.put("CookingTotalTimes", NbtTag::IntArray(total_times));
        })
    }

    fn tick<'a>(
        &'a self,
        world: &'a Arc<crate::world::World>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let state = world.get_block_state(&self.position);
            let block = world.get_block(&self.position);
            if (block != &Block::CAMPFIRE && block != &Block::SOUL_CAMPFIRE)
                || !CampfireLikeProperties::from_state_id(state.id, block).lit
            {
                return;
            }

            for slot in 0..self.items.len() {
                let mut stack = self.items[slot].lock().await;
                if stack.is_empty() {
                    continue;
                }

                let total = *self.cooking_total_times[slot].lock().await;
                let mut time = self.cooking_times[slot].lock().await;
                *time += 1;
                if *time < total {
                    continue;
                }

                let Some(recipe) = get_cooking_recipe_with_ingredient(
                    stack.get_item(),
                    CookingRecipeKind::CampfireCooking,
                ) else {
                    *stack = ItemStack::EMPTY.clone();
                    *time = 0;
                    continue;
                };
                let Some(output) = Item::from_registry_key(
                    recipe
                        .result
                        .id
                        .strip_prefix("minecraft:")
                        .unwrap_or(recipe.result.id),
                ) else {
                    continue;
                };

                let result = ItemStack::new(recipe.result.count, output);
                *stack = ItemStack::EMPTY.clone();
                *time = 0;
                *self.cooking_total_times[slot].lock().await = 0;
                drop(time);
                drop(stack);
                world.drop_stack(&self.position.up(), result).await;
            }
        })
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        let mut list = Vec::new();
        for (slot, item) in self.items.iter().enumerate() {
            if let Ok(stack) = item.try_lock()
                && !stack.is_empty()
            {
                let mut item_nbt = NbtCompound::new();
                item_nbt.put_byte("Slot", slot as i8);
                stack.write_item_stack(&mut item_nbt);
                list.push(NbtTag::Compound(item_nbt));
            }
        }
        nbt.put_list("Items", list);
        Some(nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl CampfireBlockEntity {
    pub const ID: &'static str = "minecraft:campfire";
    #[must_use]
    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            items: std::array::from_fn(|_| Arc::new(Mutex::new(ItemStack::EMPTY.clone()))),
            cooking_times: [Mutex::new(0), Mutex::new(0), Mutex::new(0), Mutex::new(0)],
            cooking_total_times: [Mutex::new(0), Mutex::new(0), Mutex::new(0), Mutex::new(0)],
        }
    }

    pub async fn add_item(&self, stack: ItemStack, cooking_time: i32) -> bool {
        for slot in 0..self.items.len() {
            let mut item = self.items[slot].lock().await;
            if item.is_empty() {
                *item = stack;
                *self.cooking_times[slot].lock().await = 0;
                *self.cooking_total_times[slot].lock().await = cooking_time;
                return true;
            }
        }
        false
    }

    pub async fn has_empty_slot(&self) -> bool {
        for item in &self.items {
            if item.lock().await.is_empty() {
                return true;
            }
        }
        false
    }

    pub async fn take_items(&self) -> Vec<ItemStack> {
        let mut taken = Vec::new();
        for slot in 0..self.items.len() {
            let mut item = self.items[slot].lock().await;
            if !item.is_empty() {
                taken.push(std::mem::replace(&mut *item, ItemStack::EMPTY.clone()));
            }
            *self.cooking_times[slot].lock().await = 0;
            *self.cooking_total_times[slot].lock().await = 0;
        }
        taken
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn campfire_manages_four_cooking_slots() {
        let campfire = CampfireBlockEntity::new(BlockPos::new(0, 64, 0));
        assert!(campfire.has_empty_slot().await);

        // Fill all 4 slots with 600-tick cooking times (Vanilla default)
        for _ in 0..4 {
            assert!(campfire.has_empty_slot().await);
            assert!(campfire.add_item(ItemStack::new(1, &Item::BEEF), 600).await);
        }

        // 5th item rejected
        assert!(!campfire.has_empty_slot().await);
        assert!(
            !campfire
                .add_item(ItemStack::new(1, &Item::PORKCHOP), 600)
                .await
        );

        // Take all items
        let items = campfire.take_items().await;
        assert_eq!(items.len(), 4);
        assert!(campfire.has_empty_slot().await);
    }
}
