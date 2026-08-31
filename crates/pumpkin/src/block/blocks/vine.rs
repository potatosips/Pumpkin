use std::collections::HashSet;

use crate::{
    block::{
        BlockBehaviour, BlockFuture, BlockIsReplacing, CanPlaceAtArgs, CanUpdateAtArgs,
        GetStateForNeighborUpdateArgs, OnPlaceArgs, RandomTickArgs, UseWithItemArgs,
        registry::BlockActionResult,
    },
    entity::{EntityBase, player::Player},
};
use pumpkin_data::{
    Block, BlockDirection, BlockStateId, FacingExt,
    block_properties::{BlockProperties, VineLikeProperties},
    item::Item,
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use rand::RngExt;

#[pumpkin_block("minecraft:vine")]
pub struct VineBlock;

impl BlockBehaviour for VineBlock {
    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if !args.world.level_info.load().game_rules.spread_vines
                || rand::rng().random_range(0..4) != 0
            {
                return;
            }

            let direction = BlockDirection::all()[rand::rng().random_range(0..6)];
            spread_vine(args.world, args.block, args.position, direction).await;
        })
    }

    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if let BlockIsReplacing::Itself(state_id) = args.replacing {
                let (Some(direction), _) = get_accurate_direction(
                    args.world,
                    args.position,
                    Some(args.player),
                    args.direction,
                    true,
                ) else {
                    return Block::AIR.default_state.id;
                };
                let mut props = VineLikeProperties::from_state_id(state_id, args.block);
                vine_direction_mapper(direction, &mut props);
                return props.to_state_id(args.block);
            }
            let (Some(direction), _) = get_accurate_direction(
                args.world,
                args.position,
                Some(args.player),
                args.direction,
                false,
            ) else {
                return Block::AIR.default_state.id;
            };
            let mut props = VineLikeProperties::default(args.block);
            vine_direction_mapper(direction, &mut props);
            props.to_state_id(args.block)
        })
    }
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_vine_at(
            args.block_accessor,
            args.position,
            args.direction,
            args.player,
            false,
        )
    }
    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let old_props = VineLikeProperties::from_state_id(args.state_id, args.block);
            let old_directions = get_vine_block_directions(old_props);
            let mut new_directions = old_directions.clone();
            for old_dir in old_directions {
                if !can_support_vine_at(args.world, args.position, old_dir) {
                    new_directions.remove(&old_dir);
                }
            }
            if new_directions.is_empty() {
                return Block::AIR.default_state.id;
            }
            let mut new_props = VineLikeProperties::default(args.block);

            for new_dir in new_directions {
                vine_direction_mapper(new_dir, &mut new_props);
            }

            new_props.to_state_id(args.block)
        })
    }
    fn can_update_at(&self, args: CanUpdateAtArgs<'_>) -> bool {
        get_accurate_direction(
            args.world,
            args.position,
            Some(args.player),
            args.direction,
            true,
        )
        .0
        .is_some()
    }
    fn use_with_item<'a>(
        &'a self,
        args: UseWithItemArgs<'a>,
    ) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async move {
            let state = args.world.get_block_state(args.position);
            let mut props = VineLikeProperties::from_state_id(state.id, args.block);

            let item = args.item_stack.item;

            if item.id != Item::VINE.id {
                return BlockActionResult::Pass;
            }
            let (Some(accurate_dir), _) = get_accurate_direction(
                args.world.as_ref(),
                args.position,
                Some(args.player),
                BlockDirection::Down,
                true,
            ) else {
                return BlockActionResult::Fail;
            };
            vine_direction_mapper(accurate_dir, &mut props);

            args.world
                .set_block_state(
                    args.position,
                    props.to_state_id(args.block),
                    BlockFlags::NOTIFY_ALL,
                )
                .await;
            BlockActionResult::Consume
        })
    }
}

