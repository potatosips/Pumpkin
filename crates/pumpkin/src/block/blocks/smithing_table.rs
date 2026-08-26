use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, BlockFuture, NormalUseArgs};

use pumpkin_data::translation;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{
    BoxFuture, InventoryPlayer, ScreenHandler, ScreenHandlerFactory, SharedScreenHandler,
};
use pumpkin_inventory::smithing_table_screen_handler::SmithingTableScreenHandler;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::text::TextComponent;
use std::sync::Arc;
use tokio::sync::Mutex;

#[pumpkin_block("minecraft:smithing_table")]
pub struct SmithingTableBlock;

impl BlockBehaviour for SmithingTableBlock {
    fn normal_use<'a>(&'a self, args: NormalUseArgs<'a>) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async move {
            args.player
                .increment_stat(
                    pumpkin_data::statistic::StatisticCategory::Custom,
                    pumpkin_data::statistic::CustomStatistic::InteractWithSmithingTable as i32,
                    1,
                )
                .await;
            args.player
                .open_handled_screen(
                    &SmithingTableScreenFactory {
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

struct SmithingTableScreenFactory {
    position: pumpkin_util::math::position::BlockPos,
    world: Arc<crate::world::World>,
}

impl ScreenHandlerFactory for SmithingTableScreenFactory {
    fn create_screen_handler<'a>(
        &'a self,
        sync_id: u8,
        player_inventory: &'a Arc<PlayerInventory>,
        _player: &'a dyn InventoryPlayer,
    ) -> BoxFuture<'a, Option<SharedScreenHandler>> {
        Box::pin(async move {
            let mut handler = SmithingTableScreenHandler::new(sync_id, player_inventory);
            let pos = self.position;
            let world = self.world.clone();
            handler
                .get_behaviour_mut()
                .set_validity_check(move |player| {
                    let state_id = world.get_block_state(&pos).id;
                    let block = pumpkin_data::Block::from_state_id(state_id);
                    block == &pumpkin_data::Block::SMITHING_TABLE
                        && player.can_interact_with_block_at(&pos, 4.0)
                });
            let handler: SharedScreenHandler = Arc::new(Mutex::new(handler));
            Some(handler)
        })
    }

    fn get_display_name(&self) -> TextComponent {
        TextComponent::translate(translation::java::CONTAINER_UPGRADE, [])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn smithing_table_block_id_parity() {
        assert_eq!(Block::SMITHING_TABLE.name, "smithing_table");
    }

    #[test]
    fn smithing_table_default_state_parity() {
        assert_ne!(
            Block::SMITHING_TABLE.default_state.id,
            Block::AIR.default_state.id
        );
    }
}
