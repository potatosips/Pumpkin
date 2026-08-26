use crate::{
    block::{
        BlockBehaviour, BlockFuture, BrokenArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
        OnNeighborUpdateArgs, OnPlaceArgs, OnScheduledTickArgs, PlacedArgs,
    },
    entity::falling::FallingEntity,
    entity::player::Player,
};
use pumpkin_data::{
    Block, BlockDirection, BlockId, BlockStateId,
    block_properties::{
        BlockProperties, PointedDripstoneLikeProperties, SpeleothemThickness, VerticalDirection,
    },
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{
    tick::TickPriority,
    world::{BlockAccessor, BlockFlags},
};

#[pumpkin_block("minecraft:pointed_dripstone")]
pub struct DripstoneBlock;

impl DripstoneBlock {
    #[must_use]
    pub fn is_pointed_dripstone_with_dir(
        world: &dyn BlockAccessor,
        pos: &BlockPos,
        dir: VerticalDirection,
    ) -> bool {
        let (block, state) = world.get_block_and_state(pos);
        if block.id == BlockId::POINTED_DRIPSTONE {
            let props = PointedDripstoneLikeProperties::from_state_id(state.id, block);
            props.vertical_direction == dir
        } else {
            false
        }
    }

    #[must_use]
    pub fn can_survive(world: &dyn BlockAccessor, pos: &BlockPos, dir: VerticalDirection) -> bool {
        match dir {
            VerticalDirection::Down => {
                let up_pos = pos.up();
                let (up_block, up_state) = world.get_block_and_state(&up_pos);
                if up_block.id == BlockId::POINTED_DRIPSTONE {
                    let props =
                        PointedDripstoneLikeProperties::from_state_id(up_state.id, up_block);
                    props.vertical_direction == VerticalDirection::Down
                } else {
                    up_state.is_full_cube()
                        || (!up_state.is_air() && !up_state.replaceable() && !up_state.is_liquid())
                }
            }
            VerticalDirection::Up => {
                let down_pos = pos.down();
                let (down_block, down_state) = world.get_block_and_state(&down_pos);
                if down_block.id == BlockId::POINTED_DRIPSTONE {
                    let props =
                        PointedDripstoneLikeProperties::from_state_id(down_state.id, down_block);
                    props.vertical_direction == VerticalDirection::Up
                } else {
                    down_state.is_full_cube()
                        || (!down_state.is_air()
                            && !down_state.replaceable()
                            && !down_state.is_liquid())
                }
            }
        }
    }

    #[must_use]
    pub fn calculate_thickness(
        world: &dyn BlockAccessor,
        pos: &BlockPos,
        dir: VerticalDirection,
        merged: bool,
    ) -> SpeleothemThickness {
        let (growth_pos, root_pos, growth2_pos) = match dir {
            VerticalDirection::Up => (pos.up(), pos.down(), pos.up_height(2)),
            VerticalDirection::Down => (pos.down(), pos.up(), pos.down_height(2)),
        };

        let is_growth = Self::is_pointed_dripstone_with_dir(world, &growth_pos, dir);
        let is_root = Self::is_pointed_dripstone_with_dir(world, &root_pos, dir);

        let opp_dir = match dir {
            VerticalDirection::Up => VerticalDirection::Down,
            VerticalDirection::Down => VerticalDirection::Up,
        };

        if is_growth && is_root {
            let (growth_block, growth_state) = world.get_block_and_state(&growth_pos);
            if growth_block.id == BlockId::POINTED_DRIPSTONE {
                let growth_props =
                    PointedDripstoneLikeProperties::from_state_id(growth_state.id, growth_block);
                if growth_props.thickness == SpeleothemThickness::Tip
                    || growth_props.thickness == SpeleothemThickness::TipMerge
                {
                    SpeleothemThickness::Frustum
                } else {
                    SpeleothemThickness::Middle
                }
            } else {
                SpeleothemThickness::Middle
            }
        } else if is_root {
            if Self::is_pointed_dripstone_with_dir(world, &growth_pos, opp_dir) || merged {
                SpeleothemThickness::TipMerge
            } else {
                SpeleothemThickness::Tip
            }
        } else if is_growth {
            if Self::is_pointed_dripstone_with_dir(world, &growth2_pos, dir) {
                SpeleothemThickness::Base
            } else {
                SpeleothemThickness::Frustum
            }
        } else {
            if Self::is_pointed_dripstone_with_dir(world, &growth_pos, opp_dir) || merged {
                SpeleothemThickness::TipMerge
            } else {
                SpeleothemThickness::Tip
            }
        }
    }
}

impl BlockBehaviour for DripstoneBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_at_pos(
            args.block_accessor,
            args.position,
            args.direction,
            args.player,
        )
    }

    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let mut dripstone_props = PointedDripstoneLikeProperties::default(args.block);
            dripstone_props.waterlogged = args.replacing.water_source();
            let dir = match args.direction {
                BlockDirection::Down => VerticalDirection::Down,
                BlockDirection::Up => VerticalDirection::Up,
                _ => {
                    let (_, pitch) = args.player.rotation();
                    let can_above = DripstoneBlock::can_survive(
                        args.world,
                        args.position,
                        VerticalDirection::Down,
                    );
                    let can_below = DripstoneBlock::can_survive(
                        args.world,
                        args.position,
                        VerticalDirection::Up,
                    );
                    match (can_above, can_below) {
                        (true, true) => {
                            if pitch > 0.0 {
                                VerticalDirection::Up
                            } else {
                                VerticalDirection::Down
                            }
                        }
                        (true, false) => VerticalDirection::Down,
                        (false, true) => VerticalDirection::Up,
                        (false, false) => VerticalDirection::Up,
                    }
                }
            };

            dripstone_props.vertical_direction = dir;
            dripstone_props.thickness =
                DripstoneBlock::calculate_thickness(args.world, args.position, dir, false);
            dripstone_props.to_state_id(&Block::POINTED_DRIPSTONE)
        })
    }

    fn placed<'a>(&'a self, args: PlacedArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let current_state = args.world.get_block_state(args.position);
            let props = PointedDripstoneLikeProperties::from_state_id(current_state.id, args.block);
            if !DripstoneBlock::can_survive(
                args.world.as_ref(),
                args.position,
                props.vertical_direction,
            ) {
                if props.vertical_direction == VerticalDirection::Down {
                    args.world.schedule_block_tick(
                        args.block,
                        *args.position,
                        2,
                        TickPriority::Normal,
                    );
                }
            }
        })
    }

    fn broken<'a>(&'a self, args: BrokenArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let broken_dripstone_props =
                PointedDripstoneLikeProperties::from_state_id(args.state.id, args.block);
            let neighbor_pos = match broken_dripstone_props.vertical_direction {
                VerticalDirection::Up => args.position.down(),
                VerticalDirection::Down => args.position.up(),
            };

            let neighbor_state = args.world.get_block_state(&neighbor_pos);
            if neighbor_state.id.to_block_id() == BlockId::POINTED_DRIPSTONE {
                let mut nb_props =
                    PointedDripstoneLikeProperties::from_state_id(neighbor_state.id, args.block);
                nb_props.thickness = DripstoneBlock::calculate_thickness(
                    args.world.as_ref(),
                    &neighbor_pos,
                    nb_props.vertical_direction,
                    false,
                );
                let new_id = nb_props.to_state_id(args.block);
                args.world
                    .set_block_state(&neighbor_pos, new_id, BlockFlags::NOTIFY_ALL)
                    .await;
            }
        })
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let mut dripstone_props =
                PointedDripstoneLikeProperties::from_state_id(args.state_id, args.block);
            if !DripstoneBlock::can_survive(
                args.world,
                args.position,
                dripstone_props.vertical_direction,
            ) {
                if dripstone_props.vertical_direction == VerticalDirection::Down {
                    args.world.schedule_block_tick(
                        args.block,
                        *args.position,
                        2,
                        TickPriority::Normal,
                    );
                    return args.state_id;
                }
                return Block::AIR.default_state.id;
            }

            let thickness = DripstoneBlock::calculate_thickness(
                args.world,
                args.position,
                dripstone_props.vertical_direction,
                false,
            );
            dripstone_props.thickness = thickness;
            dripstone_props.to_state_id(args.block)
        })
    }

    fn on_neighbor_update<'a>(&'a self, args: OnNeighborUpdateArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let state = args.world.get_block_state(args.position);
            let properties = PointedDripstoneLikeProperties::from_state_id(state.id, args.block);
            if properties.vertical_direction == VerticalDirection::Down
                && !DripstoneBlock::can_survive(
                    args.world.as_ref(),
                    args.position,
                    properties.vertical_direction,
                )
            {
                args.world
                    .schedule_block_tick(args.block, *args.position, 2, TickPriority::Normal);
            }
        })
    }

    fn on_scheduled_tick<'a>(&'a self, args: OnScheduledTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let state = args.world.get_block_state(args.position);
            if state.id.to_block_id() != Block::POINTED_DRIPSTONE.id {
                return;
            }
            let properties =
                PointedDripstoneLikeProperties::from_state_id(state.id, &Block::POINTED_DRIPSTONE);
            if properties.vertical_direction == VerticalDirection::Down
                && !DripstoneBlock::can_survive(
                    args.world.as_ref(),
                    args.position,
                    properties.vertical_direction,
                )
            {
                FallingEntity::replace_spawn(args.world, *args.position, state.id).await;
            }
        })
    }
}

