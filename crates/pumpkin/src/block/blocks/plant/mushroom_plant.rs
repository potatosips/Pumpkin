use pumpkin_data::tag::Taggable;
use pumpkin_data::{BlockId, BlockStateId, tag};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockAccessor;

use crate::block::{
    BlockBehaviour, BlockFuture, BlockMetadata, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    blocks::plant::PlantBlockBase,
};

pub struct MushroomPlantBlock;

impl BlockMetadata for MushroomPlantBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::BROWN_MUSHROOM, BlockId::RED_MUSHROOM].into()
    }
}

impl BlockBehaviour for MushroomPlantBlock {
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

impl PlantBlockBase for MushroomPlantBlock {
    fn can_plant_on_top(&self, block_accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        let block = block_accessor.get_block(pos);
        block.has_tag(&tag::Block::MINECRAFT_OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT)
            || block.has_tag(&tag::Block::MINECRAFT_SUPPORTS_VEGETATION)
            || block.is_solid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn mushroom_block_id_parity() {
        assert_eq!(Block::BROWN_MUSHROOM.name, "brown_mushroom");
        assert_eq!(Block::RED_MUSHROOM.name, "red_mushroom");
        assert_eq!(
            MushroomPlantBlock::ids().as_ref(),
            &[BlockId::BROWN_MUSHROOM, BlockId::RED_MUSHROOM]
        );
    }

    #[test]
    fn mushroom_default_state_parity() {
        assert_ne!(
            Block::BROWN_MUSHROOM.default_state.id,
            Block::AIR.default_state.id
        );
        assert_ne!(
            Block::RED_MUSHROOM.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn mushroom_supports_tag_parity() {
        assert!(
            Block::MYCELIUM.has_tag(&tag::Block::MINECRAFT_OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT)
        );
        assert!(Block::PODZOL.has_tag(&tag::Block::MINECRAFT_OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT));
        assert!(
            Block::CRIMSON_NYLIUM
                .has_tag(&tag::Block::MINECRAFT_OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT)
        );
        assert!(
            Block::WARPED_NYLIUM
                .has_tag(&tag::Block::MINECRAFT_OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT)
        );
    }
}
