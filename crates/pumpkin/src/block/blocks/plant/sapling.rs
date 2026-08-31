use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockStateId, block_properties::BlockProperties};
use pumpkin_data::{
    configured_feature::ConfiguredFeature as FeatureKey, placed_feature::PlacedFeature,
};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::{
    math::{position::BlockPos, vector2::Vector2},
    random::{RandomGenerator, xoroshiro128::Xoroshiro},
};
use pumpkin_world::{
    chunk_system::{chunk_state::Chunk, generation_cache::Cache},
    generation::{
        feature::configured_features::{CONFIGURED_FEATURES, ConfiguredFeature},
        proto_chunk::{GenerationCache, ProtoChunk},
    },
    world::BlockFlags,
};
use rand::RngExt;
use std::sync::Arc;

use crate::{
    block::{
        BlockBehaviour, BlockFuture, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
        RandomTickArgs, blocks::plant::PlantBlockBase,
    },
    world::{World, WorldPortal},
};

type SaplingProperties = pumpkin_data::block_properties::OakSaplingLikeProperties;

#[pumpkin_block_from_tag("minecraft:saplings")]
pub struct SaplingBlock;

impl SaplingBlock {
    fn two_by_two_origin(world: &World, pos: BlockPos, block: &Block) -> Option<BlockPos> {
        for (dx, dz) in [(0, 0), (-1, 0), (0, -1), (-1, -1)] {
            let origin = BlockPos::new(pos.0.x + dx, pos.0.y, pos.0.z + dz);
            if [(0, 0), (1, 0), (0, 1), (1, 1)].iter().all(|(x, z)| {
                world.get_block(&BlockPos::new(origin.0.x + x, origin.0.y, origin.0.z + z)) == block
            }) {
                return Some(origin);
            }
        }
        None
    }

    fn feature(world: &World, pos: BlockPos, block: &Block) -> Option<(FeatureKey, BlockPos)> {
        let square = Self::two_by_two_origin(world, pos, block);
        let mut rng = rand::rng();
        let mut feature = match block.name {
            "oak_sapling" if rng.random_ratio(1, 10) => FeatureKey::FancyOak,
            "oak_sapling" => FeatureKey::Oak,
            "spruce_sapling" if square.is_some() && rng.random_bool(0.5) => FeatureKey::MegaSpruce,
            "spruce_sapling" if square.is_some() => FeatureKey::MegaPine,
            "spruce_sapling" => FeatureKey::Spruce,
            "birch_sapling" => FeatureKey::Birch,
            "jungle_sapling" if square.is_some() => FeatureKey::MegaJungleTree,
            "jungle_sapling" => FeatureKey::JungleTree,
            "acacia_sapling" => FeatureKey::Acacia,
            "dark_oak_sapling" if square.is_some() => FeatureKey::DarkOak,
            "dark_oak_sapling" => return None,
            "pale_oak_sapling" if square.is_some() => FeatureKey::PaleOak,
            "pale_oak_sapling" => return None,
            "mangrove_propagule" if rng.random_bool(0.85) => FeatureKey::TallMangrove,
            "mangrove_propagule" => FeatureKey::Mangrove,
            "cherry_sapling" => FeatureKey::Cherry,
            "azalea" | "flowering_azalea" => FeatureKey::AzaleaTree,
            _ => return None,
        };
        let flowers_nearby = (-2..=2).any(|x| {
            (-1..=1).any(|y| {
                (-2..=2).any(|z| {
                    world
                        .get_block(&BlockPos::new(pos.0.x + x, pos.0.y + y, pos.0.z + z))
                        .is_tagged_with("minecraft:flowers")
                        .unwrap_or(false)
                })
            })
        });
        if flowers_nearby {
            feature = match feature {
                FeatureKey::Oak => FeatureKey::OakBees005,
                FeatureKey::FancyOak => FeatureKey::FancyOakBees005,
                FeatureKey::Birch => FeatureKey::BirchBees005,
                FeatureKey::Cherry => FeatureKey::CherryBees005,
                other => other,
            };
        }
        Some((feature, square.unwrap_or(pos)))
    }

