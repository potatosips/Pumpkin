use pumpkin_data::{
    Block, BlockStateId,
    block_properties::{BlockProperties, GrassBlockLikeProperties},
    tag::{self, Taggable},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;
use rand::RngExt;

use crate::block::{
    BlockBehaviour, BlockFuture, GetStateForNeighborUpdateArgs, RandomTickArgs,
    blocks::grass_block::{can_be_grass, can_propagate},
};

#[pumpkin_block("minecraft:mycelium")]
pub struct MyceliumBlock;

impl BlockBehaviour for MyceliumBlock {
    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if !can_be_grass(args.world, *args.position) {
                args.world
                    .set_block_state(
                        args.position,
                        Block::DIRT.default_state.id,
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;
                return;
            }
            if args.world.get_max_local_raw_brightness(&args.position.up()) < 9 {
                return;
            }
            for _ in 0..4 {
                let target = args.position.add(
                    rand::rng().random_range(-1..=1),
                    rand::rng().random_range(-3..=1),
                    rand::rng().random_range(-1..=1),
                );
                if !args.world.is_loaded(&target)
                    || args.world.get_block(&target) != &Block::DIRT
                    || !can_propagate(args.world, target)
                {
                    continue;
                }
                let mut properties = GrassBlockLikeProperties::from_state_id(
                    Block::MYCELIUM.default_state.id,
                    &Block::MYCELIUM,
                );
                properties.snowy = args
                    .world
                    .get_block(&target.up())
                    .has_tag(&tag::Block::MINECRAFT_SNOW);
                args.world
                    .set_block_state(
                        &target,
                        properties.to_state_id(&Block::MYCELIUM),
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;
            }
        })
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let snowy = args
                .world
                .get_block(&args.position.up())
                .has_tag(&tag::Block::MINECRAFT_SNOW);
            let mut properties =
                GrassBlockLikeProperties::from_state_id(args.state_id, &Block::MYCELIUM);
            if properties.snowy != snowy {
                properties.snowy = snowy;
                return properties.to_state_id(&Block::MYCELIUM);
            }
            args.state_id
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mycelium_has_random_ticks_and_snowy_states() {
        assert!(Block::MYCELIUM.default_state.has_random_ticks());
        let mut properties = GrassBlockLikeProperties::from_state_id(
            Block::MYCELIUM.default_state.id,
            &Block::MYCELIUM,
        );
        properties.snowy = false;
        let clear = properties.to_state_id(&Block::MYCELIUM);
        properties.snowy = true;
        let snowy = properties.to_state_id(&Block::MYCELIUM);
        assert_ne!(clear, snowy);
    }
}