async fn spread_vine(
    world: &std::sync::Arc<crate::world::World>,
    vine: &Block,
    position: &BlockPos,
    direction: BlockDirection,
) {
    let state_id = world.get_block_state_id(position);
    let props = VineLikeProperties::from_state_id(state_id, vine);

    if is_horizontal(direction) && !has_direction(&props, direction) {
        if !can_spread(world, position) {
            return;
        }
        let target = position.offset(direction.to_offset());
        let target_state = world.get_block_state(&target);
        if target_state.is_air() {
            let (clockwise, counterclockwise) = perpendiculars(direction);
            let clockwise_set = has_direction(&props, clockwise);
            let counterclockwise_set = has_direction(&props, counterclockwise);

            if clockwise_set && can_support_vine_at(world.as_ref(), &target, clockwise) {
                place_vine(world, vine, &target, clockwise).await;
            } else if counterclockwise_set
                && can_support_vine_at(world.as_ref(), &target, counterclockwise)
            {
                place_vine(world, vine, &target, counterclockwise).await;
            } else {
                let opposite = direction.opposite();
                let clockwise_target = target.offset(clockwise.to_offset());
                let counterclockwise_target = target.offset(counterclockwise.to_offset());
                if clockwise_set && world.get_block_state(&clockwise_target).is_air() {
                    if can_support_vine_at(world.as_ref(), &clockwise_target, opposite) {
                        place_vine(world, vine, &clockwise_target, opposite).await;
                    }
                } else if counterclockwise_set
                    && world.get_block_state(&counterclockwise_target).is_air()
                {
                    if can_support_vine_at(world.as_ref(), &counterclockwise_target, opposite) {
                        place_vine(world, vine, &counterclockwise_target, opposite).await;
                    }
                } else if rand::rng().random::<f32>() < 0.05 {
                    if can_support_vine_at(world.as_ref(), &target, BlockDirection::Up) {
                        place_vine(world, vine, &target, BlockDirection::Up).await;
                    }
                }
            }
        } else if can_support_vine_at(world.as_ref(), position, direction) {
            let mut updated = props;
            vine_direction_mapper(direction, &mut updated);
            world
                .set_block_state(position, updated.to_state_id(vine), BlockFlags::NOTIFY_ALL)
                .await;
        }
        return;
    }

    if direction == BlockDirection::Up
        && position.0.y < world.dimension.min_y + world.dimension.height - 1
    {
        if can_support_vine_at(world.as_ref(), position, BlockDirection::Up) {
            let mut updated = props;
            vine_direction_mapper(BlockDirection::Up, &mut updated);
            world
                .set_block_state(position, updated.to_state_id(vine), BlockFlags::NOTIFY_ALL)
                .await;
            return;
        }

        let above = position.up();
        if world.get_block_state(&above).is_air() && can_spread(world, position) {
            let mut copied = VineLikeProperties::default(vine);
            for horizontal in horizontal_directions() {
                if rand::random::<bool>() && has_direction(&props, horizontal) {
                    if can_support_vine_at(world.as_ref(), &above, horizontal) {
                        vine_direction_mapper(horizontal, &mut copied);
                    }
                }
            }
            if has_horizontal_connection(&copied) {
                world
                    .set_block_state(&above, copied.to_state_id(vine), BlockFlags::NOTIFY_ALL)
                    .await;
            }
        }
        return;
    }

    if position.0.y > world.min_y {
        let below = position.down();
        let below_block = world.get_block(&below);
        if below_block == &Block::AIR || below_block == vine {
            let mut below_props = if below_block == vine {
                VineLikeProperties::from_state_id(world.get_block_state_id(&below), vine)
            } else {
                VineLikeProperties::default(vine)
            };
            let old = below_props;
            for horizontal in horizontal_directions() {
                if rand::random::<bool>() && has_direction(&props, horizontal) {
                    vine_direction_mapper(horizontal, &mut below_props);
                }
            }
            if below_props != old && has_horizontal_connection(&below_props) {
                world
                    .set_block_state(
                        &below,
                        below_props.to_state_id(vine),
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;
            }
        }
    }
}

async fn place_vine(
    world: &std::sync::Arc<crate::world::World>,
    vine: &Block,
    position: &BlockPos,
    face: BlockDirection,
) {
    let mut props = VineLikeProperties::default(vine);
    vine_direction_mapper(face, &mut props);
    world
        .set_block_state(position, props.to_state_id(vine), BlockFlags::NOTIFY_ALL)
        .await;
}

fn can_spread(world: &std::sync::Arc<crate::world::World>, position: &BlockPos) -> bool {
    let mut remaining = 5;
    for x in -4..=4 {
        for y in -1..=1 {
            for z in -4..=4 {
                if world.get_block(&position.offset((x, y, z).into())) == &Block::VINE {
                    remaining -= 1;
                    if remaining <= 0 {
                        return false;
                    }
                }
            }
        }
    }
    true
}

const fn horizontal_directions() -> [BlockDirection; 4] {
    [
        BlockDirection::North,
        BlockDirection::East,
        BlockDirection::South,
        BlockDirection::West,
    ]
}

const fn is_horizontal(direction: BlockDirection) -> bool {
    matches!(
        direction,
        BlockDirection::North | BlockDirection::East | BlockDirection::South | BlockDirection::West
    )
}

const fn perpendiculars(direction: BlockDirection) -> (BlockDirection, BlockDirection) {
    match direction {
        BlockDirection::North => (BlockDirection::East, BlockDirection::West),
        BlockDirection::East => (BlockDirection::South, BlockDirection::North),
        BlockDirection::South => (BlockDirection::West, BlockDirection::East),
        BlockDirection::West => (BlockDirection::North, BlockDirection::South),
        _ => (BlockDirection::North, BlockDirection::South),
    }
}

const fn has_direction(props: &VineLikeProperties, direction: BlockDirection) -> bool {
    match direction {
        BlockDirection::Up => props.up,
        BlockDirection::North => props.north,
        BlockDirection::South => props.south,
        BlockDirection::East => props.east,
        BlockDirection::West => props.west,
        BlockDirection::Down => false,
    }
}

const fn has_horizontal_connection(props: &VineLikeProperties) -> bool {
    props.north || props.south || props.east || props.west
}
pub fn get_nearest_looking_directions(
    player: &Player,
    replace_clicked: bool,
    clicked_face: BlockDirection,
) -> [BlockDirection; 6] {
    let mut directions: [BlockDirection; 6] = {
        let fs = player.get_entity().get_entity_facing_order();
        [
            fs[0].to_block_direction(),
            fs[1].to_block_direction(),
            fs[2].to_block_direction(),
            fs[3].to_block_direction(),
            fs[4].to_block_direction(),
            fs[5].to_block_direction(),
        ]
    };

    if !replace_clicked {
        let target = clicked_face.opposite();

        let mut index = 0;

        while index < directions.len() && directions[index] != target {
            index += 1;
        }

        if index > 0 {
            directions.copy_within(0..index, 1);
            directions[0] = target;
        }
    }
    directions
}
fn can_place_vine_at(
    block_accessor: &dyn BlockAccessor,
    block_pos: &BlockPos,
    click_direction_wrapper: Option<BlockDirection>,
    player_wrapper: Option<&Player>,
    replacing: bool,
) -> bool {
    let Some(click_direction) = click_direction_wrapper else {
        return false;
    };
    let (direction, _) = get_accurate_direction(
        block_accessor,
        block_pos,
        player_wrapper,
        click_direction,
        replacing,
    );
    let Some(direction) = direction else {
        return false;
    };

    if !can_support_vine_at(block_accessor, block_pos, direction) {
        return false;
    }
    true
}

fn can_support_vine_at(
    block_accessor: &dyn BlockAccessor,
    vine_pos: &BlockPos,
    direction: BlockDirection,
) -> bool {
    if direction == BlockDirection::Down {
        return false;
    }

    let support_pos = vine_pos.offset(direction.to_offset());
    let support_state = block_accessor.get_block_state(&support_pos);
    let support_face = direction.opposite();
    if support_state.is_side_solid(support_face) {
        return true;
    }

    if is_horizontal(direction) {
        let (above_block, above_state) = block_accessor.get_block_and_state(&vine_pos.up());
        if above_block == &Block::VINE {
            let above_props = VineLikeProperties::from_state_id(above_state.id, above_block);
            return has_direction(&above_props, direction);
        }
    }

    false
}
//returns (accurate direction, boolean)
// true if this direction is for hanging vine
// false if it is not
fn get_accurate_direction(
    block_accessor: &dyn BlockAccessor,
    block_pos: &BlockPos,
    player_wrapper: Option<&Player>,
    click_direction: BlockDirection,
    replacing: bool,
) -> (Option<BlockDirection>, bool) {
    let clicked_block = block_accessor.get_block(&block_pos.offset(click_direction.to_offset()));
    if !replacing && clicked_block == &Block::VINE && click_direction != BlockDirection::Up {
        return (None, false);
    }

    if can_support_vine_at(block_accessor, block_pos, click_direction) {
        return (Some(click_direction), false);
    }
    let (replacing_block, replacing_block_state) = block_accessor.get_block_and_state(block_pos);
    let already_active_directions = if replacing_block == &Block::VINE {
        let props = VineLikeProperties::from_state_id(replacing_block_state.id, replacing_block);
        get_vine_block_directions(props)
    } else {
        HashSet::new()
    };
    if let Some(player) = player_wrapper {
        let mut up = false;
        for dir in get_nearest_looking_directions(player, replacing, click_direction) {
            if dir != BlockDirection::Down && !already_active_directions.contains(&dir) {
                if !can_support_vine_at(block_accessor, block_pos, dir) {
                    continue;
                }
                if dir == BlockDirection::Up && !replacing {
                    up = true;
                    continue;
                }

                return (Some(dir), false);
            }
        }
        if up {
            return (Some(BlockDirection::Up), false);
        }
    }
    (None, false)
}
fn get_vine_block_directions(props: VineLikeProperties) -> HashSet<BlockDirection> {
    let mut set = HashSet::new();
    if props.north {
        set.insert(BlockDirection::North);
    }
    if props.south {
        set.insert(BlockDirection::South);
    }
    if props.east {
        set.insert(BlockDirection::East);
    }
    if props.west {
        set.insert(BlockDirection::West);
    }
    if props.up {
        set.insert(BlockDirection::Up);
    }
    set
}
const fn vine_direction_mapper(direction: BlockDirection, props: &mut VineLikeProperties) {
    match direction {
        BlockDirection::Down => (),
        BlockDirection::Up => props.up = true,
        BlockDirection::North => props.north = true,
        BlockDirection::South => props.south = true,
        BlockDirection::West => props.west = true,
        BlockDirection::East => props.east = true,
    }
}

#[cfg(test)]
mod tests {
    use pumpkin_data::Block;
    use pumpkin_data::block_properties::{BlockProperties, VineLikeProperties};

    #[test]
    fn vine_block_id_parity() {
        assert_eq!(Block::VINE.name, "vine");
    }

    #[test]
    fn vine_default_state_parity() {
        // Default vine state: all faces false
        let default_props =
            VineLikeProperties::from_state_id(Block::VINE.default_state.id, &Block::VINE);
        assert!(Block::VINE.default_state.has_random_ticks());
        let mut north = VineLikeProperties::default(&Block::VINE);
        north.north = true;
        assert!(
            north
                .to_state_id(&Block::VINE)
                .to_state()
                .has_random_ticks()
        );
        assert!(!default_props.up);
        assert!(!default_props.north);
        assert!(!default_props.south);
        assert!(!default_props.east);
        assert!(!default_props.west);
    }

    #[test]
    fn vine_properties_encoding_decoding_parity() {
        // Vine has 5 boolean face properties (up, north, south, east, west) = 32 states
        let mut count = 0;
        for up in [true, false] {
            for north in [true, false] {
                for south in [true, false] {
                    for east in [true, false] {
                        for west in [true, false] {
                            let props = VineLikeProperties {
                                up,
                                north,
                                south,
                                east,
                                west,
                            };
                            let state_id = props.to_state_id(&Block::VINE);
                            let rt = VineLikeProperties::from_state_id(state_id, &Block::VINE);
                            assert_eq!(rt.up, up);
                            assert_eq!(rt.north, north);
                            assert_eq!(rt.south, south);
                            assert_eq!(rt.east, east);
                            assert_eq!(rt.west, west);
                            count += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(count, 32, "Expected 32 vine states");
    }
}