fn can_place_at_pos(
    block_accessor: &dyn BlockAccessor,
    position: &BlockPos,
    placing_direction: Option<BlockDirection>,
    player_option: Option<&Player>,
) -> bool {
    let Some(placing_direction) = placing_direction else {
        let (block, state) = block_accessor.get_block_and_state(position);
        if block.id != BlockId::POINTED_DRIPSTONE {
            return false;
        }
        let props = PointedDripstoneLikeProperties::from_state_id(state.id, block);
        return DripstoneBlock::can_survive(block_accessor, position, props.vertical_direction);
    };

    match placing_direction {
        BlockDirection::Down => {
            DripstoneBlock::can_survive(block_accessor, position, VerticalDirection::Down)
        }
        BlockDirection::Up => {
            DripstoneBlock::can_survive(block_accessor, position, VerticalDirection::Up)
        }
        _ => player_option.map_or(
            DripstoneBlock::can_survive(block_accessor, position, VerticalDirection::Up),
            |_player| {
                DripstoneBlock::can_survive(block_accessor, position, VerticalDirection::Down)
                    || DripstoneBlock::can_survive(block_accessor, position, VerticalDirection::Up)
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn dripstone_block_id_parity() {
        assert_eq!(Block::POINTED_DRIPSTONE.name, "pointed_dripstone");
    }

    #[test]
    fn dripstone_properties_encoding_decoding_parity() {
        assert_eq!(Block::POINTED_DRIPSTONE.states.len(), 20);
        for dir in [VerticalDirection::Up, VerticalDirection::Down] {
            for thickness in [
                SpeleothemThickness::TipMerge,
                SpeleothemThickness::Tip,
                SpeleothemThickness::Frustum,
                SpeleothemThickness::Middle,
                SpeleothemThickness::Base,
            ] {
                for waterlogged in [false, true] {
                    let props = PointedDripstoneLikeProperties {
                        vertical_direction: dir,
                        thickness,
                        waterlogged,
                    };
                    let state_id = props.to_state_id(&Block::POINTED_DRIPSTONE);
                    let decoded = PointedDripstoneLikeProperties::from_state_id(
                        state_id,
                        &Block::POINTED_DRIPSTONE,
                    );
                    assert_eq!(decoded.vertical_direction, dir);
                    assert_eq!(decoded.thickness, thickness);
                    assert_eq!(decoded.waterlogged, waterlogged);
                }
            }
        }
    }

    #[test]
    fn dripstone_default_state_parity() {
        let default_props = PointedDripstoneLikeProperties::from_state_id(
            Block::POINTED_DRIPSTONE.default_state.id,
            &Block::POINTED_DRIPSTONE,
        );
        assert_eq!(default_props.vertical_direction, VerticalDirection::Up);
        assert_eq!(default_props.thickness, SpeleothemThickness::Tip);
        assert_eq!(default_props.waterlogged, false);
    }
}
