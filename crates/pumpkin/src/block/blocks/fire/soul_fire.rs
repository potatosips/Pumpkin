use pumpkin_data::BlockStateId;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, tag};
use pumpkin_macros::pumpkin_block;

use crate::block::{
    BlockBehaviour, BlockFuture, BrokenArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
};

use super::FireBlockBase;
use crate::block::OnEntityCollisionArgs;

#[pumpkin_block("minecraft:soul_fire")]
pub struct SoulFireBlock;

impl SoulFireBlock {
    #[must_use]
    pub fn is_soul_base(block: &Block) -> bool {
        block.has_tag(&tag::Block::MINECRAFT_SOUL_FIRE_BASE_BLOCKS)
    }
}

impl BlockBehaviour for SoulFireBlock {
    fn on_entity_collision<'a>(&'a self, args: OnEntityCollisionArgs<'a>) -> BlockFuture<'a, ()> {
        FireBlockBase::apply_fire_collision(args, true)
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if !Self::is_soul_base(args.world.get_block(&args.position.down())) {
                return Block::AIR.default_state.id;
            }

            args.state_id
        })
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        Self::is_soul_base(args.block_accessor.get_block(&args.position.down()))
    }

    fn broken<'a>(&'a self, args: BrokenArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            FireBlockBase::broken(args.world, *args.position);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;
    use pumpkin_data::tag::{self, Taggable};

    #[test]
    fn soul_fire_block_id_parity() {
        assert_eq!(Block::SOUL_FIRE.name, "soul_fire");
    }

    #[test]
    fn soul_fire_default_state_parity() {
        assert_ne!(
            Block::SOUL_FIRE.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn soul_fire_supports_tag_parity() {
        assert!(Block::SOUL_SAND.has_tag(&tag::Block::MINECRAFT_SOUL_FIRE_BASE_BLOCKS));
        assert!(Block::SOUL_SOIL.has_tag(&tag::Block::MINECRAFT_SOUL_FIRE_BASE_BLOCKS));
        assert!(!Block::DIRT.has_tag(&tag::Block::MINECRAFT_SOUL_FIRE_BASE_BLOCKS));
    }
}
