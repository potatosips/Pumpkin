use crate::block::blocks::falling::FallingBlock;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, BlockFuture, GetStateForNeighborUpdateArgs, NormalUseArgs,
    OnNeighborUpdateArgs, OnScheduledTickArgs, PlacedArgs,
};
use crate::world::World;
use pumpkin_data::BlockStateId;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use rand::{RngExt, rng};
use std::sync::Arc;

#[pumpkin_block("minecraft:dragon_egg")]
pub struct DragonEggBlock;

impl DragonEggBlock {
    pub async fn teleport(&self, world: &Arc<World>, pos: &BlockPos) -> bool {
        let max_y = world.min_y + world.dimension.height as i32;
        for _ in 0..1000 {
            let x = pos.0.x + rng().random_range(-16..=16);
            let y = pos.0.y + rng().random_range(-7..=7);
            let z = pos.0.z + rng().random_range(-16..=16);

            if y < world.min_y || y >= max_y {
                continue;
            }

            let test_pos = BlockPos::new(x, y, z);
            let state = world.get_block_state(&test_pos);
            let below_state = world.get_block_state(&test_pos.down());

            if state.is_air() && !below_state.is_air() {
                let current_state = world.get_block_state(pos);
                world
                    .set_block_state(
                        &test_pos,
                        current_state.id,
                        pumpkin_world::world::BlockFlags::NOTIFY_ALL,
                    )
                    .await;
                world
                    .set_block_state(
                        pos,
                        pumpkin_data::Block::AIR.default_state.id,
                        pumpkin_world::world::BlockFlags::NOTIFY_ALL,
                    )
                    .await;
                return true;
            }
        }
        false
    }
}

impl BlockBehaviour for DragonEggBlock {
    fn placed<'a>(&'a self, args: PlacedArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            args.world
                .schedule_block_tick(args.block, *args.position, 5, TickPriority::Normal);
        })
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            args.world
                .schedule_block_tick(args.block, *args.position, 5, TickPriority::Normal);
            args.state_id
        })
    }

    fn on_neighbor_update<'a>(&'a self, args: OnNeighborUpdateArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            args.world
                .schedule_block_tick(args.block, *args.position, 5, TickPriority::Normal);
        })
    }

    fn normal_use<'a>(&'a self, args: NormalUseArgs<'a>) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async move {
            self.teleport(args.world, args.position).await;
            BlockActionResult::Success
        })
    }

    fn on_scheduled_tick<'a>(&'a self, args: OnScheduledTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            FallingBlock::on_scheduled_tick(&FallingBlock, args).await;
        })
    }
}

#[cfg(test)]
mod tests {
    use pumpkin_data::Block;

    #[test]
    fn dragon_egg_block_id_parity() {
        assert_eq!(Block::DRAGON_EGG.name, "dragon_egg");
    }
}
