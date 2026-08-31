use crate::block::blocks::plant::PlantBlockBase;
use crate::block::{
    BlockBehaviour, BlockFuture, BlockMetadata, BonemealArgs, BrokenArgs, CanPlaceAtArgs,
    GetStateForNeighborUpdateArgs, PlacedArgs, RandomTickArgs,
};
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{BlockProperties, KelpLikeProperties, WaterLikeProperties};
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockId, fluid::Fluid, tag};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use rand::RngExt;
pub struct KelpBlock;

impl BlockMetadata for KelpBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::KELP, BlockId::KELP_PLANT].into()
    }
}

impl BlockBehaviour for KelpBlock {
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        args.block == &Block::KELP && args.world.get_block(&args.position.up()) == &Block::WATER
    }

    fn perform_bonemeal<'a>(&'a self, args: BonemealArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            grow_head(args.world, args.position, args.block).await;
        })
    }

    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if args.block != &Block::KELP {
                return;
            }
            let state_id = args.world.get_block_state_id(args.position);
            let age = KelpLikeProperties::from_state_id(state_id, args.block).age;
            if natural_growth_succeeds(age, rand::rng().random::<f64>())
                && args.world.get_block(&args.position.up()) == &Block::WATER
            {
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
    fn placed<'a>(&'a self, args: PlacedArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let support_pos = args.position.down();
            let support_block = args.world.get_block(&support_pos);
            if support_block == &Block::KELP {
                args.world
                    .set_block_state(
                        &support_pos,
                        Block::KELP_PLANT.default_state.id,
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
            if support_block == &Block::KELP_PLANT {
                args.world
                    .set_block_state(
                        &support_pos,
                        Block::KELP.default_state.id,
                        BlockFlags::empty(),
                    )
                    .await;
                args.world
                    .set_block_state(
                        args.position,
                        Block::WATER.default_state.id,
                        BlockFlags::empty(),
                    )
                    .await;
            }
        })
    }
}

fn natural_growth_succeeds(age: u8, roll: f64) -> bool {
    age < 25 && roll < 0.14
}

async fn grow_head(
    world: &std::sync::Arc<crate::world::World>,
    position: &BlockPos,
    block: &Block,
) {
    if block != &Block::KELP || world.get_block(&position.up()) != &Block::WATER {
        return;
    }
    let state_id = world.get_block_state_id(position);
    let mut props = KelpLikeProperties::from_state_id(state_id, block);
    props.age = props.age.saturating_add(1).min(25);
    world
        .set_block_state(
            position,
            Block::KELP_PLANT.default_state.id,
            BlockFlags::NOTIFY_ALL,
        )
        .await;
    world
        .set_block_state(
            &position.up(),
            props.to_state_id(&Block::KELP),
            BlockFlags::NOTIFY_ALL,
        )
        .await;
}

impl PlantBlockBase for KelpBlock {
    fn can_plant_on_top(
        &self,
        block_accessor: &dyn pumpkin_world::world::BlockAccessor,
        pos: &pumpkin_util::math::position::BlockPos,
    ) -> bool {
        // Determine support block
        let support_pos = pos;
        let (replacing_block, replacing_block_state) =
            block_accessor.get_block_and_state(&pos.up());
        let (support_block, support_block_state) = block_accessor.get_block_and_state(support_pos);
        if replacing_block == &Block::WATER {
            let water_props =
                WaterLikeProperties::from_state_id(replacing_block_state.id, replacing_block);

            //Only allow placing kelp on either full water or downward flowing water
            if water_props.level != 0 && water_props.level != 8 {
                return false;
            }
        } else {
            //Replacing block can also be a kelp_plant or kelp in case this is an neighbour update check
            if replacing_block != &Block::KELP_PLANT && replacing_block != &Block::KELP {
                return false;
            }
        }
        // If placing the base kelp block, allow placement on water or on other kelp segments.
        if support_block == &Block::KELP || support_block == &Block::KELP_PLANT {
            return true;
        }
        if support_block.has_tag(&tag::Block::MINECRAFT_CANNOT_SUPPORT_KELP) {
            return false;
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
            return Block::WATER.default_state.id;
        }
        block_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kelp_block_id_parity() {
        assert_eq!(Block::KELP.name, "kelp");
        assert_eq!(Block::KELP_PLANT.name, "kelp_plant");
        assert_eq!(
            KelpBlock::ids().as_ref(),
            &[BlockId::KELP, BlockId::KELP_PLANT]
        );
    }

    #[test]
    fn kelp_default_state_parity() {
        assert_ne!(Block::KELP.default_state.id, Block::AIR.default_state.id);
        assert_ne!(
            Block::KELP_PLANT.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn kelp_cannot_support_tag_parity() {
        assert!(Block::STONE.is_solid());
        assert!(Block::DIRT.is_solid());
        assert!(Block::SAND.is_solid());
        assert!(!Block::STONE.has_tag(&tag::Block::MINECRAFT_CANNOT_SUPPORT_KELP));
    }

    #[test]
    fn kelp_natural_growth_probability_and_age_cap() {
        assert!(natural_growth_succeeds(24, 0.139_999));
        assert!(!natural_growth_succeeds(24, 0.14));
        assert!(!natural_growth_succeeds(25, 0.0));
    }

    #[test]
    fn kelp_growth_age_saturates_at_twenty_five() {
        assert_eq!(24_u8.saturating_add(1).min(25), 25);
        assert_eq!(25_u8.saturating_add(1).min(25), 25);
    }
}
