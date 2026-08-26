use std::sync::Arc;

use crate::block::entities::enchanting_table::EnchantingTableBlockEntity;
use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, BlockFuture, NormalUseArgs, PlacedArgs};
use pumpkin_data::{Block, translation};
use pumpkin_inventory::enchanting::enchanting_screen_handler::EnchantingTableScreenHandler;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{
    BoxFuture, InventoryPlayer, ScreenHandler, ScreenHandlerFactory, SharedScreenHandler,
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::text::TextComponent;
use pumpkin_world::inventory::{Inventory, SimpleInventory};
use tokio::sync::Mutex;

#[pumpkin_block("minecraft:enchanting_table")]
pub struct EnchantingTableBlock;

impl BlockBehaviour for EnchantingTableBlock {
    fn placed<'a>(&'a self, args: PlacedArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let entity = EnchantingTableBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(entity));
        })
    }

    fn normal_use<'a>(&'a self, args: NormalUseArgs<'a>) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async move {
            let mut bookshelf_count = 0;

            for off_z in -1..=1 {
                for off_x in -1..=1 {
                    if (off_z != 0 || off_x != 0)
                        && args
                            .world
                            .get_block_state(&args.position.add(off_x, 0, off_z))
                            .is_air()
                        && args
                            .world
                            .get_block_state(&args.position.add(off_x, 1, off_z))
                            .is_air()
                    {
                        for off_y in 0..=1 {
                            if Self::is_bookshelf(
                                args.world,
                                &args.position.add(off_x * 2, off_y, off_z * 2),
                            ) {
                                bookshelf_count += 1;
                            }
                            if off_x != 0 && off_z != 0 {
                                if Self::is_bookshelf(
                                    args.world,
                                    &args.position.add(off_x * 2, off_y, off_z),
                                ) {
                                    bookshelf_count += 1;
                                }
                                if Self::is_bookshelf(
                                    args.world,
                                    &args.position.add(off_x, off_y, off_z * 2),
                                ) {
                                    bookshelf_count += 1;
                                }
                            }
                        }
                    }
                }
            }
            let bookshelf_count = bookshelf_count.min(15);

            args.player
                .open_handled_screen(
                    &EnchantingTableScreenFactory {
                        bookshelf_count,
                        seed: args.player.enchantment_seed(),
                        position: *args.position,
                        world: args.world.clone(),
                    },
                    Some(*args.position),
                )
                .await;
            BlockActionResult::Success
        })
    }
}

impl EnchantingTableBlock {
    fn is_bookshelf(world: &Arc<crate::world::World>, pos: &BlockPos) -> bool {
        let block = world.get_block(pos);
        block == &Block::BOOKSHELF || block == &Block::CHISELED_BOOKSHELF
    }
}

struct EnchantingTableScreenFactory {
    bookshelf_count: i32,
    seed: i32,
    position: BlockPos,
    world: Arc<crate::world::World>,
}

impl ScreenHandlerFactory for EnchantingTableScreenFactory {
    fn create_screen_handler<'a>(
        &'a self,
        sync_id: u8,
        player_inventory: &'a Arc<PlayerInventory>,
        _player: &'a dyn InventoryPlayer,
    ) -> BoxFuture<'a, Option<SharedScreenHandler>> {
        Box::pin(async move {
            let inventory: Arc<dyn Inventory> = Arc::new(SimpleInventory::new(2));
            let mut handler = EnchantingTableScreenHandler::new(
                sync_id,
                player_inventory,
                &inventory,
                self.seed,
                self.bookshelf_count,
            );
            let pos = self.position;
            let world = self.world.clone();
            handler
                .get_behaviour_mut()
                .set_validity_check(move |player| {
                    let state_id = world.get_block_state(&pos).id;
                    let block = pumpkin_data::Block::from_state_id(state_id);
                    block == &pumpkin_data::Block::ENCHANTING_TABLE
                        && player.can_interact_with_block_at(&pos, 4.0)
                });
            let screen_handler_arc = Arc::new(Mutex::new(handler));
            Some(screen_handler_arc as SharedScreenHandler)
        })
    }

    fn get_display_name(&self) -> TextComponent {
        pumpkin_macros::translate_cross!(
            translation::java::CONTAINER_ENCHANT,
            translation::bedrock::CONTAINER_ENCHANT
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn enchanting_table_block_id_parity() {
        assert_eq!(Block::ENCHANTING_TABLE.name, "enchanting_table");
    }

    #[test]
    fn enchanting_table_default_state_parity() {
        assert_ne!(
            Block::ENCHANTING_TABLE.default_state.id,
            Block::AIR.default_state.id
        );
    }
}
