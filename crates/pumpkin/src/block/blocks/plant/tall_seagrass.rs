use pumpkin_data::{Block, BlockStateId, fluid::Fluid};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockAccessor;

use crate::block::{
    BlockBehaviour, BlockFuture, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    blocks::plant::{PlantBlockBase, seagrass::supports_seagrass},
};
#[pumpkin_block("minecraft:tall_seagrass")]
pub struct TallSeaGrassBlock;
impl BlockBehaviour for TallSeaGrassBlock {
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

impl PlantBlockBase for TallSeaGrassBlock {
    fn can_plant_on_top(
        &self,
        block_accessor: &dyn pumpkin_world::world::BlockAccessor,
        pos: &pumpkin_util::math::position::BlockPos,
    ) -> bool {
        let (support_block, support_block_state) = block_accessor.get_block_and_state(pos);
        let replacing_block = block_accessor.get_block(&pos.up());
        if replacing_block != &Block::WATER && replacing_block != &Block::TALL_SEAGRASS {
            return false;
        }

        if replacing_block == &Block::TALL_SEAGRASS {
            //only for blockupdate
            let block_above = block_accessor.get_block(&pos.up_height(2));
            let is_support_seagrass_block = support_block == &Block::TALL_SEAGRASS;
            let is_above_seagrass_block = block_above == &Block::TALL_SEAGRASS;
            match (is_support_seagrass_block, is_above_seagrass_block) {
                (true, true) | (false, false) => return false,
                _ => {}
            }
        }
        if support_block == &Block::TALL_SEAGRASS {
            return true;
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

#[cfg(test)]
mod tests {
    use pumpkin_data::Block;
    use pumpkin_data::block_properties::{
        BlockProperties, DoubleBlockHalf, TallSeagrassLikeProperties,
    };

    #[test]
    fn tall_seagrass_block_id_parity() {
        assert_eq!(Block::TALL_SEAGRASS.name, "tall_seagrass");
    }

    #[test]
    fn tall_seagrass_properties_parity() {
        for half in [DoubleBlockHalf::Lower, DoubleBlockHalf::Upper] {
            let props = TallSeagrassLikeProperties { half };
            let state_id = props.to_state_id(&Block::TALL_SEAGRASS);
            let roundtrip =
                TallSeagrassLikeProperties::from_state_id(state_id, &Block::TALL_SEAGRASS);
            assert_eq!(roundtrip.half, half);
        }
    }
}
