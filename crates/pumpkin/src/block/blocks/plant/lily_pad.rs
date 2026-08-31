use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockState, BlockStateId, fluid::Fluid};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

use crate::block::{BlockFuture, GetStateForNeighborUpdateArgs, blocks::plant::PlantBlockBase};

use crate::block::{BlockBehaviour, CanPlaceAtArgs, OnEntityCollisionArgs};

#[pumpkin_block("minecraft:lily_pad")]
pub struct LilyPadBlock;

impl BlockBehaviour for LilyPadBlock {
    fn on_entity_collision<'a>(&'a self, args: OnEntityCollisionArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            // Proberbly not the best solution, but works
            if args
                .entity
                .get_entity()
                .entity_type
                .resource_name
                .ends_with("_boat")
            {
                args.world
                    .break_block(args.position, None, BlockFlags::empty())
                    .await;
            }
        })
    }

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

impl PlantBlockBase for LilyPadBlock {
    fn can_plant_on_top(&self, block_accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        let (block, state) = block_accessor.get_block_and_state(pos);
        supports_lily_pad(block, state)
    }
}

fn supports_lily_pad(block: &Block, state: &BlockState) -> bool {
    if block.has_tag(&tag::Block::MINECRAFT_SUPPORTS_LILY_PAD) {
        return true;
    }

    if let Some(fluid) = Fluid::from_state_id(state.id) {
        return fluid.has_tag(&tag::Fluid::MINECRAFT_SUPPORTS_LILY_PAD)
            && fluid.is_source(state.id);
    }

    // Waterlogged states carry a source-water fluid even though their block-state
    // ID is owned by the block rather than the generated fluid registry.
    block.properties(state.id).is_some_and(|properties| {
        properties
            .to_props()
            .iter()
            .any(|(name, value)| *name == "waterlogged" && *value == "true")
    })
}

#[cfg(test)]
mod tests {
    use super::supports_lily_pad;
    use pumpkin_data::tag::{self, Taggable};
    use pumpkin_data::{
        Block,
        block_properties::{BlockProperties, OakLeavesLikeProperties},
        fluid::Fluid,
    };

    #[test]
    fn lily_pad_block_id_parity() {
        assert_eq!(Block::LILY_PAD.name, "lily_pad");
    }

    #[test]
    fn lily_pad_default_state_parity() {
        // Lily pad has no properties (single state)
        assert_ne!(
            Block::LILY_PAD.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn lily_pad_supports_tag_parity() {
        // Water should support lily pad via fluid tag
        assert!(
            Block::WATER.has_tag(&tag::Fluid::MINECRAFT_SUPPORTS_LILY_PAD)
                || Block::WATER.has_tag(&tag::Block::MINECRAFT_SUPPORTS_LILY_PAD)
        );
    }

    #[test]
    fn only_source_water_or_ice_supports_lily_pads() {
        assert!(supports_lily_pad(&Block::WATER, Block::WATER.default_state));
        assert!(supports_lily_pad(&Block::ICE, Block::ICE.default_state));

        let flowing_state = Fluid::FLOWING_WATER.states[0].block_state_id.to_state();
        assert!(!supports_lily_pad(
            Block::from_state_id(flowing_state.id),
            flowing_state
        ));
        assert!(!supports_lily_pad(
            &Block::STONE,
            Block::STONE.default_state
        ));
    }

    #[test]
    fn waterlogged_source_state_supports_lily_pads() {
        let mut props = OakLeavesLikeProperties::default(&Block::OAK_LEAVES);
        props.waterlogged = true;
        let state = props.to_state_id(&Block::OAK_LEAVES).to_state();
        assert!(supports_lily_pad(&Block::OAK_LEAVES, state));
    }
}
