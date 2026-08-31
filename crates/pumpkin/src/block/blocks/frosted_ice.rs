use pumpkin_data::{Block, BlockDirection, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{tick::TickPriority, world::BlockFlags};
use rand::RngExt;

use crate::block::{
    BlockBehaviour, BlockFuture, GetStateForNeighborUpdateArgs, OnScheduledTickArgs, PlacedArgs,
    RandomTickArgs,
};

#[pumpkin_block("minecraft:frosted_ice")]
pub struct FrostedIceBlock;

impl BlockBehaviour for FrostedIceBlock {
    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let neighbor_is_frosted =
                Block::from_state_id(args.neighbor_state_id) == &Block::FROSTED_ICE;
            if should_break_for_neighbor(
                neighbor_is_frosted,
                frosted_neighbor_count(args.world, *args.position),
            ) {
                Block::AIR.default_state.id
            } else {
                args.state_id
            }
        })
    }

    fn placed<'a>(&'a self, args: PlacedArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move { schedule_next(args.world, *args.position) })
    }

    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move { tick_frosted_ice(args.world, *args.position).await })
    }

    fn on_scheduled_tick<'a>(&'a self, args: OnScheduledTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move { tick_frosted_ice(args.world, *args.position).await })
    }
}

async fn tick_frosted_ice(world: &std::sync::Arc<crate::world::World>, position: BlockPos) {
    let state_id = world.get_block_state_id(&position);
    let Some(age) = frosted_age(state_id) else {
        return;
    };
    let opacity = world.get_block_state(&position).opacity;
    let bright_enough = world.get_max_local_raw_brightness(&position)
        > 11u8.saturating_sub(age).saturating_sub(opacity);
    let sparse = frosted_neighbor_count(world, position) < 4;
    if (rand::rng().random_ratio(1, 3) || sparse) && bright_enough {
        if age < 3 {
            world
                .set_block_state(
                    &position,
                    Block::FROSTED_ICE.states[usize::from(age + 1)].id,
                    BlockFlags::NOTIFY_ALL,
                )
                .await;
            schedule_next(world, position);
            return;
        }
        melt(world, position).await;
        for direction in BlockDirection::all() {
            let neighbor = position.offset(direction.to_offset());
            let Some(neighbor_age) = frosted_age(world.get_block_state_id(&neighbor)) else {
                continue;
            };
            if neighbor_age < 3 {
                world
                    .set_block_state(
                        &neighbor,
                        Block::FROSTED_ICE.states[usize::from(neighbor_age + 1)].id,
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;
                schedule_next(world, neighbor);
            } else {
                melt(world, neighbor).await;
            }
        }
    } else {
        schedule_next(world, position);
    }
}

async fn melt(world: &std::sync::Arc<crate::world::World>, position: BlockPos) {
    let replacement = if world.dimension.minecraft_name == "minecraft:the_nether" {
        Block::AIR.default_state.id
    } else {
        Block::WATER.default_state.id
    };
    world
        .set_block_state(&position, replacement, BlockFlags::NOTIFY_ALL)
        .await;
}

fn schedule_next(world: &crate::world::World, position: BlockPos) {
    world.schedule_block_tick(
        &Block::FROSTED_ICE,
        position,
        rand::rng().random_range(20..=40),
        TickPriority::Normal,
    );
}

fn frosted_neighbor_count(world: &crate::world::World, position: BlockPos) -> usize {
    BlockDirection::all()
        .iter()
        .filter(|direction| {
            world.get_block(&position.offset(direction.to_offset())) == &Block::FROSTED_ICE
        })
        .count()
}

const fn should_break_for_neighbor(neighbor_is_frosted: bool, frosted_neighbors: usize) -> bool {
    neighbor_is_frosted && frosted_neighbors < 2
}

fn frosted_age(state_id: BlockStateId) -> Option<u8> {
    Block::FROSTED_ICE
        .states
        .iter()
        .position(|state| state.id == state_id)
        .and_then(|age| u8::try_from(age).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frosted_ice_age_states_are_exactly_zero_through_three() {
        assert!(!Block::FROSTED_ICE.default_state.has_random_ticks());
        assert_eq!(Block::FROSTED_ICE.states.len(), 4);
        for (age, state) in Block::FROSTED_ICE.states.iter().enumerate() {
            assert_eq!(frosted_age(state.id), Some(age as u8));
        }
        assert_eq!(frosted_age(Block::ICE.default_state.id), None);
        assert!(should_break_for_neighbor(true, 1));
        assert!(!should_break_for_neighbor(true, 2));
        assert!(!should_break_for_neighbor(false, 0));
    }
}
