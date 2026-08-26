use pumpkin_data::{
    Block, BlockDirection, BlockStateId, HorizontalFacingExt,
    block_properties::{AttachFace, BlockProperties, GrindstoneLikeProperties},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockAccessor;

use crate::block::CanPlaceAtArgs;
use crate::block::{BlockBehaviour, BlockFuture};
use crate::block::{GetStateForNeighborUpdateArgs, OnPlaceArgs};

use super::abstract_wall_mounting::WallMountedBlock;

#[pumpkin_block("minecraft:grindstone")]
pub struct GrindstoneBlock;

impl BlockBehaviour for GrindstoneBlock {
    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let mut props =
                GrindstoneLikeProperties::from_state_id(args.block.default_state.id, args.block);
            (props.face, props.facing) =
                WallMountedBlock::get_placement_face(self, args.player, args.direction);

            props.to_state_id(args.block)
        })
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        // Use the provided direction, or fallback to the current state's direction if missing
        let direction = args
            .direction
            .unwrap_or_else(|| self.get_direction(args.state.id, args.block));

        WallMountedBlock::can_place_at(self, args.block_accessor, args.position, direction)
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move { WallMountedBlock::get_state_for_neighbor_update(self, args).await })
    }
}

impl WallMountedBlock for GrindstoneBlock {
    fn can_place_at<'a>(
        &'a self,
        _world: &'a dyn BlockAccessor,
        _pos: &'a BlockPos,
        _direction: BlockDirection,
    ) -> bool {
        true
    }

    fn get_direction(&self, state_id: BlockStateId, block: &Block) -> BlockDirection {
        let props = GrindstoneLikeProperties::from_state_id(state_id, block);
        match props.face {
            AttachFace::Floor => BlockDirection::Up,
            AttachFace::Ceiling => BlockDirection::Down,
            AttachFace::Wall => props.facing.to_block_direction(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;
    use pumpkin_data::block_properties::{
        AttachFace, BlockProperties, GrindstoneLikeProperties, HorizontalFacing,
    };

    #[test]
    fn grindstone_block_id_parity() {
        assert_eq!(Block::GRINDSTONE.name, "grindstone");
    }

    #[test]
    fn grindstone_default_state_parity() {
        assert_ne!(
            Block::GRINDSTONE.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn grindstone_properties_roundtrip_parity() {
        for facing in [
            HorizontalFacing::North,
            HorizontalFacing::South,
            HorizontalFacing::East,
            HorizontalFacing::West,
        ] {
            for face in [AttachFace::Floor, AttachFace::Wall, AttachFace::Ceiling] {
                let props = GrindstoneLikeProperties { face, facing };
                let state_id = props.to_state_id(&Block::GRINDSTONE);
                let rt = GrindstoneLikeProperties::from_state_id(state_id, &Block::GRINDSTONE);
                assert_eq!(rt.facing, facing);
                assert_eq!(rt.face, face);
            }
        }
    }
}