    fn tree_type(block: &Block) -> crate::plugin::api::events::world::structure_grow::TreeType {
        use crate::plugin::api::events::world::structure_grow::TreeType;
        match block.name {
            "spruce_sapling" => TreeType::Spruce,
            "birch_sapling" => TreeType::Birch,
            "jungle_sapling" => TreeType::Jungle,
            "acacia_sapling" => TreeType::Acacia,
            "dark_oak_sapling" | "pale_oak_sapling" => TreeType::DarkOak,
            "mangrove_propagule" => TreeType::Mangrove,
            "cherry_sapling" => TreeType::Cherry,
            "azalea" | "flowering_azalea" => TreeType::Azalea,
            _ => TreeType::Oak,
        }
    }

    pub(crate) async fn grow_tree(
        &self,
        world: &Arc<World>,
        pos: BlockPos,
        block: &Block,
        bone_meal: bool,
    ) {
        use crate::plugin::api::events::world::structure_grow::StructureGrowEvent;
        let Some((key, origin)) = Self::feature(world, pos, block) else {
            return;
        };
        let Some(ConfiguredFeature::Tree(tree)) = CONFIGURED_FEATURES.get(&key) else {
            return;
        };

        // Generate into cloned chunks first: obstructed trees cannot partly mutate the live world.
        let center = origin.chunk_position();
        let (first_x, first_z) = (center.x - 1, center.y - 1);
        let generator = world.level.world_gen.load();
        let mut cache = Cache::new(first_x, first_z, 3);
        let mut originals = Vec::with_capacity(9);
        for chunk_x in first_x..=first_x + 2 {
            for chunk_z in first_z..=first_z + 2 {
                let chunk_pos = Vector2::new(chunk_x, chunk_z);
                let Some(chunk) = world.level.read_chunk_sync(&chunk_pos, Arc::clone) else {
                    return;
                };
                cache
                    .chunks
                    .push(Chunk::Proto(Box::new(ProtoChunk::from_chunk_data(
                        &chunk, &generator,
                    ))));
                originals.push(chunk);
            }
        }
        let mut random = RandomGenerator::Xoroshiro(Xoroshiro::from_seed(rand::rng().random()));
        if !tree.generate(
            &WorldPortal(world.clone()),
            &mut cache,
            world.min_y as i8,
            world.dimension.height as u16,
            PlacedFeature::Oak,
            &mut random,
            origin,
        ) {
            return;
        }

        let mut changes = Vec::new();
        for chunk_x in first_x..=first_x + 2 {
            for chunk_z in first_z..=first_z + 2 {
                let index = ((chunk_x - first_x) * 3 + chunk_z - first_z) as usize;
                let generated = cache.get_chunk(chunk_x, chunk_z).expect("snapshot chunk");
                let original = &originals[index];
                for x in 0..16 {
                    for z in 0..16 {
                        for y in world.min_y..world.min_y + world.dimension.height {
                            let position = BlockPos::new((chunk_x << 4) + x, y, (chunk_z << 4) + z);
                            let old = original
                                .section
                                .get_block_absolute_y(x as usize, y, z as usize)
                                .unwrap_or(BlockStateId::AIR);
                            let new = generated.get_block_state(&position.0);
                            if old != new {
                                changes.push((position, old, new));
                            }
                        }
                    }
                }
            }
        }
        if changes.is_empty()
            || !changes.iter().any(|(_, _, state)| {
                state
                    .to_block_id()
                    .has_tag(pumpkin_data::tag::Block::MINECRAFT_LOGS)
            })
        {
            return;
        }

        let mut block_entities = Vec::new();
        for chunk_x in first_x..=first_x + 2 {
            for chunk_z in first_z..=first_z + 2 {
                if let Some(chunk) = cache.get_chunk_mut(chunk_x, chunk_z) {
                    block_entities.extend(chunk.take_pending_block_entities());
                }
            }
        }

        let mut event = StructureGrowEvent::new(origin, Self::tree_type(block), bone_meal);
        if let Some(server) = world.server.upgrade() {
            server.plugin_manager.fire(&server, &mut event).await;
        }
        if event.cancelled
            || changes
                .iter()
                .any(|(p, old, _)| world.get_block_state_id(p) != *old)
        {
            return;
        }
        for (position, _, state) in changes {
            world
                .set_block_state(&position, state, BlockFlags::NOTIFY_ALL)
                .await;
        }
        for nbt in block_entities {
            if let Some(block_entity) = crate::block::entities::block_entity_from_nbt(&nbt) {
                world.add_block_entity(block_entity);
            }
        }
    }

