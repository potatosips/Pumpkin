use pumpkin_data::Block;
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;

use crate::block::{BlockBehaviour, BlockFuture, RandomTickArgs};

#[pumpkin_block("minecraft:ice")]
pub struct IceBlock;

impl BlockBehaviour for IceBlock {
    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let state = args.world.get_block_state(args.position);
            let light = args.world.get_block_light_level(args.position).unwrap_or(0);
            if !should_melt(light, state.opacity) {
                return;
            }
            let replacement = if args.world.dimension.minecraft_name == "minecraft:the_nether" {
                Block::AIR.default_state.id
            } else {
                Block::WATER.default_state.id
            };
            args.world
                .set_block_state(args.position, replacement, BlockFlags::NOTIFY_ALL)
                .await;
        })
    }
}

const fn should_melt(block_light: u8, opacity: u8) -> bool {
    block_light > 11u8.saturating_sub(opacity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ice_melting_uses_vanilla_light_threshold() {
        assert!(Block::ICE.default_state.has_random_ticks());
        let opacity = Block::ICE.default_state.opacity;
        let threshold = 11u8.saturating_sub(opacity);
        assert!(!should_melt(threshold, opacity));
        assert!(should_melt(threshold + 1, opacity));
    }
}
