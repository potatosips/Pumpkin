use pumpkin_data::BlockStateId;
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use rand::RngExt;

use crate::block::{
    BlockBehaviour, BlockFuture, BonemealArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs,
    OnScheduledTickArgs, RandomTickArgs,
    blocks::{leaves::LeavesBlock, plant::mangrove_propagule::MangrovePropaguleBlock},
};

#[pumpkin_block("minecraft:mangrove_leaves")]
pub struct MangroveLeavesBlock;

impl MangroveLeavesBlock {
    fn can_grow_propagule(
        world: &dyn BlockAccessor,
        position: &pumpkin_util::math::position::BlockPos,
    ) -> bool {
        world.get_block_state(&position.down()).is_air()
    }
}

impl BlockBehaviour for MangroveLeavesBlock {
    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        LeavesBlock.on_place(args)
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        LeavesBlock.get_state_for_neighbor_update(args)
    }

    fn on_scheduled_tick<'a>(&'a self, args: OnScheduledTickArgs<'a>) -> BlockFuture<'a, ()> {
        LeavesBlock.on_scheduled_tick(args)
    }

    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            LeavesBlock
                .random_tick(RandomTickArgs {
                    world: args.world,
                    block: args.block,
                    position: args.position,
                })
                .await;

            if rand::rng().random_ratio(1, 10)
                && Self::can_grow_propagule(&**args.world, args.position)
            {
                args.world
                    .set_block_state(
                        &args.position.down(),
                        MangrovePropaguleBlock::create_new_hanging_propagule(0),
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;
            }
        })
    }

    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        Self::can_grow_propagule(&**args.world, args.position)
    }

    fn is_bonemeal_success(&self, _args: BonemealArgs<'_>) -> bool {
        true
    }

    fn perform_bonemeal<'a>(&'a self, args: BonemealArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if Self::can_grow_propagule(&**args.world, args.position) {
                args.world
                    .set_block_state(
                        &args.position.down(),
                        MangrovePropaguleBlock::create_new_hanging_propagule(0),
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::{
        Block,
        block_properties::{BlockProperties, MangrovePropaguleLikeProperties},
        tag,
        tag::Taggable,
    };

    #[test]
    fn exact_handler_blocks_are_vanilla_tagged() {
        assert!(Block::MANGROVE_LEAVES.has_tag(&tag::Block::MINECRAFT_LEAVES));
        assert!(
            Block::MANGROVE_LEAVES
                .has_tag(&tag::Block::MINECRAFT_SUPPORTS_HANGING_MANGROVE_PROPAGULE)
        );
    }

    #[test]
    fn bonemeal_creates_young_hanging_propagule_state() {
        let state = MangrovePropaguleBlock::create_new_hanging_propagule(0);
        let props =
            MangrovePropaguleLikeProperties::from_state_id(state, &Block::MANGROVE_PROPAGULE);
        assert!(props.hanging);
        assert_eq!(props.age, 0);
        assert_eq!(props.stage, 0);
        assert!(!props.waterlogged);
    }
}
