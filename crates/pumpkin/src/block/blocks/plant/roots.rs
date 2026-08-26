use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockId, BlockStateId, tag};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockAccessor;

use crate::block::BlockFuture;
use crate::block::{BlockBehaviour, BlockMetadata, CanPlaceAtArgs, GetStateForNeighborUpdateArgs};

pub struct RootsBlock;

impl BlockMetadata for RootsBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::WARPED_ROOTS, BlockId::CRIMSON_ROOTS].into()
    }
}

impl BlockBehaviour for RootsBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_roots_at(args.block_accessor, args.position, args.block)
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if !can_place_roots_at(args.world, args.position, args.block) {
                return Block::AIR.default_state.id;
            }
            args.state_id
        })
    }
}

fn can_place_roots_at(block_accessor: &dyn BlockAccessor, pos: &BlockPos, roots: &Block) -> bool {
    let block_below = block_accessor.get_block(&pos.down());
    if roots == &Block::WARPED_ROOTS {
        block_below.has_tag(&tag::Block::MINECRAFT_SUPPORTS_WARPED_ROOTS)
    } else {
        block_below.has_tag(&tag::Block::MINECRAFT_SUPPORTS_CRIMSON_ROOTS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn roots_block_id_parity() {
        assert_eq!(Block::CRIMSON_ROOTS.name, "crimson_roots");
        assert_eq!(Block::WARPED_ROOTS.name, "warped_roots");
        assert_eq!(
            RootsBlock::ids().as_ref(),
            &[BlockId::WARPED_ROOTS, BlockId::CRIMSON_ROOTS]
        );
    }

    #[test]
    fn roots_default_state_parity() {
        assert_ne!(
            Block::CRIMSON_ROOTS.default_state.id,
            Block::AIR.default_state.id
        );
        assert_ne!(
            Block::WARPED_ROOTS.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn roots_supports_tag_parity() {
        assert!(Block::CRIMSON_NYLIUM.has_tag(&tag::Block::MINECRAFT_SUPPORTS_CRIMSON_ROOTS));
        assert!(Block::WARPED_NYLIUM.has_tag(&tag::Block::MINECRAFT_SUPPORTS_WARPED_ROOTS));
        assert!(Block::SOUL_SOIL.has_tag(&tag::Block::MINECRAFT_SUPPORTS_CRIMSON_ROOTS));
        assert!(Block::SOUL_SOIL.has_tag(&tag::Block::MINECRAFT_SUPPORTS_WARPED_ROOTS));
        assert!(Block::DIRT.has_tag(&tag::Block::MINECRAFT_SUPPORTS_CRIMSON_ROOTS));
        assert!(Block::DIRT.has_tag(&tag::Block::MINECRAFT_SUPPORTS_WARPED_ROOTS));
    }
}
