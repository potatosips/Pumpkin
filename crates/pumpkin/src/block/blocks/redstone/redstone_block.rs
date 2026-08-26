use pumpkin_macros::pumpkin_block;

use crate::block::{BlockBehaviour, BlockFuture, EmitsRedstonePowerArgs, GetRedstonePowerArgs};

#[pumpkin_block("minecraft:redstone_block")]
pub struct RedstoneBlock;

impl BlockBehaviour for RedstoneBlock {
    fn get_weak_redstone_power<'a>(
        &'a self,
        _args: GetRedstonePowerArgs<'a>,
    ) -> BlockFuture<'a, u8> {
        Box::pin(async move { 15 })
    }

    fn emits_redstone_power<'a>(
        &'a self,
        _args: EmitsRedstonePowerArgs<'a>,
    ) -> BlockFuture<'a, bool> {
        Box::pin(async move { true })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn redstone_block_id_parity() {
        assert_eq!(Block::REDSTONE_BLOCK.name, "redstone_block");
    }

    #[test]
    fn redstone_block_default_state_parity() {
        assert_ne!(
            Block::REDSTONE_BLOCK.default_state.id,
            Block::AIR.default_state.id
        );
    }
}
