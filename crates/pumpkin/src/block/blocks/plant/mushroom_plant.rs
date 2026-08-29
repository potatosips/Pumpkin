use pumpkin_data::tag::Taggable;
use pumpkin_data::{BlockId, BlockState, BlockStateId, tag};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockAccessor;

use crate::block::{
    BlockBehaviour, BlockFuture, BlockMetadata, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    blocks::plant::PlantBlockBase,
};

pub struct MushroomPlantBlock;

impl MushroomPlantBlock {
    #[must_use]
    const fn may_place_on(state: &BlockState) -> bool {
        state.is_solid() && (state.is_full_cube() || state.is_solid_block())
    }

    fn can_survive(
        block_accessor: &dyn BlockAccessor,
        world: Option<&crate::world::World>,
        pos: &BlockPos,
    ) -> bool {
        let below_pos = pos.down();
        if block_accessor
            .get_block(&below_pos)
            .has_tag(&tag::Block::MINECRAFT_OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT)
        {
            return true;
        }

        world.is_none_or(|world| world.get_max_local_raw_brightness(pos) < 13)
            && Self::may_place_on(block_accessor.get_block_state(&below_pos))
    }
}

impl BlockMetadata for MushroomPlantBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::BROWN_MUSHROOM, BlockId::RED_MUSHROOM].into()
    }
}

impl BlockBehaviour for MushroomPlantBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        Self::can_survive(args.block_accessor, args.world, args.position)
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if Self::can_survive(args.world, Some(args.world), args.position) {
                args.state_id
            } else {
                pumpkin_data::Block::AIR.default_state.id
            }
        })
    }
}

impl PlantBlockBase for MushroomPlantBlock {
    fn can_plant_on_top(&self, block_accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        Self::may_place_on(block_accessor.get_block_state(pos))
    }

    fn can_place_at(&self, block_accessor: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
        Self::can_survive(block_accessor, None, block_pos)
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

    #[test]
    fn ordinary_mushroom_support_requires_a_solid_rendering_surface() {
        assert!(MushroomPlantBlock::may_place_on(Block::STONE.default_state));
        assert!(!MushroomPlantBlock::may_place_on(Block::AIR.default_state));
        assert!(!MushroomPlantBlock::may_place_on(
            Block::WATER.default_state
        ));
    }
}
