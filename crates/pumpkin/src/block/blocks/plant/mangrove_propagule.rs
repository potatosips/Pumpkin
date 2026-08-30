use pumpkin_data::{
    Block, BlockStateId,
    block_properties::{BlockProperties, MangrovePropaguleLikeProperties},
    tag,
    tag::Taggable,
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use rand::RngExt;

use crate::block::{
    BlockBehaviour, BlockFuture, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    OnPlaceArgs, RandomTickArgs,
};

use super::sapling::SaplingBlock;

pub const MAX_AGE: u8 = 4;

#[pumpkin_block("minecraft:mangrove_propagule")]
pub struct MangrovePropaguleBlock;

impl MangrovePropaguleBlock {
    #[must_use]
    pub const fn is_hanging(props: &MangrovePropaguleLikeProperties) -> bool {
        props.hanging
    }

    #[must_use]
    pub const fn is_fully_grown(props: &MangrovePropaguleLikeProperties) -> bool {
        props.age == MAX_AGE
    }

    #[must_use]
    pub fn can_survive(
        world: &dyn BlockAccessor,
        pos: &BlockPos,
        props: &MangrovePropaguleLikeProperties,
    ) -> bool {
        let support = if props.hanging { pos.up() } else { pos.down() };
        let tag = if props.hanging {
            &tag::Block::MINECRAFT_SUPPORTS_HANGING_MANGROVE_PROPAGULE
        } else {
            &tag::Block::MINECRAFT_SUPPORTS_MANGROVE_PROPAGULE
        };
        world.get_block(&support).has_tag(tag)
    }

    #[must_use]
    pub fn create_new_hanging_propagule(age: u8) -> BlockStateId {
        let mut props = MangrovePropaguleLikeProperties::default(&Block::MANGROVE_PROPAGULE);
        props.hanging = true;
        props.age = age.min(MAX_AGE);
        props.to_state_id(&Block::MANGROVE_PROPAGULE)
    }

    fn placed_state(block: &Block, waterlogged: bool) -> BlockStateId {
        let mut props = MangrovePropaguleLikeProperties::default(block);
        props.hanging = false;
        props.age = MAX_AGE;
        props.stage = 0;
        props.waterlogged = waterlogged;
        props.to_state_id(block)
    }

    async fn advance_tree(
        world: &std::sync::Arc<crate::world::World>,
        pos: &BlockPos,
        mut props: MangrovePropaguleLikeProperties,
        bone_meal: bool,
    ) {
        let block = &Block::MANGROVE_PROPAGULE;
        if props.stage == 0 {
            props.stage = 1;
            world
                .set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_ALL)
                .await;
        } else {
            SaplingBlock.grow_tree(world, *pos, block, bone_meal).await;
        }
    }
}

impl BlockBehaviour for MangrovePropaguleBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let props = MangrovePropaguleLikeProperties::from_state_id(args.state.id, args.block);
        Self::can_survive(args.block_accessor, args.position, &props)
    }

    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move { Self::placed_state(args.block, args.replacing.water_source()) })
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let props = MangrovePropaguleLikeProperties::from_state_id(args.state_id, args.block);
            if Self::can_survive(args.world, args.position, &props) {
                args.state_id
            } else {
                Block::AIR.default_state.id
            }
        })
    }

    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let state_id = args.world.get_block_state_id(args.position);
            let mut props = MangrovePropaguleLikeProperties::from_state_id(state_id, args.block);
            if props.hanging {
                if props.age < MAX_AGE {
                    props.age += 1;
                    args.world
                        .set_block_state(
                            args.position,
                            props.to_state_id(args.block),
                            BlockFlags::NOTIFY_ALL,
                        )
                        .await;
                }
            } else if args.world.get_max_local_raw_brightness(&args.position.up()) >= 9
                && rand::rng().random_ratio(1, 7)
            {
                Self::advance_tree(args.world, args.position, props, false).await;
            }
        })
    }

    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        let props = MangrovePropaguleLikeProperties::from_state_id(args.state_id, args.block);
        !props.hanging || props.age < MAX_AGE
    }

    fn is_bonemeal_success(&self, args: BonemealArgs<'_>) -> bool {
        let props = MangrovePropaguleLikeProperties::from_state_id(args.state_id, args.block);
        if props.hanging {
            props.age < MAX_AGE
        } else {
            rand::rng().random_bool(0.45)
        }
    }

    fn perform_bonemeal<'a>(&'a self, args: BonemealArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let mut props =
                MangrovePropaguleLikeProperties::from_state_id(args.state_id, args.block);
            if props.hanging && props.age < MAX_AGE {
                props.age += 1;
                args.world
                    .set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;
            } else if !props.hanging {
                Self::advance_tree(args.world, args.position, props, true).await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planted_state_matches_vanilla_properties() {
        for waterlogged in [false, true] {
            let state =
                MangrovePropaguleBlock::placed_state(&Block::MANGROVE_PROPAGULE, waterlogged);
            let props =
                MangrovePropaguleLikeProperties::from_state_id(state, &Block::MANGROVE_PROPAGULE);
            assert_eq!(props.age, MAX_AGE);
            assert!(!props.hanging);
            assert_eq!(props.stage, 0);
            assert_eq!(props.waterlogged, waterlogged);
        }
    }

    #[test]
    fn hanging_state_clamps_age_and_preserves_defaults() {
        let state = MangrovePropaguleBlock::create_new_hanging_propagule(u8::MAX);
        let props =
            MangrovePropaguleLikeProperties::from_state_id(state, &Block::MANGROVE_PROPAGULE);
        assert!(MangrovePropaguleBlock::is_hanging(&props));
        assert!(MangrovePropaguleBlock::is_fully_grown(&props));
        assert_eq!(props.stage, 0);
    }

    #[test]
    fn vanilla_support_tags_include_expected_blocks() {
        assert!(
            Block::MANGROVE_LEAVES
                .has_tag(&tag::Block::MINECRAFT_SUPPORTS_HANGING_MANGROVE_PROPAGULE)
        );
        assert!(Block::MUD.has_tag(&tag::Block::MINECRAFT_SUPPORTS_MANGROVE_PROPAGULE));
        assert!(Block::DIRT.has_tag(&tag::Block::MINECRAFT_SUPPORTS_MANGROVE_PROPAGULE));
    }
}
