use pumpkin_data::BlockStateId;
use pumpkin_macros::pumpkin_block;

use crate::block::blocks::plant::PlantBlockBase;
use crate::block::blocks::plant::crop::CropBlockBase;
use crate::block::{
    BlockBehaviour, BlockFuture, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, RandomTickArgs,
};

#[pumpkin_block("minecraft:wheat")]
pub struct WheatBlock;

impl BlockBehaviour for WheatBlock {
    fn is_valid_bonemeal_target(&self, args: crate::block::BonemealArgs<'_>) -> bool {
        <Self as CropBlockBase>::is_valid_bonemeal_target(self, args.world, args.position)
    }

    fn perform_bonemeal<'a>(&'a self, args: crate::block::BonemealArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            <Self as CropBlockBase>::perform_bonemeal(self, args.world, args.position).await;
        })
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as CropBlockBase>::can_plant_on_top(self, args.block_accessor, &args.position.down())
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

    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            <Self as CropBlockBase>::random_tick(self, args.world, args.position).await;
        })
    }
}

impl PlantBlockBase for WheatBlock {}

impl CropBlockBase for WheatBlock {}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;
    use pumpkin_data::block_properties::{BlockProperties, WheatLikeProperties};

    #[test]
    fn wheat_block_id_and_default_state_parity() {
        assert_eq!(Block::WHEAT.name, "wheat");
        let default_props =
            WheatLikeProperties::from_state_id(Block::WHEAT.default_state.id, &Block::WHEAT);
        assert_eq!(default_props.age, 0);
    }

    #[test]
    fn wheat_properties_encoding_decoding_parity() {
        for age in 0..=7 {
            let props = WheatLikeProperties { age };
            let state_id = props.to_state_id(&Block::WHEAT);
            let decoded = WheatLikeProperties::from_state_id(state_id, &Block::WHEAT);
            assert_eq!(decoded.age, age);
        }
    }
}
