use pumpkin_data::block_properties::{BlockProperties, Facing};
use pumpkin_data::{Block, BlockStateId, FacingExt};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;

use crate::block::blocks::piston::piston::try_move;
use crate::block::{BlockBehaviour, BlockFuture, CanPlaceAtArgs, GetStateForNeighborUpdateArgs};
use crate::block::{BrokenArgs, OnNeighborUpdateArgs};

use super::piston::PistonProps;

pub(crate) type PistonHeadProperties = pumpkin_data::block_properties::PistonHeadLikeProperties;

#[pumpkin_block("minecraft:piston_head")]
pub struct PistonHeadBlock;

impl BlockBehaviour for PistonHeadBlock {
    fn broken<'a>(&'a self, args: BrokenArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let props = PistonHeadProperties::from_state_id(args.state.id, &Block::PISTON_HEAD);
            let pos = args
                .position
                .offset(props.facing.opposite().to_block_direction().to_offset());
            let (new_block, new_state) = args.world.get_block_and_state_id(&pos);
            if &Block::PISTON == new_block || &Block::STICKY_PISTON == new_block {
                let props = PistonProps::from_state_id(new_state, new_block);
                if props.extended {
                    // TODO: use player
                    args.world
                        .break_block(&pos, None, BlockFlags::SKIP_DROPS)
                        .await;
                }
            }
        })
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let props = PistonHeadProperties::from_state_id(args.state_id, &Block::PISTON_HEAD);
            if args.direction == props.facing.opposite().to_block_direction() {
                let piston_pos = args.position.offset(args.direction.to_offset());
                let (piston_block, piston_state_id) =
                    args.world.get_block_and_state_id(&piston_pos);
                if &Block::PISTON == piston_block || &Block::STICKY_PISTON == piston_block {
                    let p_props = PistonProps::from_state_id(piston_state_id, piston_block);
                    if !p_props.extended || p_props.facing != props.facing {
                        return Block::AIR.default_state.id;
                    }
                } else {
                    return Block::AIR.default_state.id;
                }
            }
            args.state_id
        })
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let state = args.block_accessor.get_block_state(args.position);
        let props = PistonHeadProperties::from_state_id(state.id, &Block::PISTON_HEAD);
        let piston_pos = args
            .position
            .offset(props.facing.opposite().to_block_direction().to_offset());
        let (piston_block, piston_state) = args.block_accessor.get_block_and_state(&piston_pos);
        if &Block::PISTON == piston_block || &Block::STICKY_PISTON == piston_block {
            let p_props = PistonProps::from_state_id(piston_state.id, piston_block);
            p_props.extended && p_props.facing == props.facing
        } else {
            false
        }
    }
    fn on_neighbor_update<'a>(&'a self, args: OnNeighborUpdateArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let head_state_id = args.world.get_block_state_id(args.position);
            let head_props =
                PistonHeadProperties::from_state_id(head_state_id, &Block::PISTON_HEAD);
            if head_props.facing != Facing::Up {
                return;
            }
            let piston_pos = args.position.offset(
                head_props
                    .facing
                    .opposite()
                    .to_block_direction()
                    .to_offset(),
            );
            let piston_block = args.world.get_block(&piston_pos);
            if &Block::PISTON == piston_block || &Block::STICKY_PISTON == piston_block {
                let up_pos = args
                    .position
                    .offset(head_props.facing.to_block_direction().to_offset());
                let upper_block = args.world.get_block(&up_pos);
                if upper_block != &Block::REDSTONE_BLOCK {
                    //Then somebody probably broke the redstone block, try to check if piston should still be extended.
                    try_move(args.world, piston_block, &piston_pos).await;
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;
    use pumpkin_data::block_properties::{BlockProperties, PistonHeadLikeProperties, PistonType};

    #[test]
    fn piston_head_block_id_parity() {
        assert_eq!(Block::PISTON_HEAD.name, "piston_head");
    }

    #[test]
    fn piston_head_default_state_parity() {
        assert_ne!(
            Block::PISTON_HEAD.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn piston_head_properties_parity() {
        for facing in [
            Facing::North,
            Facing::South,
            Facing::East,
            Facing::West,
            Facing::Up,
            Facing::Down,
        ] {
            for short in [true, false] {
                for r#type in [PistonType::Normal, PistonType::Sticky] {
                    let props = PistonHeadLikeProperties {
                        facing,
                        short,
                        r#type,
                    };
                    let state_id = props.to_state_id(&Block::PISTON_HEAD);
                    let rt = PistonHeadLikeProperties::from_state_id(state_id, &Block::PISTON_HEAD);
                    assert_eq!(rt.facing, facing);
                    assert_eq!(rt.short, short);
                    assert_eq!(rt.r#type, r#type);
                }
            }
        }
    }
}
