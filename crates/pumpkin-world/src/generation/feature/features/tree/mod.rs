use decorator::TreeDecorator;
use foliage::FoliagePlacer;
use pumpkin_data::BlockState;
use pumpkin_data::block_properties::{BlockProperties, OakLeavesLikeProperties};
use pumpkin_data::tag::Taggable;
use pumpkin_data::{BlockId, tag};
use pumpkin_util::{math::position::BlockPos, random::RandomGenerator};
use root::RootPlacer;

use trunk::TrunkPlacer;

use crate::generation::proto_chunk::GenerationCache;
use crate::generation::{block_state_provider::BlockStateProvider, feature::size::FeatureSize};
use crate::world::WorldPortalExt;

pub mod decorator;
pub mod foliage;
pub mod root;
pub mod trunk;

pub struct TreeFeature {
    pub trunk_provider: BlockStateProvider,
    pub trunk_placer: TrunkPlacer,
    pub foliage_provider: BlockStateProvider,
    pub foliage_placer: FoliagePlacer,
    pub minimum_size: FeatureSize,
    pub ignore_vines: bool,
    pub decorators: Vec<TreeDecorator>,
    pub below_trunk_provider: BlockStateProvider,
    pub root_placer: Option<RootPlacer>,
}

pub struct TreeNode {
    center: BlockPos,
    foliage_radius: i32,
    giant_trunk: bool,
}

impl TreeFeature {
    #[expect(clippy::too_many_arguments)]
    pub fn generate<T: GenerationCache>(
        &self,
        block_registry: &dyn WorldPortalExt,
        chunk: &mut T,
        min_y: i8,
        height: u16,
        feature_name: pumpkin_data::placed_feature::PlacedFeature, // This placed feature
        random: &mut RandomGenerator,
        pos: BlockPos,
    ) -> bool {
        let (log_positions, root_positions, foliage_positions) = self.generate_main(
            block_registry,
            chunk,
            min_y,
            height,
            feature_name,
            random,
            pos,
        );

        if log_positions.is_empty() && foliage_positions.is_empty() {
            return false;
        }

        for decorator in &self.decorators {
            decorator.generate(
                chunk,
                block_registry,
                random,
                &root_positions,
                &log_positions,
                &foliage_positions,
            );
        }

        Self::update_leaves(chunk, &log_positions, &root_positions, &foliage_positions);
        true
    }

    pub fn update_leaves<T: GenerationCache>(
        chunk: &mut T,
        logs: &[BlockPos],
        roots: &[BlockPos],
        foliage: &[BlockPos],
    ) {
        if logs.is_empty() && foliage.is_empty() {
            return;
        }

        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];
        for pos in logs.iter().chain(roots).chain(foliage) {
            min[0] = min[0].min(pos.0.x);
            min[1] = min[1].min(pos.0.y);
            min[2] = min[2].min(pos.0.z);
            max[0] = max[0].max(pos.0.x);
            max[1] = max[1].max(pos.0.y);
            max[2] = max[2].max(pos.0.z);
        }

        let spans = [
            (max[0] - min[0] + 1) as usize,
            (max[1] - min[1] + 1) as usize,
            (max[2] - min[2] + 1) as usize,
        ];
        let mut visited = vec![false; spans[0] * spans[1] * spans[2]];
        let index = |pos: BlockPos| {
            if pos.0.x < min[0]
                || pos.0.x > max[0]
                || pos.0.y < min[1]
                || pos.0.y > max[1]
                || pos.0.z < min[2]
                || pos.0.z > max[2]
            {
                None
            } else {
                Some(
                    (((pos.0.x - min[0]) as usize * spans[1] + (pos.0.y - min[1]) as usize)
                        * spans[2])
                        + (pos.0.z - min[2]) as usize,
                )
            }
        };

        for pos in roots {
            if let Some(i) = index(*pos) {
                visited[i] = true;
            }
        }

