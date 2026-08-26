use pumpkin_data::BlockId;
use pumpkin_data::BlockStateId;

use crate::block::{
    BlockBehaviour, BlockFuture, BlockMetadata, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    blocks::plant::PlantBlockBase,
};

pub struct BushBlock;

impl BlockMetadata for BushBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::BUSH, BlockId::FIREFLY_BUSH].into()
    }
}

impl BlockBehaviour for BushBlock {
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

impl PlantBlockBase for BushBlock {}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn bush_block_ids_parity() {
        let ids = BushBlock::ids();
        assert!(ids.contains(&BlockId::BUSH));
        assert!(ids.contains(&BlockId::FIREFLY_BUSH));
    }

    #[test]
    fn bush_default_state_parity() {
        assert_eq!(Block::BUSH.name, "bush");
    }
}
