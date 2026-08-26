use crate::block::{BlockBehaviour, BlockFuture, BlockMetadata, OnPlaceArgs};
use pumpkin_data::{
    BlockId, BlockStateId,
    block_properties::{BlockProperties, SculkCatalystLikeProperties},
};

pub struct SculkCatalystBlock;

impl BlockMetadata for SculkCatalystBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::SCULK_CATALYST].into()
    }
}

impl BlockBehaviour for SculkCatalystBlock {
    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let mut props = SculkCatalystLikeProperties::default(args.block);
            props.bloom = false;
            props.to_state_id(args.block)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn sculk_catalyst_block_id_parity() {
        assert_eq!(Block::SCULK_CATALYST.name, "sculk_catalyst");
    }

    #[test]
    fn sculk_catalyst_default_state_parity() {
        assert_ne!(
            Block::SCULK_CATALYST.default_state.id,
            Block::AIR.default_state.id
        );
    }
}
