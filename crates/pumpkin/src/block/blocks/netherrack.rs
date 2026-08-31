use pumpkin_data::Block;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use rand::RngExt;

use crate::block::{BlockBehaviour, BlockFuture, BonemealArgs};

#[pumpkin_block("minecraft:netherrack")]
pub struct NetherrackBlock;

impl BlockBehaviour for NetherrackBlock {
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        let (crimson, warped) = nearby_nylium(args.world, args.position);
        crimson || warped
    }

    fn perform_bonemeal<'a>(&'a self, args: BonemealArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let (crimson, warped) = nearby_nylium(args.world, args.position);
            let Some(block) = conversion(crimson, warped, rand::rng().random::<bool>()) else {
                return;
            };
            args.world
                .set_block_state(
                    args.position,
                    block.default_state.id,
                    BlockFlags::NOTIFY_ALL,
                )
                .await;
        })
    }
}

fn nearby_nylium(world: &crate::world::World, position: &BlockPos) -> (bool, bool) {
    let mut crimson = false;
    let mut warped = false;
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                let block = world.get_block(&position.offset((x, y, z).into()));
                crimson |= block == &Block::CRIMSON_NYLIUM;
                warped |= block == &Block::WARPED_NYLIUM;
                if crimson && warped {
                    return (true, true);
                }
            }
        }
    }
    (crimson, warped)
}

fn conversion(crimson: bool, warped: bool, choose_warped: bool) -> Option<&'static Block> {
    match (crimson, warped, choose_warped) {
        (false, false, _) => None,
        (true, false, _) => Some(&Block::CRIMSON_NYLIUM),
        (false, true, _) => Some(&Block::WARPED_NYLIUM),
        (true, true, true) => Some(&Block::WARPED_NYLIUM),
        (true, true, false) => Some(&Block::CRIMSON_NYLIUM),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn netherrack_nylium_conversion_selection() {
        assert!(conversion(false, false, false).is_none());
        assert_eq!(conversion(true, false, true), Some(&Block::CRIMSON_NYLIUM));
        assert_eq!(conversion(false, true, false), Some(&Block::WARPED_NYLIUM));
        assert_eq!(conversion(true, true, false), Some(&Block::CRIMSON_NYLIUM));
        assert_eq!(conversion(true, true, true), Some(&Block::WARPED_NYLIUM));
    }
}
