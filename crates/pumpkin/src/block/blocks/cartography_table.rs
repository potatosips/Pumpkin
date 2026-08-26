use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, BlockFuture, NormalUseArgs};

use pumpkin_data::translation;
use pumpkin_inventory::cartography_table_screen_handler::CartographyTableScreenHandler;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{
    BoxFuture, InventoryPlayer, ScreenHandler, ScreenHandlerFactory, SharedScreenHandler,
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::text::TextComponent;
use std::sync::Arc;
use tokio::sync::Mutex;

#[pumpkin_block("minecraft:cartography_table")]
pub struct CartographyTableBlock;

impl BlockBehaviour for CartographyTableBlock {
    fn normal_use<'a>(&'a self, args: NormalUseArgs<'a>) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async move {
            args.player
                .increment_stat(
                    pumpkin_data::statistic::StatisticCategory::Custom,
                    pumpkin_data::statistic::CustomStatistic::InteractWithCartographyTable as i32,
                    1,
                )
                .await;
            args.player
                .open_handled_screen(
                    &CartographyTableScreenFactory {
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

struct CartographyTableScreenFactory {
    position: pumpkin_util::math::position::BlockPos,
    world: Arc<crate::world::World>,
}

impl ScreenHandlerFactory for CartographyTableScreenFactory {
    fn create_screen_handler<'a>(
        &'a self,
        sync_id: u8,
        player_inventory: &'a Arc<PlayerInventory>,
        _player: &'a dyn InventoryPlayer,
    ) -> BoxFuture<'a, Option<SharedScreenHandler>> {
        Box::pin(async move {
            let mut handler = CartographyTableScreenHandler::new(sync_id, player_inventory);
            let pos = self.position;
            let world = self.world.clone();
            handler
                .get_behaviour_mut()
                .set_validity_check(move |player| {
                    let state_id = world.get_block_state(&pos).id;
                    let block = pumpkin_data::Block::from_state_id(state_id);
                    block == &pumpkin_data::Block::CARTOGRAPHY_TABLE
                        && player.can_interact_with_block_at(&pos, 4.0)
                });
            let handler: SharedScreenHandler = Arc::new(Mutex::new(handler));
            Some(handler)
        })
    }

    fn get_display_name(&self) -> TextComponent {
        TextComponent::translate(translation::java::CONTAINER_CARTOGRAPHY_TABLE, [])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn cartography_table_block_id_parity() {
        assert_eq!(Block::CARTOGRAPHY_TABLE.name, "cartography_table");
    }

    #[test]
    fn cartography_table_default_state_parity() {
        assert_ne!(
            Block::CARTOGRAPHY_TABLE.default_state.id,
            Block::AIR.default_state.id
        );
    }
}
