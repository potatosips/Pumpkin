use crate::{
    block::{
        BlockBehaviour, BlockFuture, GetStateForNeighborUpdateArgs, OnNeighborUpdateArgs,
        OnPlaceArgs, OnScheduledTickArgs, PlacedArgs,
    },
    entity::falling::FallingEntity,
};
use pumpkin_data::{
    BlockDirection, BlockId, BlockStateId,
    block_properties::{BlockProperties, ScaffoldingLikeProperties},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{
    tick::TickPriority,
    world::{BlockAccessor, BlockFlags},
};

#[pumpkin_block("minecraft:scaffolding")]
pub struct ScaffoldingBlock;

impl ScaffoldingBlock {
    pub const MAX_DISTANCE: u8 = 7;

    #[must_use]
    pub fn get_distance(world: &dyn BlockAccessor, pos: &BlockPos) -> u8 {
        let below_pos = pos.down();
        let (below_block, below_state) = world.get_block_and_state(&below_pos);

        if below_block.id == BlockId::SCAFFOLDING {
            return 0;
        }

        // If block below is solid / full cube or non-replaceable solid block
        if below_state.is_full_cube()
            || (!below_state.is_air() && !below_state.replaceable() && !below_state.is_liquid())
        {
            return 0;
        }

        let mut min_distance = Self::MAX_DISTANCE;
        for dir in &[
            BlockDirection::North,
            BlockDirection::South,
            BlockDirection::East,
            BlockDirection::West,
        ] {
            let neighbor_pos = pos.offset(dir.to_offset());
            let (nb_block, nb_state) = world.get_block_and_state(&neighbor_pos);
            if nb_block.id == BlockId::SCAFFOLDING {
                let props = ScaffoldingLikeProperties::from_state_id(nb_state.id, nb_block);
                min_distance = min_distance.min(props.distance);
            }
        }

        if min_distance < Self::MAX_DISTANCE {
            min_distance + 1
        } else {
            Self::MAX_DISTANCE
        }
    }

    #[must_use]
    pub fn is_bottom(world: &dyn BlockAccessor, pos: &BlockPos, distance: u8) -> bool {
        if distance == 0 {
            return false;
        }
        let below_pos = pos.down();
        let below_block = world.get_block(&below_pos);
        below_block.id != BlockId::SCAFFOLDING
    }
}

impl BlockBehaviour for ScaffoldingBlock {
    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let distance = Self::get_distance(args.world, args.position);
            let bottom = Self::is_bottom(args.world, args.position, distance);
            let mut props =
                ScaffoldingLikeProperties::from_state_id(args.block.default_state.id, args.block);
            props.distance = distance;
            props.bottom = bottom;
            props.to_state_id(args.block)
        })
    }

    fn placed<'a>(&'a self, args: PlacedArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let distance = Self::get_distance(args.world.as_ref(), args.position);
            if distance >= Self::MAX_DISTANCE {
                args.world
                    .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
            }
        })
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let distance = Self::get_distance(args.world, args.position);
            let bottom = Self::is_bottom(args.world, args.position, distance);
            let mut props = ScaffoldingLikeProperties::from_state_id(args.state_id, args.block);
            if props.distance != distance || props.bottom != bottom {
                args.world
                    .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
            }
            props.distance = distance;
            props.bottom = bottom;
            props.to_state_id(args.block)
        })
    }

    fn on_neighbor_update<'a>(&'a self, args: OnNeighborUpdateArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let distance = Self::get_distance(args.world.as_ref(), args.position);
            if distance >= Self::MAX_DISTANCE {
                args.world
                    .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
            }
        })
    }

    fn on_scheduled_tick<'a>(&'a self, args: OnScheduledTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let distance = Self::get_distance(args.world.as_ref(), args.position);
            if distance >= Self::MAX_DISTANCE {
                let current_state = args.world.get_block_state(args.position);
                FallingEntity::replace_spawn(args.world, *args.position, current_state.id).await;
            } else {
                let bottom = Self::is_bottom(args.world.as_ref(), args.position, distance);
                let current_state = args.world.get_block_state(args.position);
                let mut props =
                    ScaffoldingLikeProperties::from_state_id(current_state.id, args.block);
                if props.distance != distance || props.bottom != bottom {
                    props.distance = distance;
                    props.bottom = bottom;
                    let new_state_id = props.to_state_id(args.block);
                    args.world
                        .set_block_state(args.position, new_state_id, BlockFlags::NOTIFY_ALL)
                        .await;
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn scaffolding_block_id_parity() {
        assert_eq!(Block::SCAFFOLDING.name, "scaffolding");
    }

    #[test]
    fn scaffolding_properties_encoding_decoding_parity() {
        assert_eq!(Block::SCAFFOLDING.states.len(), 32);
        for distance in 0..=7 {
            for bottom in [false, true] {
                for waterlogged in [false, true] {
                    let props = ScaffoldingLikeProperties {
                        bottom,
                        distance,
                        waterlogged,
                    };
                    let state_id = props.to_state_id(&Block::SCAFFOLDING);
                    let decoded =
                        ScaffoldingLikeProperties::from_state_id(state_id, &Block::SCAFFOLDING);
                    assert_eq!(decoded.distance, distance);
                    assert_eq!(decoded.bottom, bottom);
                    assert_eq!(decoded.waterlogged, waterlogged);
                }
            }
        }
    }

    #[test]
    fn scaffolding_default_state_parity() {
        let default_props = ScaffoldingLikeProperties::from_state_id(
            Block::SCAFFOLDING.default_state.id,
            &Block::SCAFFOLDING,
        );
        assert_eq!(default_props.distance, 7);
        assert_eq!(default_props.bottom, false);
        assert_eq!(default_props.waterlogged, false);
    }
}
