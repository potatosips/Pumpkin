use pumpkin_data::{
    Block, BlockId, BlockState,
    block_properties::{BlockProperties, KelpLikeProperties},
};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use rand::RngExt;

use crate::block::{BlockBehaviour, BlockFuture, BlockMetadata, BonemealArgs, RandomTickArgs};

pub struct NyliumBlock;

impl BlockMetadata for NyliumBlock {
    fn ids() -> Box<[BlockId]> {
        [Block::CRIMSON_NYLIUM.id, Block::WARPED_NYLIUM.id].into()
    }
}

impl BlockBehaviour for NyliumBlock {
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        let above = args.position.up();
        args.world.is_in_height_limit(above.0.y)
            && args.world.is_loaded(&above)
            && args.world.get_block_state(&above).is_air()
    }

    fn perform_bonemeal<'a>(&'a self, args: BonemealArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let origin = args.position.up();
            if args.block == &Block::CRIMSON_NYLIUM {
                spread_vegetation(args.world, origin, VegetationKind::Crimson).await;
            } else if args.block == &Block::WARPED_NYLIUM {
                spread_vegetation(args.world, origin, VegetationKind::Warped).await;
                spread_vegetation(args.world, origin, VegetationKind::NetherSprouts).await;
                if rand::rng().random_ratio(1, 8) {
                    spread_twisting_vines(args.world, origin).await;
                }
            }
        })
    }

    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if !can_survive_under(args.world.get_block_state(&args.position.up())) {
                args.world
                    .set_block_state(
                        args.position,
                        Block::NETHERRACK.default_state.id,
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;
            }
        })
    }
}

async fn spread_twisting_vines(world: &std::sync::Arc<crate::world::World>, origin: BlockPos) {
    if !valid_twisting_vines_origin(world, origin) {
        return;
    }
    for _ in 0..9 {
        let target = origin.add(
            rand::rng().random_range(0..3) - rand::rng().random_range(0..3),
            0,
            rand::rng().random_range(0..3) - rand::rng().random_range(0..3),
        );
        if !valid_twisting_vines_origin(world, target) {
            continue;
        }
        let height = twisting_column_height(
            rand::rng().random_range(1..=2),
            rand::rng().random_ratio(1, 6),
            rand::rng().random_ratio(1, 10),
        );
        for step in 0..height {
            let position = target.add(0, step, 0);
            if !world.is_loaded(&position) || !world.get_block_state(&position).is_air() {
                break;
            }
            let above = position.up();
            let is_head = step == height - 1
                || !world.is_loaded(&above)
                || !world.get_block_state(&above).is_air();
            let state = if is_head {
                let mut properties = KelpLikeProperties::from_state_id(
                    Block::TWISTING_VINES.default_state.id,
                    &Block::TWISTING_VINES,
                );
                properties.age = rand::rng().random_range(17..=25);
                properties.to_state_id(&Block::TWISTING_VINES)
            } else {
                Block::TWISTING_VINES_PLANT.default_state.id
            };
            world
                .set_block_state(&position, state, BlockFlags::NOTIFY_ALL)
                .await;
            if is_head {
                break;
            }
        }
    }
}

fn valid_twisting_vines_origin(world: &crate::world::World, position: BlockPos) -> bool {
    world.is_in_height_limit(position.0.y)
        && world.is_loaded(&position)
        && world.get_block_state(&position).is_air()
        && supports_twisting_vines(world.get_block(&position.down()))
}

fn supports_twisting_vines(block: &Block) -> bool {
    block.id == Block::WARPED_NYLIUM.id
        || block.id == Block::WARPED_WART_BLOCK.id
        || block.id == Block::TWISTING_VINES.id
        || block.id == Block::TWISTING_VINES_PLANT.id
}

const fn twisting_column_height(base: i32, double: bool, force_one: bool) -> i32 {
    if force_one {
        1
    } else if double {
        base * 2
    } else {
        base
    }
}

#[derive(Clone, Copy)]
enum VegetationKind {
    Crimson,
    Warped,
    NetherSprouts,
}

