use pumpkin_data::{
    BlockId, BlockStateId,
    tag::{self, Taggable},
};
use rand::RngExt;

use crate::block::{
    BlockBehaviour, BlockFuture, BlockMetadata, BonemealArgs, CanPlaceAtArgs,
    GetStateForNeighborUpdateArgs,
    blocks::plant::{PlantBlockBase, sapling::SaplingBlock},
};

pub struct AzaleaBlock;

impl BlockMetadata for AzaleaBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::AZALEA, BlockId::FLOWERING_AZALEA].into()
    }
}

impl BlockBehaviour for AzaleaBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        args.block_accessor
            .get_block(&args.position.down())
            .has_tag(&tag::Block::MINECRAFT_SUPPORTS_AZALEA)
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if args
                .world
                .get_block(&args.position.down())
                .has_tag(&tag::Block::MINECRAFT_SUPPORTS_AZALEA)
            {
                args.state_id
            } else {
                BlockStateId::AIR
            }
        })
    }

    fn is_valid_bonemeal_target(&self, _args: BonemealArgs<'_>) -> bool {
        true
    }

    fn is_bonemeal_success(&self, _args: BonemealArgs<'_>) -> bool {
        rand::rng().random_bool(0.45)
    }

    fn perform_bonemeal<'a>(&'a self, args: BonemealArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            SaplingBlock
                .grow_tree(args.world, *args.position, args.block, true)
                .await;
        })
    }
}

impl PlantBlockBase for AzaleaBlock {}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn azalea_ids_and_support_tags() {
        assert_eq!(
            AzaleaBlock::ids().as_ref(),
            &[BlockId::AZALEA, BlockId::FLOWERING_AZALEA]
        );
        assert!(Block::MOSS_BLOCK.has_tag(&tag::Block::MINECRAFT_SUPPORTS_AZALEA));
        assert!(Block::DIRT.has_tag(&tag::Block::MINECRAFT_SUPPORTS_AZALEA));
    }
}
