use pumpkin_data::{
    Block, BlockStateId,
    entity::EntityType,
    sound::{Sound, SoundCategory},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::{tick::TickPriority, world::BlockFlags};
use rand::RngExt;
use uuid::Uuid;

use crate::{
    block::{
        BlockBehaviour, BlockFuture, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs,
        OnScheduledTickArgs,
    },
    entity::r#type::from_type,
};

const HATCH_POLL_TICKS: u8 = u8::MAX;
const HATCH_CHANCE_PER_POLL: u32 = 31;

#[pumpkin_block("minecraft:frogspawn")]
pub struct FrogspawnBlock;

impl BlockBehaviour for FrogspawnBlock {
    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            schedule_hatch(args.world, *args.position);
            args.block.default_state.id
        })
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        args.block_accessor.get_block(&args.position.down()) == &Block::WATER
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if args.world.get_block(&args.position.down()) != &Block::WATER {
                Block::AIR.default_state.id
            } else {
                args.state_id
            }
        })
    }

    fn on_scheduled_tick<'a>(&'a self, args: OnScheduledTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if args.world.get_block(args.position) != &Block::FROGSPAWN
                || args.world.get_block(&args.position.down()) != &Block::WATER
            {
                return;
            }
            if !rand::rng().random_ratio(1, HATCH_CHANCE_PER_POLL) {
                schedule_hatch(args.world, *args.position);
                return;
            }
            args.world
                .set_block_state(
                    args.position,
                    Block::AIR.default_state.id,
                    BlockFlags::NOTIFY_ALL,
                )
                .await;
            args.world.play_sound(
                Sound::BlockFrogspawnHatch,
                SoundCategory::Blocks,
                &args.position.to_f64(),
            );
            let count = rand::rng().random_range(2..=5);
            for _ in 0..count {
                let tadpole = from_type(
                    &EntityType::TADPOLE,
                    Vector3::new(
                        f64::from(args.position.0.x) + rand::random_range(0.2..0.8),
                        f64::from(args.position.0.y) - 0.5,
                        f64::from(args.position.0.z) + rand::random_range(0.2..0.8),
                    ),
                    args.world,
                    Uuid::new_v4(),
                );
                args.world.spawn_entity(tadpole).await;
            }
        })
    }
}

pub fn schedule_hatch(
    world: &crate::world::World,
    position: pumpkin_util::math::position::BlockPos,
) {
    world.schedule_block_tick(
        &Block::FROGSPAWN,
        position,
        HATCH_POLL_TICKS,
        TickPriority::Normal,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hatch_poll_interval_fits_scheduler() {
        assert_eq!(HATCH_POLL_TICKS, 255);
        assert_eq!(HATCH_CHANCE_PER_POLL, 31);
    }
}
