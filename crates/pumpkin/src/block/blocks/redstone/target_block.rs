use pumpkin_data::{Block, BlockDirection, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{tick::TickPriority, world::BlockFlags};
use std::sync::Arc;

use crate::{
    block::{
        BlockBehaviour, BlockFuture, EmitsRedstonePowerArgs, GetRedstonePowerArgs,
        OnScheduledTickArgs,
    },
    world::World,
};

#[pumpkin_block("minecraft:target")]
pub struct TargetBlock;

impl TargetBlock {
    pub const PROJECTILE_PULSE_TICKS: u8 = 16;
    pub const NORMAL_PULSE_TICKS: u8 = 8;

    #[must_use]
    pub fn get_power_from_state_id(state_id: BlockStateId) -> u8 {
        Block::TARGET
            .states
            .iter()
            .position(|s| s.id == state_id)
            .map_or(0, |pos| pos as u8)
    }

    #[must_use]
    pub fn get_state_id_for_power(power: u8) -> BlockStateId {
        let index = (power.min(15)) as usize;
        Block::TARGET.states[index].id
    }

    /// Calculates vanilla hit power from relative offset on face (0.0 to 1.0)
    #[must_use]
    pub fn calculate_hit_power(
        hit_offset_x: f64,
        hit_offset_y: f64,
        hit_offset_z: f64,
        hit_direction: BlockDirection,
    ) -> u8 {
        let (u, v) = match hit_direction {
            BlockDirection::Down | BlockDirection::Up => {
                ((hit_offset_x - 0.5).abs(), (hit_offset_z - 0.5).abs())
            }
            BlockDirection::North | BlockDirection::South => {
                ((hit_offset_x - 0.5).abs(), (hit_offset_y - 0.5).abs())
            }
            BlockDirection::West | BlockDirection::East => {
                ((hit_offset_y - 0.5).abs(), (hit_offset_z - 0.5).abs())
            }
        };

        let max_offset = u.max(v);
        if max_offset > 0.5 {
            return 1;
        }

        // Vanilla formula: power = clamp(floor((1.0 - max_offset * 2.0) * 15.0) + 1, 1, 15)
        let normalized = (1.0 - max_offset * 2.0).clamp(0.0, 1.0);
        ((normalized * 15.0).floor() as u8 + 1).clamp(1, 15)
    }

    pub async fn trigger(world: &Arc<World>, position: &BlockPos, power: u8, pulse_ticks: u8) {
        let target_state = Self::get_state_id_for_power(power);
        world
            .set_block_state(position, target_state, BlockFlags::NOTIFY_ALL)
            .await;
        world.update_neighbors(position, None).await;
        world.schedule_block_tick(&Block::TARGET, *position, pulse_ticks, TickPriority::Normal);
    }
}

impl BlockBehaviour for TargetBlock {
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
        Box::pin(async move { Self::get_power_from_state_id(args.state.id) })
    }

    fn get_strong_redstone_power<'a>(
        &'a self,
        args: GetRedstonePowerArgs<'a>,
    ) -> BlockFuture<'a, u8> {
        Box::pin(async move { Self::get_power_from_state_id(args.state.id) })
    }

    fn on_scheduled_tick<'a>(&'a self, args: OnScheduledTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let state_id = args.world.get_block_state(args.position).id;
            let current_power = Self::get_power_from_state_id(state_id);
            if current_power > 0 {
                let default_state = Block::TARGET.default_state.id;
                args.world
                    .set_block_state(args.position, default_state, BlockFlags::NOTIFY_ALL)
                    .await;
                args.world.update_neighbors(args.position, None).await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_target_block_power_mapping() {
        assert_eq!(
            TargetBlock::get_power_from_state_id(Block::TARGET.default_state.id),
            0
        );
        for p in 0..=15 {
            let state_id = TargetBlock::get_state_id_for_power(p);
            assert_eq!(TargetBlock::get_power_from_state_id(state_id), p);
        }
    }

    #[test]
    fn vanilla_target_block_hit_calculations() {
        // Exact center hit -> Bullseye (Power 15)
        let center_power = TargetBlock::calculate_hit_power(0.5, 0.5, 0.0, BlockDirection::North);
        assert_eq!(center_power, 15);

        // Near center (offset 0.02) -> Power 15
        let near_center = TargetBlock::calculate_hit_power(0.52, 0.51, 0.0, BlockDirection::North);
        assert_eq!(near_center, 15);

        // Mid-ring hit (offset 0.25) -> Power 8
        let mid_power = TargetBlock::calculate_hit_power(0.75, 0.5, 0.0, BlockDirection::North);
        assert_eq!(mid_power, 8);

        // Edge hit (offset 0.48) -> Power 1
        let edge_power = TargetBlock::calculate_hit_power(0.98, 0.5, 0.0, BlockDirection::North);
        assert_eq!(edge_power, 1);
    }
}
