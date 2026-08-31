use crate::block::blocks::falling::FallingBlock;
use crate::block::registry::BlockActionResult;
use crate::block::{
    AttackArgs, BlockBehaviour, BlockFuture, GetStateForNeighborUpdateArgs, NormalUseArgs,
    OnNeighborUpdateArgs, OnScheduledTickArgs, PlacedArgs,
};
use crate::world::World;
use pumpkin_data::{BlockStateId, particle::Particle};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::tick::TickPriority;
use rand::{RngExt, rng};
use std::sync::Arc;

#[pumpkin_block("minecraft:dragon_egg")]
pub struct DragonEggBlock;

impl DragonEggBlock {
    fn is_valid_destination(state: &pumpkin_data::BlockState) -> bool {
        state.is_air()
    }

    pub async fn teleport(&self, world: &Arc<World>, pos: &BlockPos) -> bool {
        let max_y = world.min_y + world.dimension.height as i32;
        for _ in 0..1000 {
            // Vanilla uses differences of two random values, giving triangular offsets.
            let x = pos.0.x + rng().random_range(0..16) - rng().random_range(0..16);
            let y = pos.0.y + rng().random_range(0..8) - rng().random_range(0..8);
            let z = pos.0.z + rng().random_range(0..16) - rng().random_range(0..16);

            if y < world.min_y || y >= max_y {
                continue;
            }

            let test_pos = BlockPos::new(x, y, z);
            if !world.worldborder.lock().await.contains_block(x, z) {
                continue;
            }

            // Unsupported air is valid; the falling-block tick handles it afterward.
            if Self::is_valid_destination(world.get_block_state(&test_pos)) {
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

                // Vanilla emits 128 individually positioned portal particles along
                // the line between the old and new positions, each with its own velocity.
                for _ in 0..128 {
                    let interpolation = rand::random::<f64>();
                    let particle_position = Vector3::new(
                        f64::from(x)
                            .mul_add(1.0 - interpolation, f64::from(pos.0.x) * interpolation)
                            + rand::random::<f64>(),
                        f64::from(y)
                            .mul_add(1.0 - interpolation, f64::from(pos.0.y) * interpolation)
                            + rand::random::<f64>()
                            - 0.5,
                        f64::from(z)
                            .mul_add(1.0 - interpolation, f64::from(pos.0.z) * interpolation)
                            + rand::random::<f64>(),
                    );
                    let velocity = Vector3::new(
                        (rand::random::<f32>() - 0.5) * 0.2,
                        (rand::random::<f32>() - 0.5) * 0.2,
                        (rand::random::<f32>() - 0.5) * 0.2,
                    );
                    // A zero packet count makes the offsets an exact velocity vector.
                    world.spawn_particle(particle_position, velocity, 1.0, 0, Particle::Portal);
                }
                return true;
            }
        }
        false
    }
}

impl BlockBehaviour for DragonEggBlock {
    fn on_attack<'a>(&'a self, args: AttackArgs<'a>) -> BlockFuture<'a, bool> {
        Box::pin(async move { self.teleport(args.world, args.position).await })
    }

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
    use super::DragonEggBlock;
    use pumpkin_data::Block;

    #[test]
    fn dragon_egg_block_id_parity() {
        assert_eq!(Block::DRAGON_EGG.name, "dragon_egg");
    }

    #[test]
    fn destination_requires_only_air_not_support_below() {
        assert!(DragonEggBlock::is_valid_destination(
            Block::AIR.default_state
        ));
        assert!(!DragonEggBlock::is_valid_destination(
            Block::STONE.default_state
        ));
    }
}