        let mut frontier: [std::collections::HashSet<BlockPos>; 7] = Default::default();
        frontier[0].extend(logs.iter().copied());
        for distance in 0..7 {
            while let Some(pos) = frontier[distance].iter().next().copied() {
                frontier[distance].remove(&pos);
                let Some(i) = index(pos) else { continue };
                if visited[i] {
                    continue;
                }
                visited[i] = true;

                if distance != 0 {
                    let (block, state) = chunk.get_block_and_state(&pos);
                    if OakLeavesLikeProperties::handles_block_id(block.id) {
                        let mut props = OakLeavesLikeProperties::from_state_id(state.id, block);
                        props.distance = distance as u8;
                        chunk.set_block_state(&pos.0, &block.states[props.to_index() as usize]);
                    }
                }

                if distance == 6 {
                    continue;
                }
                for direction in pumpkin_data::BlockDirection::all() {
                    let neighbor = pos.offset(direction.to_offset());
                    let Some(neighbor_index) = index(neighbor) else {
                        continue;
                    };
                    if visited[neighbor_index] {
                        continue;
                    }
                    let (block, _) = chunk.get_block_and_state(&neighbor);
                    if block.has_tag(&tag::Block::MINECRAFT_LEAVES) {
                        frontier[distance + 1].insert(neighbor);
                    }
                }
            }
        }
    }

    pub fn can_replace_or_log(state: &BlockState, id: BlockId) -> bool {
        Self::can_replace(state, id) || id.has_tag(tag::Block::MINECRAFT_LOGS)
    }

    pub fn is_air_or_leaves(state: &BlockState, id: BlockId) -> bool {
        state.is_air() || id.has_tag(tag::Block::MINECRAFT_LEAVES)
    }

    pub fn can_replace(state: &BlockState, id: BlockId) -> bool {
        state.is_air() || id.has_tag(tag::Block::MINECRAFT_REPLACEABLE_BY_TREES)
    }

    #[expect(clippy::too_many_arguments)]
    fn generate_main<T: GenerationCache>(
        &self,
        block_registry: &dyn WorldPortalExt,
        chunk: &mut T,
        _min_y: i8,
        _height: u16,
        _feature_name: pumpkin_data::placed_feature::PlacedFeature, // This placed feature
        random: &mut RandomGenerator,
        pos: BlockPos,
    ) -> (Vec<BlockPos>, Vec<BlockPos>, Vec<BlockPos>) {
        let height = self.trunk_placer.get_height(random);

        let trunk_start = self
            .root_placer
            .as_ref()
            .map_or(pos, |placer| placer.trunk_offset(pos, random));

        let clipped_height = self.minimum_size.min_clipped_height;
        let top = self.get_top(height, chunk, trunk_start);
        if top < height && top < clipped_height.map_or(u32::MAX, |h| h as u32) {
            return (vec![], vec![], vec![]);
        }

        let root_positions = if let Some(placer) = &self.root_placer {
            match placer.generate(chunk, block_registry, random, pos, trunk_start) {
                Some(positions) => positions,
                None => return (vec![], vec![], vec![]),
            }
        } else {
            Vec::new()
        };

        let trunk_state = self.trunk_provider.get(random, pos, chunk, block_registry);

        let (nodes, logs) = self.trunk_placer.generate(
            block_registry,
            top,
            trunk_start,
            chunk,
            random,
            &self.below_trunk_provider,
            trunk_state,
        );

        let foliage_height = self
            .foliage_placer
            .r#type
            .get_random_height(random, height as i32);
        let base_height = height as i32 - foliage_height;
        let foliage_radius = self.foliage_placer.get_random_radius(random, base_height);
        let foliage_state = self
            .foliage_provider
            .get(random, pos, chunk, block_registry);
        let mut foliage_positions = Vec::new();
        for node in nodes {
            foliage_positions.extend(self.foliage_placer.generate(
                chunk,
                random,
                &node,
                foliage_height,
                foliage_radius,
                foliage_state,
            ));
        }
        (logs, root_positions, foliage_positions)
    }

    fn get_top<T: GenerationCache>(&self, height: u32, chunk: &T, init_pos: BlockPos) -> u32 {
        for y in 0..=height + 1 {
            let j = self.minimum_size.r#type.get_radius(height, y as i32);
            for x in -j..=j {
                for z in -j..=j {
                    let pos = BlockPos(init_pos.0.add_raw(x, y as i32, z));
                    let rstate = GenerationCache::get_block_state(chunk, &pos.0);
                    let block = rstate.to_block_id();
                    if Self::can_replace_or_log(rstate.to_state(), block)
                        && (self.ignore_vines || block != BlockId::VINE)
                    {
                        continue;
                    }
                    return y.saturating_sub(2);
                }
            }
        }
        height
    }
}
