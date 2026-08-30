use std::sync::Arc;

use pumpkin_data::block_properties::{BlockProperties, BrownMushroomBlockLikeProperties};
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockId, BlockState, BlockStateId, tag};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use rand::RngExt;

use crate::block::{
    BlockBehaviour, BlockFuture, BlockMetadata, BonemealArgs, CanPlaceAtArgs,
    GetStateForNeighborUpdateArgs, RandomTickArgs, blocks::plant::PlantBlockBase,
};
use crate::plugin::api::events::world::structure_grow::{StructureGrowEvent, TreeType};
use crate::world::World;

pub struct MushroomPlantBlock;

fn mushroom_tree_height(rng: &mut impl rand::Rng) -> i32 {
    let mut height = rng.random_range(0..3) + 4;
    if rng.random_range(0..12) == 0 {
        height *= 2;
    }
    height
}

const fn brown_cap_position_included(x: i32, z: i32) -> bool {
    !((x == -3 || x == 3) && (z == -3 || z == 3))
}

const fn red_cap_position_included(below_top: bool, x: i32, z: i32, radius: i32) -> bool {
    let on_x_edge = x == -radius || x == radius;
    let on_z_edge = z == -radius || z == radius;
    !below_top || on_x_edge != on_z_edge
}

impl MushroomPlantBlock {
    #[must_use]
    const fn may_place_on(state: &BlockState) -> bool {
        state.is_solid() && (state.is_full_cube() || state.is_solid_block())
    }

    fn can_survive(
        block_accessor: &dyn BlockAccessor,
        world: Option<&World>,
        pos: &BlockPos,
    ) -> bool {
        let below_pos = pos.down();
        if block_accessor
            .get_block(&below_pos)
            .has_tag(&tag::Block::MINECRAFT_OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT)
        {
            return true;
        }

        world.is_none_or(|world| world.get_max_local_raw_brightness(pos) < 13)
            && Self::may_place_on(block_accessor.get_block_state(&below_pos))
    }

    pub async fn grow_mushroom(
        world: &Arc<World>,
        pos: &BlockPos,
        block: &Block,
        _state_id: BlockStateId,
    ) -> bool {
        let species = if block == &Block::BROWN_MUSHROOM {
            TreeType::BrownMushroom
        } else if block == &Block::RED_MUSHROOM {
            TreeType::RedMushroom
        } else {
            TreeType::Custom
        };

        let mut event = StructureGrowEvent::new(*pos, species, true);
        if let Some(server) = world.server.upgrade() {
            server.plugin_manager.fire(&server, &mut event).await;
        }
        if event.cancelled {
            return false;
        }

        let tree_height = mushroom_tree_height(&mut rand::rng());
        if !world.is_in_height_limit(pos.0.y + tree_height + 1) {
            return false;
        }

        let foliage_radius = if block == &Block::BROWN_MUSHROOM {
            3
        } else {
            2
        };
        for dy in 0..=tree_height + 1 {
            let radius = if dy <= 3 { 0 } else { foliage_radius };
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    let check_pos = pos.add(dx, dy, dz);
                    if check_pos == *pos {
                        continue;
                    }
                    if !world.is_loaded(&check_pos) {
                        return false;
                    }
                    let check_state = world.get_block_state(&check_pos);
                    let check_block = world.get_block(&check_pos);
                    if !(check_state.is_air()
                        || check_block.has_tag(&tag::Block::MINECRAFT_LEAVES)
                        || check_block.has_tag(&tag::Block::MINECRAFT_REPLACEABLE_BY_MUSHROOMS)
                        || check_state.replaceable())
                    {
                        return false;
                    }
                }
            }
        }

        world
            .set_block_state(pos, BlockStateId::AIR, BlockFlags::NOTIFY_ALL)
            .await;
        if block == &Block::BROWN_MUSHROOM {
            place_huge_brown_mushroom(world, pos, tree_height).await;
        } else if block == &Block::RED_MUSHROOM {
            place_huge_red_mushroom(world, pos, tree_height).await;
        }
        true
    }
}

