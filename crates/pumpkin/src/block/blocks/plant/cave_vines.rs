use pumpkin_data::{
    Block, BlockDirection, BlockId, BlockStateId,
    block_properties::{BlockProperties, CaveVinesLikeProperties, CaveVinesPlantLikeProperties},
    item::Item,
    item_stack::ItemStack,
    sound::{Sound, SoundCategory},
};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use rand::RngExt;

use crate::block::{
    BlockBehaviour, BlockFuture, BlockMetadata, BonemealArgs, BrokenArgs, CanPlaceAtArgs,
    GetStateForNeighborUpdateArgs, NormalUseArgs, PlacedArgs, RandomTickArgs,
    registry::BlockActionResult,
};

pub struct CaveVinesBlock;

impl BlockMetadata for CaveVinesBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::CAVE_VINES, BlockId::CAVE_VINES_PLANT].into()
    }
}

impl BlockBehaviour for CaveVinesBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        has_support(args.block_accessor, args.position)
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if args.direction == BlockDirection::Up && !has_support(args.world, args.position) {
                BlockStateId::AIR
            } else {
                args.state_id
            }
        })
    }

    fn placed<'a>(&'a self, args: PlacedArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let above = args.position.up();
            if args.world.get_block(&above) == &Block::CAVE_VINES {
                let head = CaveVinesLikeProperties::from_state_id(
                    args.world.get_block_state_id(&above),
                    &Block::CAVE_VINES,
                );
                args.world
                    .set_block_state(
                        &above,
                        CaveVinesPlantLikeProperties {
                            berries: head.berries,
                        }
                        .to_state_id(&Block::CAVE_VINES_PLANT),
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;
            }
        })
    }

    fn broken<'a>(&'a self, args: BrokenArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let above = args.position.up();
            if args.world.get_block(&above) == &Block::CAVE_VINES_PLANT {
                let body = CaveVinesPlantLikeProperties::from_state_id(
                    args.world.get_block_state_id(&above),
                    &Block::CAVE_VINES_PLANT,
                );
                let mut head = CaveVinesLikeProperties::default(&Block::CAVE_VINES);
                head.berries = body.berries;
                args.world
                    .set_block_state(
                        &above,
                        head.to_state_id(&Block::CAVE_VINES),
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;
            }
        })
    }

    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if args.block != &Block::CAVE_VINES {
                return;
            }
            let props = CaveVinesLikeProperties::from_state_id(
                args.world.get_block_state_id(args.position),
                args.block,
            );
            if natural_growth_succeeds(props.age, rand::rng().random::<f64>()) {
                grow_head(args.world, args.position, props).await;
            }
        })
    }

    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        !has_berries(args.block, args.world.get_block_state_id(args.position))
    }

    fn perform_bonemeal<'a>(&'a self, args: BonemealArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move { set_berries(args.world, args.position, args.block, true).await })
    }

    fn normal_use<'a>(&'a self, args: NormalUseArgs<'a>) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async move {
            let state_id = args.world.get_block_state_id(args.position);
            if !has_berries(args.block, state_id) {
                return BlockActionResult::Pass;
            }
            args.world
                .drop_stack(args.position, ItemStack::new(1, &Item::GLOW_BERRIES))
                .await;
            args.world.play_sound_fine(
                Sound::BlockCaveVinesPickBerries,
                SoundCategory::Blocks,
                &args.position.to_f64(),
                1.0,
                rand::random_range(0.8..1.2),
            );
            set_berries(args.world, args.position, args.block, false).await;
            BlockActionResult::SuccessServer
        })
    }
}

fn has_support(world: &dyn BlockAccessor, position: &BlockPos) -> bool {
    let above = position.up();
    let block = world.get_block(&above);
    block == &Block::CAVE_VINES
        || block == &Block::CAVE_VINES_PLANT
        || world
            .get_block_state(&above)
            .is_side_solid(BlockDirection::Down)
}

fn natural_growth_succeeds(age: u8, roll: f64) -> bool {
    age < 25 && roll < 0.1
}

fn has_berries(block: &Block, state_id: BlockStateId) -> bool {
    if block == &Block::CAVE_VINES {
        CaveVinesLikeProperties::from_state_id(state_id, block).berries
    } else if block == &Block::CAVE_VINES_PLANT {
        CaveVinesPlantLikeProperties::from_state_id(state_id, block).berries
    } else {
        false
    }
}

async fn set_berries(
    world: &std::sync::Arc<crate::world::World>,
    position: &BlockPos,
    block: &Block,
    berries: bool,
) {
    let state_id = world.get_block_state_id(position);
    let new_state = if block == &Block::CAVE_VINES {
        let mut props = CaveVinesLikeProperties::from_state_id(state_id, block);
        props.berries = berries;
        props.to_state_id(block)
    } else {
        let mut props = CaveVinesPlantLikeProperties::from_state_id(state_id, block);
        props.berries = berries;
        props.to_state_id(block)
    };
    world
        .set_block_state(position, new_state, BlockFlags::NOTIFY_LISTENERS)
        .await;
}

async fn grow_head(
    world: &std::sync::Arc<crate::world::World>,
    position: &BlockPos,
    old: CaveVinesLikeProperties,
) {
    let destination = position.down();
    if !world.get_block_state(&destination).is_air() {
        return;
    }
    world
        .set_block_state(
            position,
            CaveVinesPlantLikeProperties {
                berries: old.berries,
            }
            .to_state_id(&Block::CAVE_VINES_PLANT),
            BlockFlags::NOTIFY_ALL,
        )
        .await;
    let mut new_head = old;
    new_head.age = new_head.age.saturating_add(1).min(25);
    new_head.berries = rand::rng().random::<f64>() < 0.11;
    world
        .set_block_state(
            &destination,
            new_head.to_state_id(&Block::CAVE_VINES),
            BlockFlags::NOTIFY_ALL,
        )
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cave_vines_head_properties_roundtrip() {
        for age in 0..=25 {
            for berries in [false, true] {
                let props = CaveVinesLikeProperties { age, berries };
                assert!(
                    CaveVinesLikeProperties::from_state_id(
                        props.to_state_id(&Block::CAVE_VINES),
                        &Block::CAVE_VINES,
                    ) == props
                );
            }
        }
    }

    #[test]
    fn cave_vines_body_preserves_berries() {
        for berries in [false, true] {
            let props = CaveVinesPlantLikeProperties { berries };
            assert!(
                CaveVinesPlantLikeProperties::from_state_id(
                    props.to_state_id(&Block::CAVE_VINES_PLANT),
                    &Block::CAVE_VINES_PLANT,
                ) == props
            );
        }
    }

    #[test]
    fn cave_vines_growth_probability_and_age_cap() {
        assert!(natural_growth_succeeds(24, 0.099_999));
        assert!(!natural_growth_succeeds(24, 0.1));
        assert!(!natural_growth_succeeds(25, 0.0));
    }
}
