use crate::block::blocks::falling::FallingBlock;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, BlockFuture, GetStateForNeighborUpdateArgs, NormalUseArgs,
    OnNeighborUpdateArgs, OnPlaceArgs, OnScheduledTickArgs, PlacedArgs,
};

use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{BlockProperties, WallTorchLikeProperties};
use pumpkin_data::translation;
use pumpkin_inventory::anvil::AnvilScreenHandler;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{
    BoxFuture, InventoryPlayer, ScreenHandler, ScreenHandlerFactory, SharedScreenHandler,
};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::text::TextComponent;
use pumpkin_world::inventory::SimpleInventory;
use std::sync::Arc;
use tokio::sync::Mutex;

#[pumpkin_block_from_tag("minecraft:anvil")]
pub struct AnvilBlock;

impl BlockBehaviour for AnvilBlock {
    fn normal_use<'a>(&'a self, args: NormalUseArgs<'a>) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async move {
            args.player
                .increment_stat(
                    pumpkin_data::statistic::StatisticCategory::Custom,
                    pumpkin_data::statistic::CustomStatistic::InteractWithAnvil as i32,
                    1,
                )
                .await;
            args.player
                .open_handled_screen(
                    &AnvilScreenFactory {
                        position: *args.position,
                        world: args.world.clone(),
                    },
                    Some(*args.position),
                )
                .await;

            BlockActionResult::Success
        })
    }

    fn placed<'a>(&'a self, args: PlacedArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            FallingBlock::placed(&FallingBlock, args).await;
        })
    }

    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let dir = args
                .player
                .living_entity
                .entity
                .get_horizontal_facing()
                .rotate_clockwise();

            let mut props = WallTorchLikeProperties::default(args.block);

            props.facing = dir;
            props.to_state_id(args.block)
        })
    }

    fn on_scheduled_tick<'a>(&'a self, args: OnScheduledTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            FallingBlock::on_scheduled_tick(&FallingBlock, args).await;
        })
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(
            async move { FallingBlock::get_state_for_neighbor_update(&FallingBlock, args).await },
        )
    }

    fn on_neighbor_update<'a>(&'a self, args: OnNeighborUpdateArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move { FallingBlock::on_neighbor_update(&FallingBlock, args).await })
    }
}

struct AnvilScreenFactory {
    position: pumpkin_util::math::position::BlockPos,
    world: Arc<crate::world::World>,
}

impl ScreenHandlerFactory for AnvilScreenFactory {
    fn create_screen_handler<'a>(
        &'a self,
        sync_id: u8,
        player_inventory: &'a Arc<PlayerInventory>,
        _player: &'a dyn InventoryPlayer,
    ) -> BoxFuture<'a, Option<SharedScreenHandler>> {
        Box::pin(async move {
            let inventory = Arc::new(SimpleInventory::new(3));
            let mut handler = AnvilScreenHandler::new(sync_id, player_inventory, inventory);
            let pos = self.position;
            let world = self.world.clone();
            handler
                .get_behaviour_mut()
                .set_validity_check(move |player| {
                    let state_id = world.get_block_state(&pos).id;
                    let block = pumpkin_data::Block::from_state_id(state_id);
                    (block == &pumpkin_data::Block::ANVIL
                        || block == &pumpkin_data::Block::CHIPPED_ANVIL
                        || block == &pumpkin_data::Block::DAMAGED_ANVIL)
                        && player.can_interact_with_block_at(&pos, 4.0)
                });
            let concrete_arc = Arc::new(Mutex::new(handler));

            Some(concrete_arc as SharedScreenHandler)
        })
    }

    fn get_display_name(&self) -> TextComponent {
        pumpkin_macros::translate_cross!(
            translation::java::CONTAINER_REPAIR,
            translation::bedrock::CONTAINER_REPAIR
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
    fn anvil_ids_parity() {
        assert_eq!(Block::ANVIL.name, "anvil");
        assert_eq!(Block::CHIPPED_ANVIL.name, "chipped_anvil");
        assert_eq!(Block::DAMAGED_ANVIL.name, "damaged_anvil");
    }

    #[test]
    fn anvil_default_state_parity() {
        assert_ne!(Block::ANVIL.default_state.id, Block::AIR.default_state.id);
        assert_ne!(
            Block::CHIPPED_ANVIL.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn anvil_properties_roundtrip_parity() {
        for facing in [
            HorizontalFacing::North,
            HorizontalFacing::South,
            HorizontalFacing::East,
            HorizontalFacing::West,
        ] {
            let props = WallTorchLikeProperties { facing };
            let state_id = props.to_state_id(&Block::ANVIL);
            let rt = WallTorchLikeProperties::from_state_id(state_id, &Block::ANVIL);
            assert_eq!(rt.facing, facing);
        }
    }
}