async fn place_huge_brown_mushroom(world: &Arc<World>, pos: &BlockPos, tree_height: i32) {
    let radius = 3;
    let cap_y = pos.0.y + tree_height;
    for x in -radius..=radius {
        for z in -radius..=radius {
            if !brown_cap_position_included(x, z) {
                continue;
            }
            let on_x_edge = x == -radius || x == radius;
            let on_z_edge = z == -radius || z == radius;
            let props = BrownMushroomBlockLikeProperties {
                up: true,
                down: false,
                west: x == -radius || (on_z_edge && x == 1 - radius),
                east: x == radius || (on_z_edge && x == radius - 1),
                north: z == -radius || (on_x_edge && z == 1 - radius),
                south: z == radius || (on_x_edge && z == radius - 1),
            };
            let cap_pos = BlockPos::new(pos.0.x + x, cap_y, pos.0.z + z);
            world
                .set_block_state(
                    &cap_pos,
                    props.to_state_id(&Block::BROWN_MUSHROOM_BLOCK),
                    BlockFlags::NOTIFY_ALL,
                )
                .await;
        }
    }
    place_mushroom_stem(world, pos, tree_height).await;
}

async fn place_huge_red_mushroom(world: &Arc<World>, pos: &BlockPos, tree_height: i32) {
    let radius = 2;
    for y in (tree_height - 3)..=tree_height {
        let layer_radius = if y < tree_height { radius } else { radius - 1 };
        for x in -layer_radius..=layer_radius {
            for z in -layer_radius..=layer_radius {
                if !red_cap_position_included(y < tree_height, x, z, layer_radius) {
                    continue;
                }
                let props = BrownMushroomBlockLikeProperties {
                    up: y >= tree_height - 1,
                    down: false,
                    west: x < 0,
                    east: x > 0,
                    north: z < 0,
                    south: z > 0,
                };
                let cap_pos = BlockPos::new(pos.0.x + x, pos.0.y + y, pos.0.z + z);
                world
                    .set_block_state(
                        &cap_pos,
                        props.to_state_id(&Block::RED_MUSHROOM_BLOCK),
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;
            }
        }
    }
    place_mushroom_stem(world, pos, tree_height).await;
}

async fn place_mushroom_stem(world: &Arc<World>, pos: &BlockPos, tree_height: i32) {
    let props = BrownMushroomBlockLikeProperties {
        up: false,
        down: false,
        north: true,
        east: true,
        south: true,
        west: true,
    };
    let state = props.to_state_id(&Block::MUSHROOM_STEM);
    for y in 0..tree_height {
        world
            .set_block_state(
                &BlockPos::new(pos.0.x, pos.0.y + y, pos.0.z),
                state,
                BlockFlags::NOTIFY_ALL,
            )
            .await;
    }
}

impl BlockMetadata for MushroomPlantBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::BROWN_MUSHROOM, BlockId::RED_MUSHROOM].into()
    }
}

impl BlockBehaviour for MushroomPlantBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        Self::can_survive(args.block_accessor, args.world, args.position)
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if Self::can_survive(args.world, Some(args.world), args.position) {
                args.state_id
            } else {
                pumpkin_data::Block::AIR.default_state.id
            }
        })
    }

    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if rand::rng().random_range(0..25) != 0 {
                return;
            }
            let pos = *args.position;
            let world = args.world;
            let mut remaining_density = 5;
            for dx in -4..=4 {
                for dy in -1..=1 {
                    for dz in -4..=4 {
                        let check_pos = pos.add(dx, dy, dz);
                        if world.is_loaded(&check_pos) && world.get_block(&check_pos) == args.block
                        {
                            remaining_density -= 1;
                            if remaining_density <= 0 {
                                return;
                            }
                        }
                    }
                }
            }

            let state_id = world.get_block_state_id(&pos);
            let mut current_pos = pos;
            let mut offset = random_spread_offset(current_pos);
            for _ in 0..4 {
                if world.is_loaded(&offset)
                    && world.get_block_state(&offset).is_air()
                    && Self::can_survive(world.as_ref(), Some(world.as_ref()), &offset)
                {
                    current_pos = offset;
                }
                offset = random_spread_offset(current_pos);
            }
            if world.is_loaded(&offset)
                && world.get_block_state(&offset).is_air()
                && Self::can_survive(world.as_ref(), Some(world.as_ref()), &offset)
            {
                world
                    .set_block_state(&offset, state_id, BlockFlags::NOTIFY_LISTENERS)
                    .await;
            }
        })
    }

    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        let foliage_radius = if args.block == &Block::BROWN_MUSHROOM {
            3
        } else {
            2
        };
        args.world
            .is_in_height_limit(args.position.0.y + 4 + foliage_radius)
    }

    fn is_bonemeal_success(&self, _args: BonemealArgs<'_>) -> bool {
        rand::rng().random::<f32>() < 0.4
    }

    fn perform_bonemeal<'a>(&'a self, args: BonemealArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            Self::grow_mushroom(args.world, args.position, args.block, args.state_id).await;
        })
    }
}

