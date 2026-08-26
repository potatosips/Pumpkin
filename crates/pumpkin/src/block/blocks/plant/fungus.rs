use crate::block::{
    BlockBehaviour, BlockFuture, BlockMetadata, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
};
use pumpkin_data::BlockStateId;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockId, tag};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockAccessor;

pub struct FungusBlock;

impl BlockMetadata for FungusBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::CRIMSON_FUNGUS, BlockId::WARPED_FUNGUS].into()
    }
}

impl BlockBehaviour for FungusBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_fungus_at(args.block_accessor, args.position, args.block)
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if !can_place_fungus_at(args.world, args.position, args.block) {
                return Block::AIR.default_state.id;
            }
            args.state_id
        })
    }
}

fn can_place_fungus_at(block_accessor: &dyn BlockAccessor, pos: &BlockPos, fungus: &Block) -> bool {
    let block_below = block_accessor.get_block(&pos.down());
    if fungus == &Block::WARPED_FUNGUS {
        block_below.has_tag(&tag::Block::MINECRAFT_SUPPORTS_WARPED_FUNGUS)
    } else {
        block_below.has_tag(&tag::Block::MINECRAFT_SUPPORTS_CRIMSON_FUNGUS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn fungus_block_id_parity() {
        assert_eq!(Block::CRIMSON_FUNGUS.name, "crimson_fungus");
        assert_eq!(Block::WARPED_FUNGUS.name, "warped_fungus");
        assert_eq!(
            FungusBlock::ids().as_ref(),
            &[BlockId::CRIMSON_FUNGUS, BlockId::WARPED_FUNGUS]
        );
    }

    #[test]
    fn fungus_default_state_parity() {
        assert_ne!(
            Block::CRIMSON_FUNGUS.default_state.id,
            Block::AIR.default_state.id
        );
        assert_ne!(
            Block::WARPED_FUNGUS.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn fungus_supports_tag_parity() {
        assert!(Block::CRIMSON_NYLIUM.has_tag(&tag::Block::MINECRAFT_SUPPORTS_CRIMSON_FUNGUS));
        assert!(Block::WARPED_NYLIUM.has_tag(&tag::Block::MINECRAFT_SUPPORTS_WARPED_FUNGUS));
        assert!(Block::SOUL_SOIL.has_tag(&tag::Block::MINECRAFT_SUPPORTS_CRIMSON_FUNGUS));
        assert!(Block::SOUL_SOIL.has_tag(&tag::Block::MINECRAFT_SUPPORTS_WARPED_FUNGUS));
        assert!(Block::DIRT.has_tag(&tag::Block::MINECRAFT_SUPPORTS_CRIMSON_FUNGUS));
        assert!(Block::DIRT.has_tag(&tag::Block::MINECRAFT_SUPPORTS_WARPED_FUNGUS));
    }
}
