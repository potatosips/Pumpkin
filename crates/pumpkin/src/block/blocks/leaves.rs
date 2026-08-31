use pumpkin_data::{
    Block, BlockDirection, BlockStateId,
    block_properties::{BlockProperties, OakLeavesLikeProperties},
    fluid::Fluid,
    tag,
    tag::Taggable,
};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{
    tick::TickPriority,
    world::{BlockAccessor, BlockFlags},
};

use crate::block::{
    BlockBehaviour, BlockFuture, GetStateForNeighborUpdateArgs, OnPlaceArgs, OnScheduledTickArgs,
    RandomTickArgs,
};

pub const DECAY_DISTANCE: u8 = 7;

#[pumpkin_block_from_tag("minecraft:leaves")]
pub struct LeavesBlock;

#[must_use]
pub fn get_distance_at(block: &Block, state_id: BlockStateId) -> u8 {
    if block.has_tag(&tag::Block::MINECRAFT_PREVENTS_NEARBY_LEAF_DECAY) {
        0
    } else if block.has_tag(&tag::Block::MINECRAFT_LEAVES) {
        OakLeavesLikeProperties::from_state_id(state_id, block).distance
    } else {
        DECAY_DISTANCE
    }
}

#[must_use]
pub fn update_distance(
    world: &dyn BlockAccessor,
    pos: &BlockPos,
    mut props: OakLeavesLikeProperties,
) -> OakLeavesLikeProperties {
    let mut min_distance = DECAY_DISTANCE;
    for direction in BlockDirection::all() {
        let neighbor_pos = pos.offset(direction.to_offset());
        let (neighbor_block, neighbor_state) = world.get_block_and_state(&neighbor_pos);
        min_distance = min_distance.min(
            get_distance_at(neighbor_block, neighbor_state.id)
                .saturating_add(1)
                .min(DECAY_DISTANCE),
        );
        if min_distance == 1 {
            break;
        }
    }
    props.distance = min_distance;
    props
}

fn placement_state(
    block: &Block,
    world: &dyn BlockAccessor,
    pos: &BlockPos,
    waterlogged: bool,
) -> BlockStateId {
    let mut props = OakLeavesLikeProperties::default(block);
    props.persistent = true;
    props.waterlogged = waterlogged;
    update_distance(world, pos, props).to_state_id(block)
}

impl BlockBehaviour for LeavesBlock {
    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            placement_state(
                args.block,
                args.world,
                args.position,
                args.replacing.water_source(),
            )
        })
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let current_props = OakLeavesLikeProperties::from_state_id(args.state_id, args.block);
            if current_props.waterlogged {
                args.world.schedule_fluid_tick(
                    &Fluid::WATER,
                    *args.position,
                    Fluid::WATER.flow_speed as u8,
                    TickPriority::Normal,
                );
            }

            let neighbor_block = args.world.get_block(args.neighbor_position);
            let neighbor_distance =
                get_distance_at(neighbor_block, args.neighbor_state_id).saturating_add(1);
            if neighbor_distance != 1 || current_props.distance != neighbor_distance {
                args.world
                    .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
            }
            args.state_id
        })
    }

    fn on_scheduled_tick<'a>(&'a self, args: OnScheduledTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let state_id = args.world.get_block_state_id(args.position);
            let props = OakLeavesLikeProperties::from_state_id(state_id, args.block);
            let new_state_id =
                update_distance(&**args.world, args.position, props).to_state_id(args.block);
            if new_state_id != state_id {
                args.world
                    .set_block_state(args.position, new_state_id, BlockFlags::NOTIFY_ALL)
                    .await;
            }
        })
    }

    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let state_id = args.world.get_block_state_id(args.position);
            let props = OakLeavesLikeProperties::from_state_id(state_id, args.block);
            if !props.persistent && props.distance == DECAY_DISTANCE {
                args.world
                    .break_block(args.position, None, BlockFlags::empty())
                    .await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_are_distance_zero_and_non_support_is_seven() {
        assert_eq!(
            get_distance_at(&Block::OAK_LOG, Block::OAK_LOG.default_state.id),
            0
        );
        assert_eq!(
            get_distance_at(&Block::STONE, Block::STONE.default_state.id),
            7
        );
    }

    #[test]
    fn all_vanilla_leaves_use_distance_properties() {
        for block in [
            &Block::OAK_LEAVES,
            &Block::SPRUCE_LEAVES,
            &Block::BIRCH_LEAVES,
            &Block::JUNGLE_LEAVES,
            &Block::ACACIA_LEAVES,
            &Block::DARK_OAK_LEAVES,
            &Block::MANGROVE_LEAVES,
            &Block::CHERRY_LEAVES,
            &Block::AZALEA_LEAVES,
            &Block::FLOWERING_AZALEA_LEAVES,
        ] {
            assert!(block.has_tag(&tag::Block::MINECRAFT_LEAVES));
            assert!(OakLeavesLikeProperties::handles_block_id(block.id));
        }
    }
}
