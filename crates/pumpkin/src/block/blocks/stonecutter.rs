use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, BlockFuture, NormalUseArgs};

use pumpkin_data::translation;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{
    BoxFuture, InventoryPlayer, ScreenHandler, ScreenHandlerFactory, SharedScreenHandler,
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::text::TextComponent;
use std::sync::Arc;
use tokio::sync::Mutex;

use pumpkin_inventory::stonecutter_screen_handler::StonecutterScreenHandler;

#[pumpkin_block("minecraft:stonecutter")]
pub struct StonecutterBlock;

impl BlockBehaviour for StonecutterBlock {
    fn normal_use<'a>(&'a self, args: NormalUseArgs<'a>) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async move {
            args.player
                .increment_stat(
                    pumpkin_data::statistic::StatisticCategory::Custom,
                    pumpkin_data::statistic::CustomStatistic::InteractWithStonecutter as i32,
                    1,
                )
                .await;
            args.player
                .open_handled_screen(
                    &StonecutterScreenFactory {
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

struct StonecutterScreenFactory {
    position: pumpkin_util::math::position::BlockPos,
    world: Arc<crate::world::World>,
}

impl ScreenHandlerFactory for StonecutterScreenFactory {
    fn create_screen_handler<'a>(
        &'a self,
        sync_id: u8,
        player_inventory: &'a Arc<PlayerInventory>,
        _player: &'a dyn InventoryPlayer,
    ) -> BoxFuture<'a, Option<SharedScreenHandler>> {
        Box::pin(async move {
            let mut handler = StonecutterScreenHandler::new(sync_id, player_inventory);
            let pos = self.position;
            let world = self.world.clone();
            handler
                .get_behaviour_mut()
                .set_validity_check(move |player| {
                    let state_id = world.get_block_state(&pos).id;
                    let block = pumpkin_data::Block::from_state_id(state_id);
                    block == &pumpkin_data::Block::STONECUTTER
                        && player.can_interact_with_block_at(&pos, 4.0)
                });
            let handler: SharedScreenHandler = Arc::new(Mutex::new(handler));
            Some(handler)
        })
    }

    fn get_display_name(&self) -> TextComponent {
        pumpkin_macros::translate_cross!(
            translation::java::CONTAINER_STONECUTTER,
            translation::bedrock::CONTAINER_STONECUTTER
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;
    use pumpkin_data::block_properties::{
        BlockProperties, HorizontalFacing, WallTorchLikeProperties,
    };

    #[test]
    fn stonecutter_block_id_parity() {
        assert_eq!(Block::STONECUTTER.name, "stonecutter");
    }

    #[test]
    fn stonecutter_default_state_parity() {
        assert_ne!(
            Block::STONECUTTER.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn stonecutter_properties_roundtrip_parity() {
        for facing in [
            HorizontalFacing::North,
            HorizontalFacing::South,
            HorizontalFacing::East,
            HorizontalFacing::West,
        ] {
            let props = WallTorchLikeProperties { facing };
            let state_id = props.to_state_id(&Block::STONECUTTER);
            let rt = WallTorchLikeProperties::from_state_id(state_id, &Block::STONECUTTER);
            assert_eq!(rt.facing, facing);
        }
    }
}
