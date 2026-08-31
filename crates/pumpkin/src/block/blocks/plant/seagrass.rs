use pumpkin_data::{
    Block, BlockDirection, BlockState, BlockStateId,
    block_properties::{
        BlockProperties, DoubleBlockHalf, TallSeagrassLikeProperties, WaterLikeProperties,
    },
    fluid::Fluid,
    tag::{self, Taggable},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockAccessor;
use pumpkin_world::world::BlockFlags;

use crate::block::{
    BlockBehaviour, BlockFuture, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    blocks::plant::PlantBlockBase,
};
#[pumpkin_block("minecraft:seagrass")]
pub struct SeaGrassBlock;
impl BlockBehaviour for SeaGrassBlock {
    fn is_valid_bonemeal_target(&self, args: crate::block::BonemealArgs<'_>) -> bool {
        let above = args.position.up();
        if !args.world.is_in_height_limit(above.0.y) || !args.world.is_loaded(&above) {
            return false;
        }
        let (above_block, above_state) = args.world.get_block_and_state(&above);
        above_block == &Block::WATER
            && WaterLikeProperties::from_state_id(above_state.id, above_block).level == 0
    }

    fn perform_bonemeal<'a>(&'a self, args: crate::block::BonemealArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let lower = Block::TALL_SEAGRASS.default_state.id;
            args.world
                .set_block_state(args.position, lower, BlockFlags::NOTIFY_LISTENERS)
                .await;
            let mut props = TallSeagrassLikeProperties::from_state_id(lower, &Block::TALL_SEAGRASS);
            props.half = DoubleBlockHalf::Upper;
            args.world
                .set_block_state(
                    &args.position.up(),
                    props.to_state_id(&Block::TALL_SEAGRASS),
                    BlockFlags::NOTIFY_LISTENERS,
                )
                .await;
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
            let state = <Self as PlantBlockBase>::get_state_for_neighbor_update(
                self,
                args.world,
                args.position,
                args.state_id,
            )
            .await;
            if state != Block::WATER.default_state.id {
                args.world.schedule_fluid_tick(
                    &Fluid::WATER,
                    *args.position,
                    Fluid::WATER.flow_speed as u8,
                    TickPriority::Normal,
                );
            }
            state
        })
    }
}

impl PlantBlockBase for SeaGrassBlock {
    fn can_plant_on_top(
        &self,
        block_accessor: &dyn pumpkin_world::world::BlockAccessor,
        pos: &pumpkin_util::math::position::BlockPos,
    ) -> bool {
        let (support_block, support_block_state) = block_accessor.get_block_and_state(pos);
        let replacing_block = block_accessor.get_block(&pos.up());
        if replacing_block != &Block::WATER && replacing_block != &Block::SEAGRASS {
            return false;
        }
        if supports_seagrass(support_block, support_block_state) {
            return true;
        }
        false
    }
    async fn get_state_for_neighbor_update(
        &self,
        block_accessor: &dyn BlockAccessor,
        block_pos: &BlockPos,
        block_state: BlockStateId,
    ) -> BlockStateId {
        if !<Self as PlantBlockBase>::can_place_at(self, block_accessor, block_pos) {
            return Block::WATER.default_state.id;
        }
        block_state
    }
}
pub fn supports_seagrass(support_block: &Block, support_block_state: &BlockState) -> bool {
    support_block_state.is_side_solid(BlockDirection::Up)
        && !support_block.has_tag(&tag::Block::MINECRAFT_CANNOT_SUPPORT_SEAGRASS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn seagrass_block_id_parity() {
        assert_eq!(Block::SEAGRASS.name, "seagrass");
    }

    #[test]
    fn seagrass_default_state_parity() {
        assert_ne!(
            Block::SEAGRASS.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn seagrass_supports_parity() {
        assert!(supports_seagrass(&Block::DIRT, &Block::DIRT.default_state));
        assert!(supports_seagrass(&Block::SAND, &Block::SAND.default_state));
        assert!(supports_seagrass(
            &Block::STONE,
            &Block::STONE.default_state
        ));
        assert!(supports_seagrass(
            &Block::GRAVEL,
            &Block::GRAVEL.default_state
        ));
    }

    #[test]
    fn seagrass_bonemeal_result_has_both_tall_halves() {
        let lower = Block::TALL_SEAGRASS.default_state.id;
        let mut props = TallSeagrassLikeProperties::from_state_id(lower, &Block::TALL_SEAGRASS);
        assert_eq!(props.half, DoubleBlockHalf::Lower);
        props.half = DoubleBlockHalf::Upper;
        assert_eq!(
            TallSeagrassLikeProperties::from_state_id(
                props.to_state_id(&Block::TALL_SEAGRASS),
                &Block::TALL_SEAGRASS,
            )
            .half,
            DoubleBlockHalf::Upper
        );
    }

    #[test]
    fn only_source_water_is_valid_above_seagrass_for_bonemeal() {
        let source = WaterLikeProperties { level: 0 }.to_state_id(&Block::WATER);
        let flowing = WaterLikeProperties { level: 1 }.to_state_id(&Block::WATER);
        assert_eq!(
            WaterLikeProperties::from_state_id(source, &Block::WATER).level,
            0
        );
        assert_ne!(
            WaterLikeProperties::from_state_id(flowing, &Block::WATER).level,
            0
        );
    }
}
