use pumpkin_data::BlockStateId;
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;

use crate::block::BlockBehaviour;
use crate::block::BlockFuture;
use crate::block::CanPlaceAtArgs;
use crate::block::OnNeighborUpdateArgs;
use crate::block::OnPlaceArgs;
use crate::block::PlacedArgs;
use crate::entity::EntityBase;

use super::RailProperties;
use super::common::{
    can_place_rail_at, compute_placed_rail_shape, rail_placement_is_valid,
    update_flanking_rails_shape,
};

#[pumpkin_block("minecraft:detector_rail")]
pub struct DetectorRailBlock;

impl BlockBehaviour for DetectorRailBlock {
    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let mut rail_props = RailProperties::default(args.block);
            let player_facing = args.player.get_entity().get_horizontal_facing();

            rail_props.set_waterlogged(args.replacing.water_source());
            rail_props.set_straight_shape(
                compute_placed_rail_shape(args.world, args.position, player_facing).await,
            );

            rail_props.to_state_id(args.block)
        })
    }

    fn placed<'a>(&'a self, args: PlacedArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            update_flanking_rails_shape(args.world, args.block, args.state_id, args.position).await;
        })
    }

    fn on_neighbor_update<'a>(&'a self, args: OnNeighborUpdateArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if !rail_placement_is_valid(args.world, args.block, args.position).await {
                args.world
                    .break_block(args.position, None, BlockFlags::NOTIFY_ALL)
                    .await;
            }
        })
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_rail_at(args.block_accessor, args.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;
    use pumpkin_data::block_properties::{
        BlockProperties, PoweredRailLikeProperties, RailShapeStraight,
    };

    #[test]
    fn detector_rail_block_id_parity() {
        assert_eq!(Block::DETECTOR_RAIL.name, "detector_rail");
    }

    #[test]
    fn detector_rail_default_state_parity() {
        assert_ne!(
            Block::DETECTOR_RAIL.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn detector_rail_properties_roundtrip_parity() {
        for shape in [
            RailShapeStraight::NorthSouth,
            RailShapeStraight::EastWest,
            RailShapeStraight::AscendingEast,
            RailShapeStraight::AscendingWest,
            RailShapeStraight::AscendingNorth,
            RailShapeStraight::AscendingSouth,
        ] {
            for powered in [true, false] {
                for waterlogged in [true, false] {
                    let props = PoweredRailLikeProperties {
                        powered,
                        shape,
                        waterlogged,
                    };
                    let state_id = props.to_state_id(&Block::DETECTOR_RAIL);
                    let rt =
                        PoweredRailLikeProperties::from_state_id(state_id, &Block::DETECTOR_RAIL);
                    assert_eq!(rt.shape, shape);
                    assert_eq!(rt.powered, powered);
                    assert_eq!(rt.waterlogged, waterlogged);
                }
            }
        }
    }
}