    async fn advance(&self, world: &Arc<World>, pos: &BlockPos, bone_meal: bool) {
        let (block, state) = world.get_block_and_state_id(pos);
        let mut props = SaplingProperties::from_state_id(state, block);
        if props.stage == 0 {
            props.stage = 1;
            world
                .set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_ALL)
                .await;
        } else {
            self.grow_tree(world, *pos, block, bone_meal).await;
        }
    }
}

impl BlockBehaviour for SaplingBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
    }
    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            <Self as PlantBlockBase>::get_state_for_neighbor_update(
                self,
                args.world,
                args.position,
                args.state_id,
            )
            .await
        })
    }
    fn is_valid_bonemeal_target(&self, _args: BonemealArgs<'_>) -> bool {
        true
    }
    fn is_bonemeal_success(&self, _args: BonemealArgs<'_>) -> bool {
        rand::rng().random_bool(0.45)
    }
    fn perform_bonemeal<'a>(&'a self, args: BonemealArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move { self.advance(args.world, args.position, true).await })
    }
    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if args.world.get_max_local_raw_brightness(&args.position.up()) >= 9
                && rand::rng().random_ratio(1, 7)
            {
                self.advance(args.world, args.position, false).await;
            }
        })
    }
}

impl PlantBlockBase for SaplingBlock {}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::tag;

    #[test]
    fn saplings_block_id_and_default_state_parity() {
        let saplings = [
            &Block::OAK_SAPLING,
            &Block::SPRUCE_SAPLING,
            &Block::BIRCH_SAPLING,
            &Block::JUNGLE_SAPLING,
            &Block::ACACIA_SAPLING,
            &Block::DARK_OAK_SAPLING,
            &Block::CHERRY_SAPLING,
            &Block::PALE_OAK_SAPLING,
        ];

        for sapling in saplings {
            assert!(sapling.has_tag(&tag::Block::MINECRAFT_SAPLINGS));
            let default_props = SaplingProperties::from_state_id(sapling.default_state.id, sapling);
            assert_eq!(default_props.stage, 0);
        }
        assert!(Block::MANGROVE_PROPAGULE.has_tag(&tag::Block::MINECRAFT_SAPLINGS));
    }

    #[test]
    fn sapling_stage_properties_encoding_decoding_parity() {
        for stage in 0..=1 {
            let props = SaplingProperties { stage };
            let state_id = props.to_state_id(&Block::OAK_SAPLING);
            let decoded = SaplingProperties::from_state_id(state_id, &Block::OAK_SAPLING);
            assert_eq!(decoded.stage, stage);
        }
    }

    #[test]
    fn sapling_supports_tag_parity() {
        assert!(Block::DIRT.has_tag(&tag::Block::MINECRAFT_SUPPORTS_VEGETATION));
        assert!(Block::GRASS_BLOCK.has_tag(&tag::Block::MINECRAFT_SUPPORTS_VEGETATION));
        assert!(Block::PODZOL.has_tag(&tag::Block::MINECRAFT_SUPPORTS_VEGETATION));
        assert!(Block::COARSE_DIRT.has_tag(&tag::Block::MINECRAFT_SUPPORTS_VEGETATION));
        assert!(Block::MOSS_BLOCK.has_tag(&tag::Block::MINECRAFT_SUPPORTS_VEGETATION));
    }
}
