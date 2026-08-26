use crate::block::{BlockBehaviour, BlockFuture, CanPlaceAtArgs};
use crate::block::{GetStateForNeighborUpdateArgs, blocks::plant::PlantBlockBase};
use pumpkin_data::BlockStateId;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, tag};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockAccessor;

#[pumpkin_block("minecraft:spore_blossom")]
pub struct SporeBlossomBlock;

impl BlockBehaviour for SporeBlossomBlock {
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
impl PlantBlockBase for SporeBlossomBlock {
    fn can_plant_on_top(
        &self,
        _block_accessor: &dyn pumpkin_world::world::BlockAccessor,
        _pos: &pumpkin_util::math::position::BlockPos,
    ) -> bool {
        false
    }
    fn can_place_at(&self, block_accessor: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
        let ceiling_block = block_accessor.get_block(&block_pos.up());
        supports_spore_blossom(ceiling_block)
    }
}
fn supports_spore_blossom(block: &Block) -> bool {
    !block.has_tag(&tag::Block::MINECRAFT_LEAVES) && block.is_solid()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn spore_blossom_block_id_parity() {
        assert_eq!(Block::SPORE_BLOSSOM.name, "spore_blossom");
    }

    #[test]
    fn spore_blossom_default_state_parity() {
        assert_ne!(
            Block::SPORE_BLOSSOM.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn spore_blossom_ceiling_support_parity() {
        assert!(supports_spore_blossom(&Block::STONE));
        assert!(supports_spore_blossom(&Block::DIRT));
        assert!(supports_spore_blossom(&Block::DEEPSLATE));
        assert!(!supports_spore_blossom(&Block::OAK_LEAVES));
        assert!(!supports_spore_blossom(&Block::AIR));
    }
}
