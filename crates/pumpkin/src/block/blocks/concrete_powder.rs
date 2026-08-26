use crate::{
    block::{
        BlockBehaviour, BlockFuture, BlockMetadata, GetStateForNeighborUpdateArgs,
        OnNeighborUpdateArgs, OnPlaceArgs, OnScheduledTickArgs, PlacedArgs,
        blocks::falling::FallingBlock,
    },
    entity::falling::FallingEntity,
};
use pumpkin_data::{Block, BlockDirection, BlockId, BlockState, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{
    tick::TickPriority,
    world::{BlockAccessor, BlockFlags},
};

pub struct ConcretePowderBlock;

impl ConcretePowderBlock {
    #[must_use]
    pub fn concrete_for_powder(block_id: BlockId) -> Option<&'static Block> {
        match block_id {
            BlockId::WHITE_CONCRETE_POWDER => Some(&Block::WHITE_CONCRETE),
            BlockId::ORANGE_CONCRETE_POWDER => Some(&Block::ORANGE_CONCRETE),
            BlockId::MAGENTA_CONCRETE_POWDER => Some(&Block::MAGENTA_CONCRETE),
            BlockId::LIGHT_BLUE_CONCRETE_POWDER => Some(&Block::LIGHT_BLUE_CONCRETE),
            BlockId::YELLOW_CONCRETE_POWDER => Some(&Block::YELLOW_CONCRETE),
            BlockId::LIME_CONCRETE_POWDER => Some(&Block::LIME_CONCRETE),
            BlockId::PINK_CONCRETE_POWDER => Some(&Block::PINK_CONCRETE),
            BlockId::GRAY_CONCRETE_POWDER => Some(&Block::GRAY_CONCRETE),
            BlockId::LIGHT_GRAY_CONCRETE_POWDER => Some(&Block::LIGHT_GRAY_CONCRETE),
            BlockId::CYAN_CONCRETE_POWDER => Some(&Block::CYAN_CONCRETE),
            BlockId::PURPLE_CONCRETE_POWDER => Some(&Block::PURPLE_CONCRETE),
            BlockId::BLUE_CONCRETE_POWDER => Some(&Block::BLUE_CONCRETE),
            BlockId::BROWN_CONCRETE_POWDER => Some(&Block::BROWN_CONCRETE),
            BlockId::GREEN_CONCRETE_POWDER => Some(&Block::GREEN_CONCRETE),
            BlockId::RED_CONCRETE_POWDER => Some(&Block::RED_CONCRETE),
            BlockId::BLACK_CONCRETE_POWDER => Some(&Block::BLACK_CONCRETE),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_water(state: &BlockState) -> bool {
        let block = Block::from_state_id(state.id);
        block.id == Block::WATER.id || state.is_waterlogged() || block.id == Block::BUBBLE_COLUMN.id
    }

    #[must_use]
    pub fn should_harden(world: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        let current_state = world.get_block_state(pos);
        if Self::is_water(current_state) {
            return true;
        }
        for dir in &[
            BlockDirection::North,
            BlockDirection::South,
            BlockDirection::East,
            BlockDirection::West,
            BlockDirection::Up,
        ] {
            let neighbor_pos = pos.offset(dir.to_offset());
            let neighbor_state = world.get_block_state(&neighbor_pos);
            if Self::is_water(neighbor_state) {
                return true;
            }
        }
        false
    }
}

impl BlockMetadata for ConcretePowderBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::WHITE_CONCRETE_POWDER,
            BlockId::ORANGE_CONCRETE_POWDER,
            BlockId::MAGENTA_CONCRETE_POWDER,
            BlockId::LIGHT_BLUE_CONCRETE_POWDER,
            BlockId::YELLOW_CONCRETE_POWDER,
            BlockId::LIME_CONCRETE_POWDER,
            BlockId::PINK_CONCRETE_POWDER,
            BlockId::GRAY_CONCRETE_POWDER,
            BlockId::LIGHT_GRAY_CONCRETE_POWDER,
            BlockId::CYAN_CONCRETE_POWDER,
            BlockId::PURPLE_CONCRETE_POWDER,
            BlockId::BLUE_CONCRETE_POWDER,
            BlockId::BROWN_CONCRETE_POWDER,
            BlockId::GREEN_CONCRETE_POWDER,
            BlockId::RED_CONCRETE_POWDER,
            BlockId::BLACK_CONCRETE_POWDER,
        ]
        .into()
    }
}

impl BlockBehaviour for ConcretePowderBlock {
    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if Self::should_harden(args.world, args.position) {
                if let Some(concrete) = Self::concrete_for_powder(args.block.id) {
                    return concrete.default_state.id;
                }
            }
            args.block.default_state.id
        })
    }

    fn placed<'a>(&'a self, args: PlacedArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if Self::should_harden(args.world.as_ref(), args.position) {
                if let Some(concrete) = Self::concrete_for_powder(args.block.id) {
                    args.world
                        .set_block_state(
                            args.position,
                            concrete.default_state.id,
                            BlockFlags::NOTIFY_ALL,
                        )
                        .await;
                    return;
                }
            }
            args.world
                .schedule_block_tick(args.block, *args.position, 2, TickPriority::Normal);
        })
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if Self::should_harden(args.world, args.position) {
                if let Some(concrete) = Self::concrete_for_powder(args.block.id) {
                    return concrete.default_state.id;
                }
            }
            args.world
                .schedule_block_tick(args.block, *args.position, 2, TickPriority::Normal);
            args.state_id
        })
    }

    fn on_neighbor_update<'a>(&'a self, args: OnNeighborUpdateArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if Self::should_harden(args.world.as_ref(), args.position) {
                if let Some(concrete) = Self::concrete_for_powder(args.block.id) {
                    args.world
                        .set_block_state(
                            args.position,
                            concrete.default_state.id,
                            BlockFlags::NOTIFY_ALL,
                        )
                        .await;
                    return;
                }
            }
            args.world
                .schedule_block_tick(args.block, *args.position, 2, TickPriority::Normal);
        })
    }

    fn on_scheduled_tick<'a>(&'a self, args: OnScheduledTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if Self::should_harden(args.world.as_ref(), args.position) {
                if let Some(concrete) = Self::concrete_for_powder(args.block.id) {
                    args.world
                        .set_block_state(
                            args.position,
                            concrete.default_state.id,
                            BlockFlags::NOTIFY_ALL,
                        )
                        .await;
                    return;
                }
            }
            let (block, state) = args.world.get_block_and_state(&args.position.down());
            if !FallingBlock::can_fall_through(state, block) || args.position.0.y < args.world.min_y
            {
                return;
            }
            let state = args.world.get_block_state(args.position);
            FallingEntity::replace_spawn(args.world, *args.position, state.id).await;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_registers_every_vanilla_concrete_powder_color() {
        let ids = ConcretePowderBlock::ids();
        assert_eq!(ids.len(), 16);
        for id in ids.iter() {
            assert!(ConcretePowderBlock::concrete_for_powder(*id).is_some());
        }
    }
}
