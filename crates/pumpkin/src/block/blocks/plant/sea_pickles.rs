use crate::block::blocks::plant::PlantBlockBase;
use crate::block::{
    BlockBehaviour, BonemealArgs, CanPlaceAtArgs, CanUpdateAtArgs, GetStateForNeighborUpdateArgs,
    OnPlaceArgs,
};
use crate::block::{BlockFuture, BlockIsReplacing};
use crate::entity::EntityBase;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::BlockProperties;
use pumpkin_data::entity::EntityPose;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockDirection, tag};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;
use rand::RngExt;

type SeaPickleProperties = pumpkin_data::block_properties::SeaPickleLikeProperties;

#[pumpkin_block("minecraft:sea_pickle")]
pub struct SeaPickleBlock;

impl BlockBehaviour for SeaPickleBlock {
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        SeaPickleProperties::from_state_id(args.state_id, args.block).waterlogged
            && args
                .world
                .get_block(&args.position.down())
                .has_tag(&tag::Block::MINECRAFT_CORAL_BLOCKS)
    }

    fn is_bonemeal_success(&self, _args: BonemealArgs<'_>) -> bool {
        true
    }

    fn perform_bonemeal<'a>(&'a self, args: BonemealArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let placements = {
                let mut rng = rand::rng();
                let mut placements = Vec::new();
                for position in spread_positions(args.position) {
                    // Vanilla constructs a fresh BlockPos and compares it by reference to the
                    // origin, so the origin still consumes a roll before failing the water check.
                    if rng.random_range(0..6) != 0
                        || args.world.get_block(&position) != &Block::WATER
                        || !args
                            .world
                            .get_block(&position.down())
                            .has_tag(&tag::Block::MINECRAFT_CORAL_BLOCKS)
                    {
                        continue;
                    }
                    let mut sea_pickle_prop = SeaPickleProperties::default(args.block);
                    sea_pickle_prop.pickles = rng.random_range(1..=4);
                    placements.push((position, sea_pickle_prop.to_state_id(args.block)));
                }
                placements
            };

            for (position, state_id) in placements {
                args.world
                    .set_block_state(&position, state_id, BlockFlags::NOTIFY_ALL)
                    .await;
            }
            let mut sea_pickle_prop = SeaPickleProperties::from_state_id(args.state_id, args.block);
            sea_pickle_prop.pickles = 4;
            args.world
                .set_block_state(
                    args.position,
                    sea_pickle_prop.to_state_id(args.block),
                    BlockFlags::NOTIFY_LISTENERS,
                )
                .await;
        })
    }

    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if args.player.get_entity().pose.load() != EntityPose::Crouching
                && let BlockIsReplacing::Itself(state_id) = args.replacing
            {
                let mut sea_pickle_prop = SeaPickleProperties::from_state_id(state_id, args.block);
                if sea_pickle_prop.pickles < 4 {
                    sea_pickle_prop.pickles += 1;
                }
                return sea_pickle_prop.to_state_id(args.block);
            }

            let mut sea_pickle_prop = SeaPickleProperties::default(args.block);
            sea_pickle_prop.waterlogged = args.replacing.water_source();
            sea_pickle_prop.to_state_id(args.block)
        })
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let support_block = args.block_accessor.get_block_state(&args.position.down());
        can_support_sea_pickle(support_block)
    }

    fn can_update_at(&self, args: CanUpdateAtArgs<'_>) -> bool {
        args.player.get_entity().pose.load() != EntityPose::Crouching
            && SeaPickleProperties::from_state_id(args.state_id, args.block).pickles < 4
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let state = <Self as PlantBlockBase>::get_state_for_neighbor_update(
                self,
                args.world,
                args.position,
                args.state_id,
            )
            .await;
            if state != Block::AIR.default_state.id
                && SeaPickleProperties::from_state_id(state, args.block).waterlogged
            {
                args.world.schedule_fluid_tick(
                    &Fluid::WATER,
                    *args.position,
                    Fluid::WATER.flow_speed as u8,
                    TickPriority::Normal,
                );
            }
            state
        })
    }
}

impl PlantBlockBase for SeaPickleBlock {}

