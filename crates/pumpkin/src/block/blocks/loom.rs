use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, BlockFuture, NormalUseArgs, OnPlaceArgs};
use crate::entity::EntityBase;

use pumpkin_data::FacingExt;
use pumpkin_data::block_properties::{BlockProperties, WallTorchLikeProperties};
use pumpkin_data::translation;
use pumpkin_inventory::loom_screen_handler::LoomScreenHandler;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{
    BoxFuture, InventoryPlayer, ScreenHandler, ScreenHandlerFactory, SharedScreenHandler,
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::text::TextComponent;
use std::sync::Arc;
use tokio::sync::Mutex;

#[pumpkin_block("minecraft:loom")]
pub struct LoomBlock;

impl BlockBehaviour for LoomBlock {
    fn on_place<'a>(
        &'a self,
        args: OnPlaceArgs<'a>,
    ) -> BlockFuture<'a, pumpkin_data::BlockStateId> {
        Box::pin(async move {
            let mut props = WallTorchLikeProperties::default(args.block);
            if let Some(facing) = args
                .player
                .get_entity()
                .get_facing()
                .opposite()
                .to_horizontal_facing()
            {
                props.facing = facing;
            }
            props.to_state_id(args.block)
        })
    }

    fn normal_use<'a>(&'a self, args: NormalUseArgs<'a>) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async move {
            args.player
                .increment_stat(
                    pumpkin_data::statistic::StatisticCategory::Custom,
                    pumpkin_data::statistic::CustomStatistic::InteractWithLoom as i32,
                    1,
                )
                .await;
            args.player
                .open_handled_screen(
                    &LoomScreenFactory {
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

struct LoomScreenFactory {
    position: pumpkin_util::math::position::BlockPos,
    world: Arc<crate::world::World>,
}

impl ScreenHandlerFactory for LoomScreenFactory {
    fn create_screen_handler<'a>(
        &'a self,
        sync_id: u8,
        player_inventory: &'a Arc<PlayerInventory>,
        _player: &'a dyn InventoryPlayer,
    ) -> BoxFuture<'a, Option<SharedScreenHandler>> {
        Box::pin(async move {
            let mut handler = LoomScreenHandler::new(sync_id, player_inventory);
            let pos = self.position;
            let world = self.world.clone();
            handler
                .get_behaviour_mut()
                .set_validity_check(move |player| {
                    let state_id = world.get_block_state(&pos).id;
                    let block = pumpkin_data::Block::from_state_id(state_id);
                    block == &pumpkin_data::Block::LOOM
                        && player.can_interact_with_block_at(&pos, 4.0)
                });
            let handler: SharedScreenHandler = Arc::new(Mutex::new(handler));
            Some(handler)
        })
    }

    fn get_display_name(&self) -> TextComponent {
        TextComponent::translate(translation::java::CONTAINER_LOOM, [])
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
    fn loom_block_id_parity() {
        assert_eq!(Block::LOOM.name, "loom");
    }

    #[test]
    fn loom_default_state_parity() {
        assert_ne!(Block::LOOM.default_state.id, Block::AIR.default_state.id);
    }

    #[test]
    fn loom_properties_roundtrip_parity() {
        for facing in [
            HorizontalFacing::North,
            HorizontalFacing::South,
            HorizontalFacing::East,
            HorizontalFacing::West,
        ] {
            let props = WallTorchLikeProperties { facing };
            let state_id = props.to_state_id(&Block::LOOM);
            let rt = WallTorchLikeProperties::from_state_id(state_id, &Block::LOOM);
            assert_eq!(rt.facing, facing);
        }
    }
}