async fn spread_vegetation(
    world: &std::sync::Arc<crate::world::World>,
    origin: BlockPos,
    kind: VegetationKind,
) {
    // The three bonemeal configured features all use spread_width=3 and
    // spread_height=1, producing nine horizontal placement attempts.
    for _ in 0..9 {
        let target = origin.add(
            rand::rng().random_range(0..3) - rand::rng().random_range(0..3),
            0,
            rand::rng().random_range(0..3) - rand::rng().random_range(0..3),
        );
        if !world.is_loaded(&target) || !world.get_block_state(&target).is_air() {
            continue;
        }
        let roll = match kind {
            VegetationKind::Crimson => rand::rng().random_range(0..99),
            VegetationKind::Warped => rand::rng().random_range(0..100),
            VegetationKind::NetherSprouts => 0,
        };
        let block = vegetation_for_roll(kind, roll);
        if !world.block_registry.can_place_at(
            None,
            Some(world.as_ref()),
            world.as_ref(),
            None,
            block,
            block.default_state,
            &target,
            None,
            None,
        ) {
            continue;
        }
        world
            .set_block_state(&target, block.default_state.id, BlockFlags::NOTIFY_ALL)
            .await;
    }
}

const fn vegetation_for_roll(kind: VegetationKind, roll: u8) -> &'static Block {
    match kind {
        VegetationKind::Crimson if roll < 87 => &Block::CRIMSON_ROOTS,
        VegetationKind::Crimson if roll < 98 => &Block::CRIMSON_FUNGUS,
        VegetationKind::Crimson => &Block::WARPED_FUNGUS,
        VegetationKind::Warped if roll < 85 => &Block::WARPED_ROOTS,
        VegetationKind::Warped if roll < 86 => &Block::CRIMSON_ROOTS,
        VegetationKind::Warped if roll < 99 => &Block::WARPED_FUNGUS,
        VegetationKind::Warped => &Block::CRIMSON_FUNGUS,
        VegetationKind::NetherSprouts => &Block::NETHER_SPROUTS,
    }
}

// Vanilla's nylium decay uses the same light-blocking test as grass/mycelium.
// A completely opaque block above prevents survival; transparent and partial
// blocks do not.
const fn can_survive_under(above: &BlockState) -> bool {
    above.opacity < 15
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nylium_only_decays_below_fully_opaque_blocks() {
        assert!(!can_survive_under(Block::STONE.default_state));
        assert!(can_survive_under(Block::GLASS.default_state));
        assert!(can_survive_under(Block::AIR.default_state));
        assert!(Block::CRIMSON_NYLIUM.default_state.has_random_ticks());
        assert!(Block::WARPED_NYLIUM.default_state.has_random_ticks());
    }

    #[test]
    fn nylium_bonemeal_vegetation_weights_match_configured_features() {
        assert_eq!(
            vegetation_for_roll(VegetationKind::Crimson, 86),
            &Block::CRIMSON_ROOTS
        );
        assert_eq!(
            vegetation_for_roll(VegetationKind::Crimson, 87),
            &Block::CRIMSON_FUNGUS
        );
        assert_eq!(
            vegetation_for_roll(VegetationKind::Crimson, 98),
            &Block::WARPED_FUNGUS
        );
        assert_eq!(
            vegetation_for_roll(VegetationKind::Warped, 84),
            &Block::WARPED_ROOTS
        );
        assert_eq!(
            vegetation_for_roll(VegetationKind::Warped, 85),
            &Block::CRIMSON_ROOTS
        );
        assert_eq!(
            vegetation_for_roll(VegetationKind::Warped, 86),
            &Block::WARPED_FUNGUS
        );
        assert_eq!(
            vegetation_for_roll(VegetationKind::Warped, 99),
            &Block::CRIMSON_FUNGUS
        );
        assert_eq!(
            vegetation_for_roll(VegetationKind::NetherSprouts, 99),
            &Block::NETHER_SPROUTS
        );
    }

    #[test]
    fn warped_nylium_twisting_vines_feature_parameters() {
        assert!(supports_twisting_vines(&Block::WARPED_NYLIUM));
        assert!(supports_twisting_vines(&Block::WARPED_WART_BLOCK));
        assert!(supports_twisting_vines(&Block::TWISTING_VINES));
        assert!(supports_twisting_vines(&Block::TWISTING_VINES_PLANT));
        assert!(!supports_twisting_vines(&Block::CRIMSON_NYLIUM));
        assert_eq!(twisting_column_height(2, false, false), 2);
        assert_eq!(twisting_column_height(2, true, false), 4);
        assert_eq!(twisting_column_height(2, true, true), 1);
    }
}