fn spread_positions(origin: &BlockPos) -> Vec<BlockPos> {
    let mut positions = Vec::with_capacity(26);
    let mut width = 1;
    let mut z_offset = 0;

    for x_offset in 0..5 {
        for z_index in 0..width {
            for y in (origin.0.y - 1)..=origin.0.y {
                positions.push(BlockPos::new(
                    origin.0.x - 2 + x_offset,
                    y,
                    origin.0.z - z_offset + z_index,
                ));
            }
        }

        if x_offset < 2 {
            width += 2;
            z_offset += 1;
        } else {
            width -= 2;
            z_offset -= 1;
        }
    }
    positions
}

fn can_support_sea_pickle(state: &pumpkin_data::BlockState) -> bool {
    state.is_side_solid(BlockDirection::Up)
        || state.get_block_collision_shapes().any(|shape| {
            shape.max.y >= 1.0 && shape.max.x > shape.min.x && shape.max.z > shape.min.z
        })
}

#[cfg(test)]
mod tests {
    use super::{can_support_sea_pickle, spread_positions};
    use pumpkin_data::Block;
    use pumpkin_data::block_properties::BlockProperties;

    type SeaPickleProperties = pumpkin_data::block_properties::SeaPickleLikeProperties;

    #[test]
    fn sea_pickle_block_id_parity() {
        assert_eq!(Block::SEA_PICKLE.name, "sea_pickle");
    }

    #[test]
    fn sea_pickle_properties_encoding_decoding_parity() {
        // Sea pickle has 8 states: pickles 1..=4 × waterlogged true/false
        let mut count = 0;
        for pickles in 1..=4u8 {
            for waterlogged in [true, false] {
                let props = SeaPickleProperties {
                    pickles,
                    waterlogged,
                };
                let state_id = props.to_state_id(&Block::SEA_PICKLE);
                let roundtrip = SeaPickleProperties::from_state_id(state_id, &Block::SEA_PICKLE);
                assert_eq!(
                    roundtrip.pickles, pickles,
                    "Pickles roundtrip failed for pickles={pickles}, waterlogged={waterlogged}"
                );
                assert_eq!(
                    roundtrip.waterlogged, waterlogged,
                    "Waterlogged roundtrip failed for pickles={pickles}, waterlogged={waterlogged}"
                );
                count += 1;
            }
        }
        assert_eq!(count, 8, "Expected 8 sea pickle states");
    }

    #[test]
    fn sea_pickle_default_state_parity() {
        // Vanilla default: pickles=1, waterlogged=true
        let default_props = SeaPickleProperties::from_state_id(
            Block::SEA_PICKLE.default_state.id,
            &Block::SEA_PICKLE,
        );
        assert_eq!(default_props.pickles, 1, "Default pickles should be 1");
        assert!(
            default_props.waterlogged,
            "Default waterlogged should be true"
        );
    }

    #[test]
    fn sea_pickle_support_requires_center_solid_up() {
        assert!(can_support_sea_pickle(&Block::STONE.default_state));
        assert!(can_support_sea_pickle(&Block::DIRT.default_state));
        assert!(can_support_sea_pickle(&Block::OAK_FENCE.default_state));
        assert!(!can_support_sea_pickle(&Block::AIR.default_state));
    }

    #[test]
    fn bonemeal_spread_uses_vanilla_diamond_and_two_layers() {
        let origin = pumpkin_util::math::position::BlockPos::new(10, 64, 20);
        let positions = spread_positions(&origin);
        assert_eq!(positions.len(), 26);
        assert_eq!(positions.iter().filter(|pos| pos.0.x == 8).count(), 2);
        assert_eq!(positions.iter().filter(|pos| pos.0.x == 9).count(), 6);
        assert_eq!(positions.iter().filter(|pos| pos.0.x == 10).count(), 10);
        assert_eq!(positions.iter().filter(|pos| pos.0.x == 11).count(), 6);
        assert_eq!(positions.iter().filter(|pos| pos.0.x == 12).count(), 2);
        assert!(positions.iter().all(|pos| pos.0.y == 63 || pos.0.y == 64));
    }
}
