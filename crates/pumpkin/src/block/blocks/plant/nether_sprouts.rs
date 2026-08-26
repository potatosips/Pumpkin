use crate::block::{BlockBehaviour, BlockFuture, CanPlaceAtArgs};
use crate::block::{GetStateForNeighborUpdateArgs, blocks::plant::PlantBlockBase};
use pumpkin_data::BlockStateId;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_macros::pumpkin_block;
#[pumpkin_block("minecraft:nether_sprouts")]
pub struct NetherSproutsBlock;

impl BlockBehaviour for NetherSproutsBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
    }
    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            <Self as PlantBlockBase>::get_state_for_neighbor_update(
                self,
                args.world,
                args.position,
                args.state_id,
            )
            .await
        })
    }
}
impl PlantBlockBase for NetherSproutsBlock {
    fn can_plant_on_top(
        &self,
        block_accessor: &dyn pumpkin_world::world::BlockAccessor,
        pos: &pumpkin_util::math::position::BlockPos,
    ) -> bool {
        let block = block_accessor.get_block(pos);
        block.has_tag(&tag::Block::MINECRAFT_SUPPORTS_NETHER_SPROUTS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn nether_sprouts_block_id_parity() {
        assert_eq!(Block::NETHER_SPROUTS.name, "nether_sprouts");
    }

    #[test]
    fn nether_sprouts_default_state_parity() {
        assert_ne!(
            Block::NETHER_SPROUTS.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn nether_sprouts_supports_tag_parity() {
        assert!(Block::WARPED_NYLIUM.has_tag(&tag::Block::MINECRAFT_SUPPORTS_NETHER_SPROUTS));
        assert!(Block::SOUL_SOIL.has_tag(&tag::Block::MINECRAFT_SUPPORTS_NETHER_SPROUTS));
        assert!(Block::DIRT.has_tag(&tag::Block::MINECRAFT_SUPPORTS_NETHER_SPROUTS));
    }
}