fn random_spread_offset(pos: BlockPos) -> BlockPos {
    pos.add(
        rand::rng().random_range(0..3) - 1,
        rand::rng().random_range(0..2) - rand::rng().random_range(0..2),
        rand::rng().random_range(0..3) - 1,
    )
}

impl PlantBlockBase for MushroomPlantBlock {
    fn can_plant_on_top(&self, block_accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        Self::may_place_on(block_accessor.get_block_state(pos))
    }

    fn can_place_at(&self, block_accessor: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
        Self::can_survive(block_accessor, None, block_pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn mushroom_block_id_parity() {
        assert_eq!(Block::BROWN_MUSHROOM.name, "brown_mushroom");
        assert_eq!(Block::RED_MUSHROOM.name, "red_mushroom");
        assert_eq!(
            MushroomPlantBlock::ids().as_ref(),
            &[BlockId::BROWN_MUSHROOM, BlockId::RED_MUSHROOM]
        );
    }

    #[test]
    fn mushroom_default_state_parity() {
        assert_ne!(
            Block::BROWN_MUSHROOM.default_state.id,
            Block::AIR.default_state.id
        );
        assert_ne!(
            Block::RED_MUSHROOM.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn mushroom_supports_tag_parity() {
        assert!(
            Block::MYCELIUM.has_tag(&tag::Block::MINECRAFT_OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT)
        );
        assert!(Block::PODZOL.has_tag(&tag::Block::MINECRAFT_OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT));
        assert!(
            Block::CRIMSON_NYLIUM
                .has_tag(&tag::Block::MINECRAFT_OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT)
        );
        assert!(
            Block::WARPED_NYLIUM
                .has_tag(&tag::Block::MINECRAFT_OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT)
        );
    }

    #[test]
    fn ordinary_mushroom_support_requires_a_solid_rendering_surface() {
        assert!(MushroomPlantBlock::may_place_on(Block::STONE.default_state));
        assert!(!MushroomPlantBlock::may_place_on(Block::AIR.default_state));
        assert!(!MushroomPlantBlock::may_place_on(
            Block::WATER.default_state
        ));
    }

    #[test]
    fn huge_brown_mushroom_cap_has_vanilla_shape() {
        let count = (-3..=3)
            .flat_map(|x| (-3..=3).map(move |z| (x, z)))
            .filter(|&(x, z)| brown_cap_position_included(x, z))
            .count();
        assert_eq!(count, 45);
        assert!(!brown_cap_position_included(-3, -3));
        assert!(brown_cap_position_included(0, 0));
    }

    #[test]
    fn huge_red_mushroom_cap_has_vanilla_shape() {
        let lower_layer = (-2..=2)
            .flat_map(|x| (-2..=2).map(move |z| (x, z)))
            .filter(|&(x, z)| red_cap_position_included(true, x, z, 2))
            .count();
        let crown = (-1..=1)
            .flat_map(|x| (-1..=1).map(move |z| (x, z)))
            .filter(|&(x, z)| red_cap_position_included(false, x, z, 1))
            .count();
        assert_eq!(lower_layer, 12);
        assert_eq!(crown, 9);
        assert_eq!(lower_layer * 3 + crown, 45);
    }
}
