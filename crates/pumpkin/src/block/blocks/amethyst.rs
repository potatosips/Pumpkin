use crate::block::BlockBehaviour;
use crate::block::BlockFuture;
use crate::block::BlockMetadata;
use crate::block::CanPlaceAtArgs;
use crate::block::GetStateForNeighborUpdateArgs;
use crate::block::OnNeighborUpdateArgs;
use crate::block::OnPlaceArgs;
use crate::block::RandomTickArgs;
use crate::block::blocks::abstract_wall_mounting::WallMountedBlock;
use pumpkin_data::Block;
use pumpkin_data::BlockDirection;
use pumpkin_data::BlockId;
use pumpkin_data::BlockStateId;
use pumpkin_data::FacingExt;
use pumpkin_data::block_properties::AmethystClusterLikeProperties;
use pumpkin_data::block_properties::BlockProperties;
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;

pub struct AmethystBlock;

impl BlockMetadata for AmethystBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::SMALL_AMETHYST_BUD,
            BlockId::MEDIUM_AMETHYST_BUD,
            BlockId::LARGE_AMETHYST_BUD,
            BlockId::AMETHYST_CLUSTER,
        ]
        .into()
    }
}

impl BlockBehaviour for AmethystBlock {
    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let mut props = AmethystClusterLikeProperties::from_state_id(
                args.block.default_state.id,
                args.block,
            );
            props.facing = args.direction.to_facing();
            props.waterlogged = args.replacing.water_source();
            props.to_state_id(args.block)
        })
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let attachment_dir = match args.direction {
            Some(dir) => dir.opposite(),
            None => {
                let props = AmethystClusterLikeProperties::from_state_id(args.state.id, args.block);
                props.facing.to_block_direction().opposite()
            }
        };

        WallMountedBlock::can_place_at(self, args.block_accessor, args.position, attachment_dir)
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let props = AmethystClusterLikeProperties::from_state_id(args.state_id, args.block);
            let attachment_dir = props.facing.to_block_direction().opposite();
            if args.direction == attachment_dir {
                if !WallMountedBlock::can_place_at(self, args.world, args.position, attachment_dir)
                {
                    if props.waterlogged {
                        return Block::WATER.default_state.id;
                    }
                    return Block::AIR.default_state.id;
                }
            }
            args.state_id
        })
    }

    fn on_neighbor_update<'a>(&'a self, args: OnNeighborUpdateArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let state = args.world.get_block_state(args.position);
            let props = AmethystClusterLikeProperties::from_state_id(state.id, args.block);
            let attachment_dir = props.facing.to_block_direction().opposite();
            if !WallMountedBlock::can_place_at(
                self,
                args.world.as_ref(),
                args.position,
                attachment_dir,
            ) {
                let replacement = if props.waterlogged {
                    Block::WATER.default_state.id
                } else {
                    Block::AIR.default_state.id
                };
                args.world
                    .set_block_state(args.position, replacement, BlockFlags::NOTIFY_ALL)
                    .await;
            }
        })
    }
}

impl WallMountedBlock for AmethystBlock {
    fn get_direction(&self, state_id: BlockStateId, block: &Block) -> BlockDirection {
        let props = AmethystClusterLikeProperties::from_state_id(state_id, block);
        props.facing.to_block_direction()
    }
}

#[pumpkin_block("minecraft:budding_amethyst")]
pub struct BuddingAmethystBlock;

impl BlockBehaviour for BuddingAmethystBlock {
    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if rand::random_range(0..5) != 0 {
                return;
            }

            let directions = [
                BlockDirection::Up,
                BlockDirection::Down,
                BlockDirection::North,
                BlockDirection::South,
                BlockDirection::East,
                BlockDirection::West,
            ];
            let dir = directions[rand::random_range(0..6)];
            let target_pos = args.position.offset(dir.to_offset());
            let (target_block, target_state) = args.world.get_block_and_state(&target_pos);

            let next_block = if target_state.is_air() {
                Some((&Block::SMALL_AMETHYST_BUD, false))
            } else if target_block.id == BlockId::WATER
                && target_state.id == Block::WATER.default_state.id
            {
                Some((&Block::SMALL_AMETHYST_BUD, true))
            } else if target_block.id == BlockId::SMALL_AMETHYST_BUD {
                let props =
                    AmethystClusterLikeProperties::from_state_id(target_state.id, target_block);
                if props.facing == dir.to_facing() {
                    Some((&Block::MEDIUM_AMETHYST_BUD, props.waterlogged))
                } else {
                    None
                }
            } else if target_block.id == BlockId::MEDIUM_AMETHYST_BUD {
                let props =
                    AmethystClusterLikeProperties::from_state_id(target_state.id, target_block);
                if props.facing == dir.to_facing() {
                    Some((&Block::LARGE_AMETHYST_BUD, props.waterlogged))
                } else {
                    None
                }
            } else if target_block.id == BlockId::LARGE_AMETHYST_BUD {
                let props =
                    AmethystClusterLikeProperties::from_state_id(target_state.id, target_block);
                if props.facing == dir.to_facing() {
                    Some((&Block::AMETHYST_CLUSTER, props.waterlogged))
                } else {
                    None
                }
            } else {
                None
            };

            if let Some((block_to_place, is_waterlogged)) = next_block {
                let mut props = AmethystClusterLikeProperties::default(block_to_place);
                props.facing = dir.to_facing();
                props.waterlogged = is_waterlogged;
                args.world
                    .set_block_state(
                        &target_pos,
                        props.to_state_id(block_to_place),
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::block_properties::Facing;

    #[test]
    fn amethyst_block_ids_parity() {
        assert_eq!(Block::SMALL_AMETHYST_BUD.name, "small_amethyst_bud");
        assert_eq!(Block::MEDIUM_AMETHYST_BUD.name, "medium_amethyst_bud");
        assert_eq!(Block::LARGE_AMETHYST_BUD.name, "large_amethyst_bud");
        assert_eq!(Block::AMETHYST_CLUSTER.name, "amethyst_cluster");
        assert_eq!(Block::BUDDING_AMETHYST.name, "budding_amethyst");
    }

    #[test]
    fn amethyst_cluster_properties_encoding_decoding_parity() {
        for block in [
            &Block::SMALL_AMETHYST_BUD,
            &Block::MEDIUM_AMETHYST_BUD,
            &Block::LARGE_AMETHYST_BUD,
            &Block::AMETHYST_CLUSTER,
        ] {
            assert_eq!(block.states.len(), 12);
            for facing in [
                Facing::North,
                Facing::South,
                Facing::East,
                Facing::West,
                Facing::Up,
                Facing::Down,
            ] {
                for waterlogged in [false, true] {
                    let props = AmethystClusterLikeProperties {
                        facing,
                        waterlogged,
                    };
                    let state_id = props.to_state_id(block);
                    let decoded = AmethystClusterLikeProperties::from_state_id(state_id, block);
                    assert_eq!(decoded.facing, facing);
                    assert_eq!(decoded.waterlogged, waterlogged);
                }
            }
        }
    }

    #[test]
    fn amethyst_default_state_parity() {
        let default_props = AmethystClusterLikeProperties::from_state_id(
            Block::AMETHYST_CLUSTER.default_state.id,
            &Block::AMETHYST_CLUSTER,
        );
        assert_eq!(default_props.facing, Facing::Up);
        assert_eq!(default_props.waterlogged, false);
    }
}
