use crate::block::blocks::plant::PlantBlockBase;
use crate::block::{
    BlockBehaviour, BlockFuture, BlockMetadata, BonemealArgs, BrokenArgs, CanPlaceAtArgs,
    GetStateForNeighborUpdateArgs, PlacedArgs, RandomTickArgs,
};
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{BlockProperties, KelpLikeProperties};
use pumpkin_data::{Block, BlockId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use rand::RngExt;

pub struct TwistingVinesBlock;
impl BlockMetadata for TwistingVinesBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::TWISTING_VINES, BlockId::TWISTING_VINES_PLANT].into()
    }
}

impl BlockBehaviour for TwistingVinesBlock {
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        let destination = args.position.up();
        args.block == &Block::TWISTING_VINES
            && args.world.is_in_height_limit(destination.0.y)
            && args.world.is_loaded(&destination)
            && args.world.get_block_state(&destination).is_air()
    }

    fn perform_bonemeal<'a>(&'a self, args: BonemealArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move { grow_head(args.world, args.position, args.block).await })
    }

    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if args.block != &Block::TWISTING_VINES {
                return;
            }
            let age = KelpLikeProperties::from_state_id(
                args.world.get_block_state_id(args.position),
                args.block,
            )
            .age;
            if natural_growth_succeeds(age, rand::rng().random::<f64>()) {
                grow_head(args.world, args.position, args.block).await;
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
    fn placed<'a>(&'a self, args: PlacedArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let support_pos = args.position.down();
            let support_block = args.world.get_block(&support_pos);
            if support_block == &Block::TWISTING_VINES {
                args.world
                    .set_block_state(
                        &support_pos,
                        Block::TWISTING_VINES_PLANT.default_state.id,
                        BlockFlags::empty(),
                    )
                    .await;
            }
        })
    }
    fn broken<'a>(&'a self, args: BrokenArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let support_pos = args.position.down();
            let support_block = args.world.get_block(&support_pos);
            if support_block == &Block::TWISTING_VINES_PLANT {
                args.world
                    .set_block_state(
                        &support_pos,
                        Block::TWISTING_VINES.default_state.id,
                        BlockFlags::empty(),
                    )
                    .await;
            }
        })
    }
}

impl PlantBlockBase for TwistingVinesBlock {
    fn can_plant_on_top(
        &self,
        block_accessor: &dyn pumpkin_world::world::BlockAccessor,
        pos: &pumpkin_util::math::position::BlockPos,
    ) -> bool {
        // Determine support block
        let support_pos = pos;
        let (support_block, support_block_state) = block_accessor.get_block_and_state(support_pos);

        if support_block == &Block::TWISTING_VINES || support_block == &Block::TWISTING_VINES_PLANT
        {
            return true;
        }
        if support_block_state.is_side_solid(pumpkin_data::BlockDirection::Up) {
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
            return Block::AIR.default_state.id;
        }
        block_state
    }
}

async fn grow_head(
    world: &std::sync::Arc<crate::world::World>,
    position: &BlockPos,
    block: &Block,
) {
    let destination = position.up();
    if block != &Block::TWISTING_VINES || !world.get_block_state(&destination).is_air() {
        return;
    }
    let mut props = KelpLikeProperties::from_state_id(world.get_block_state_id(position), block);
    props.age = props.age.saturating_add(1).min(25);
    world
        .set_block_state(
            position,
            Block::TWISTING_VINES_PLANT.default_state.id,
            BlockFlags::NOTIFY_ALL,
        )
        .await;
    world
        .set_block_state(
            &destination,
            props.to_state_id(&Block::TWISTING_VINES),
            BlockFlags::NOTIFY_ALL,
        )
        .await;
}

fn natural_growth_succeeds(age: u8, roll: f64) -> bool {
    age < 25 && roll < 0.1
}

#[cfg(test)]
mod tests {
    use super::{TwistingVinesBlock, natural_growth_succeeds};
    use crate::block::BlockMetadata;
    use pumpkin_data::Block;
    use pumpkin_data::BlockId;

    #[test]
    fn twisting_vines_block_id_parity() {
        assert_eq!(Block::TWISTING_VINES.name, "twisting_vines");
        assert_eq!(Block::TWISTING_VINES_PLANT.name, "twisting_vines_plant");
        assert_eq!(
            TwistingVinesBlock::ids().as_ref(),
            &[BlockId::TWISTING_VINES, BlockId::TWISTING_VINES_PLANT]
        );
    }

    #[test]
    fn twisting_vines_default_state_parity() {
        assert_ne!(
            Block::TWISTING_VINES.default_state.id,
            Block::AIR.default_state.id
        );
        assert_ne!(
            Block::TWISTING_VINES_PLANT.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn twisting_vines_growth_probability_and_age_cap() {
        assert!(natural_growth_succeeds(24, 0.099_999));
        assert!(!natural_growth_succeeds(24, 0.1));
        assert!(!natural_growth_succeeds(25, 0.0));
    }
}
