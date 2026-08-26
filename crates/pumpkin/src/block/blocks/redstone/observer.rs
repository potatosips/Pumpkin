use std::sync::Arc;

use crate::block::{
    BlockFuture, EmitsRedstonePowerArgs, GetRedstonePowerArgs, GetStateForNeighborUpdateArgs,
    OnPlaceArgs, OnScheduledTickArgs, OnStateReplacedArgs,
};
use crate::entity::EntityBase;
use pumpkin_data::{
    Block, BlockStateId, FacingExt,
    block_properties::{BlockProperties, ObserverLikeProperties},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{tick::TickPriority, world::BlockFlags};

use crate::{block::BlockBehaviour, world::World};

#[pumpkin_block("minecraft:observer")]
pub struct ObserverBlock;

impl BlockBehaviour for ObserverBlock {
    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let mut props = ObserverLikeProperties::default(args.block);
            props.facing = args.player.get_entity().get_facing();
            props.to_state_id(args.block)
        })
    }

    fn on_scheduled_tick<'a>(&'a self, args: OnScheduledTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let state = args.world.get_block_state(args.position);
            let mut props = ObserverLikeProperties::from_state_id(state.id, args.block);

            if props.powered {
                props.powered = false;
                args.world
                    .set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_LISTENERS,
                    )
                    .await;
            } else {
                props.powered = true;
                args.world
                    .set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_LISTENERS,
                    )
                    .await;
                args.world
                    .schedule_block_tick(args.block, *args.position, 2, TickPriority::Normal);
            }

            Self::update_neighbors(args.world, args.block, args.position, &props).await;
        })
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let props = ObserverLikeProperties::from_state_id(args.state_id, args.block);

            if props.facing.to_block_direction() == args.direction
                && !props.powered
                && !args
                    .world
                    .is_block_tick_scheduled(args.position, &Block::OBSERVER)
            {
                Self::schedule_tick(args.world, args.position);
            }

            args.state_id
        })
    }

    fn emits_redstone_power<'a>(
        &'a self,
        _args: EmitsRedstonePowerArgs<'a>,
    ) -> BlockFuture<'a, bool> {
        Box::pin(async move { true })
    }

    fn get_weak_redstone_power<'a>(
        &'a self,
        args: GetRedstonePowerArgs<'a>,
    ) -> BlockFuture<'a, u8> {
        Box::pin(async move {
            let props = ObserverLikeProperties::from_state_id(args.state.id, args.block);
            if props.facing.to_block_direction() == args.direction && props.powered {
                15
            } else {
                0
            }
        })
    }

    fn get_strong_redstone_power<'a>(
        &'a self,
        args: GetRedstonePowerArgs<'a>,
    ) -> BlockFuture<'a, u8> {
        Box::pin(async move { self.get_weak_redstone_power(args).await })
    }

    fn on_state_replaced<'a>(&'a self, args: OnStateReplacedArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if !args.moved {
                let props = ObserverLikeProperties::from_state_id(args.old_state_id, args.block);
                if props.powered
                    && args
                        .world
                        .is_block_tick_scheduled(args.position, &Block::OBSERVER)
                {
                    Self::update_neighbors(args.world, args.block, args.position, &props).await;
                }
            }
        })
    }
}

impl ObserverBlock {
    async fn update_neighbors(
        world: &Arc<World>,
        block: &Block,
        block_pos: &BlockPos,
        props: &ObserverLikeProperties,
    ) {
        let facing = props.facing.to_block_direction();
        let opposite_facing_pos = block_pos.offset(facing.opposite().to_offset());

        world.update_neighbor(&opposite_facing_pos, block).await;
        world
            .update_neighbors(&opposite_facing_pos, Some(facing))
            .await;
    }

    fn schedule_tick(world: &World, block_pos: &BlockPos) {
        world.schedule_block_tick(&Block::OBSERVER, *block_pos, 2, TickPriority::Normal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;
    use pumpkin_data::block_properties::{BlockProperties, Facing, ObserverLikeProperties};

    #[test]
    fn observer_block_id_parity() {
        assert_eq!(Block::OBSERVER.name, "observer");
    }

    #[test]
    fn observer_default_state_parity() {
        assert_ne!(
            Block::OBSERVER.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn observer_properties_roundtrip_parity() {
        for facing in [
            Facing::North,
            Facing::South,
            Facing::East,
            Facing::West,
            Facing::Up,
            Facing::Down,
        ] {
            for powered in [true, false] {
                let props = ObserverLikeProperties { facing, powered };
                let state_id = props.to_state_id(&Block::OBSERVER);
                let rt = ObserverLikeProperties::from_state_id(state_id, &Block::OBSERVER);
                assert_eq!(rt.facing, facing);
                assert_eq!(rt.powered, powered);
            }
        }
    }
}
