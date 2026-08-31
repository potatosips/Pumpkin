use pumpkin_data::{
    Block, BlockDirection, BlockStateId, FacingExt, HorizontalFacingExt,
    block_properties::{BlockProperties, CocoaLikeProperties, Facing},
    tag::{self, Taggable},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use rand::RngExt;

use crate::block::{
    BlockBehaviour, BlockFuture, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    OnPlaceArgs, RandomTickArgs,
};
use crate::entity::EntityBase;

#[pumpkin_block("minecraft:cocoa")]
pub struct CocoaBlock;

impl BlockBehaviour for CocoaBlock {
    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let mut props = CocoaLikeProperties::default(args.block);
            for direction in args.player.get_entity().get_entity_facing_order() {
                if matches!(direction, Facing::Up | Facing::Down) {
                    continue;
                }
                let support_direction = direction.to_block_direction();
                if has_support(args.world, args.position, support_direction)
                    && let Some(facing) = direction.opposite().to_horizontal_facing()
                {
                    props.facing = facing;
                    return props.to_state_id(args.block);
                }
            }
            BlockStateId::AIR
        })
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        BlockDirection::horizontal().into_iter().any(|direction| {
            has_support(
                args.block_accessor,
                args.position,
                direction.to_block_direction(),
            )
        })
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let props = CocoaLikeProperties::from_state_id(args.state_id, args.block);
            let support_direction = props.facing.to_block_direction().opposite();
            if args.direction == support_direction
                && !has_support(args.world, args.position, support_direction)
            {
                return BlockStateId::AIR;
            }
            args.state_id
        })
    }

    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if rand::rng().random_range(0..5) == 0 {
                grow(args.world, args.position, args.block).await;
            }
        })
    }

    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        CocoaLikeProperties::from_state_id(args.world.get_block_state_id(args.position), args.block)
            .age
            < 2
    }

    fn perform_bonemeal<'a>(&'a self, args: BonemealArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move { grow(args.world, args.position, args.block).await })
    }
}

fn has_support(world: &dyn BlockAccessor, position: &BlockPos, direction: BlockDirection) -> bool {
    world
        .get_block(&position.offset(direction.to_offset()))
        .has_tag(&tag::Block::MINECRAFT_SUPPORTS_COCOA)
}

async fn grow(world: &std::sync::Arc<crate::world::World>, position: &BlockPos, block: &Block) {
    let state_id = world.get_block_state_id(position);
    if state_id.to_block_id() != block.id {
        return;
    }
    let mut props = CocoaLikeProperties::from_state_id(state_id, block);
    if props.age >= 2 {
        return;
    }
    props.age += 1;
    world
        .set_block_state(
            position,
            props.to_state_id(block),
            BlockFlags::NOTIFY_LISTENERS,
        )
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::block_properties::HorizontalFacing;

    #[test]
    fn cocoa_properties_roundtrip() {
        for age in 0..=2 {
            for facing in HorizontalFacing::all() {
                let props = CocoaLikeProperties { age, facing };
                assert!(
                    CocoaLikeProperties::from_state_id(
                        props.to_state_id(&Block::COCOA),
                        &Block::COCOA,
                    ) == props
                );
            }
        }
    }

    #[test]
    fn cocoa_support_tag_contains_jungle_logs() {
        assert!(Block::JUNGLE_LOG.has_tag(&tag::Block::MINECRAFT_SUPPORTS_COCOA));
        assert!(Block::STRIPPED_JUNGLE_LOG.has_tag(&tag::Block::MINECRAFT_SUPPORTS_COCOA));
        assert!(!Block::OAK_LOG.has_tag(&tag::Block::MINECRAFT_SUPPORTS_COCOA));
    }
}
