use sha2::{Digest, Sha256};
use std::cell::RefCell;

use pumpkin_data::chunk::{Biome, BiomeTree, NETHER_BIOME_SOURCE, OVERWORLD_BIOME_SOURCE};

use crate::generation::noise::router::multi_noise_sampler::MultiNoiseSampler;
pub mod end;
pub mod multi_noise;
pub mod position_finder;

thread_local! {
    /// A shortcut; check if last used biome is what we should use
    static LAST_RESULT_NODE: RefCell<Option<&'static BiomeTree>> = const {RefCell::new(None) };
}

pub trait BiomeSupplier {
    fn biome(&self, x: i32, y: i32, z: i32, noise: &mut MultiNoiseSampler<'_>) -> &'static Biome;
}

pub struct MultiNoiseBiomeSupplier {
    source: &'static BiomeTree,
}

impl MultiNoiseBiomeSupplier {
    pub const OVERWORLD: Self = Self::new(&OVERWORLD_BIOME_SOURCE);
    pub const NETHER: Self = Self::new(&NETHER_BIOME_SOURCE);

    const fn new(source: &'static BiomeTree) -> Self {
        Self { source }
    }
}

impl BiomeSupplier for MultiNoiseBiomeSupplier {
    fn biome(&self, x: i32, y: i32, z: i32, noise: &mut MultiNoiseSampler<'_>) -> &'static Biome {
        let point = noise.sample(x, y, z);
        let point_list = point.convert_to_list();
        LAST_RESULT_NODE.with_borrow_mut(|last_result| self.source.get(&point_list, last_result))
    }
}

#[must_use]
pub fn hash_seed(seed: u64) -> i64 {
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    let result = hasher.finalize();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&result[..8]);
    i64::from_le_bytes(bytes)
}

/// Returns the noise-biome quart cell selected by Java's seeded biome zoom.
/// `seed` is the SHA-256-obfuscated world seed, not the generation seed.
pub fn zoomed_quart_position(seed: i64, position: [i32; 3]) -> [i32; 3] {
    fn mix(state: i64, salt: i64) -> i64 {
        state
            .wrapping_mul(
                state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407),
            )
            .wrapping_add(salt)
    }
    fn jitter(state: i64) -> f64 {
        ((state >> 24).rem_euclid(1024) as f64 / 1024.0 - 0.5) * 0.9
    }
    let shifted = position.map(|v| v.wrapping_sub(2));
    let base = shifted.map(|v| v >> 2);
    let fraction = shifted.map(|v| f64::from(v & 3) / 4.0);
    let mut nearest = base;
    let mut distance = f64::INFINITY;
    for corner in 0..8 {
        let offsets = [(corner >> 2) & 1, (corner >> 1) & 1, corner & 1];
        let cell = std::array::from_fn::<_, 3, _>(|i| base[i] + offsets[i]);
        let mut state = seed;
        for coordinate in cell.into_iter().cycle().take(6) {
            state = mix(state, i64::from(coordinate));
        }
        let dx = fraction[0] - f64::from(offsets[0]) + jitter(state);
        state = mix(state, seed);
        let dy = fraction[1] - f64::from(offsets[1]) + jitter(state);
        state = mix(state, seed);
        let dz = fraction[2] - f64::from(offsets[2]) + jitter(state);
        let candidate = dz * dz + dy * dy + dx * dx;
        if candidate < distance {
            distance = candidate;
            nearest = cell;
        }
    }
    nearest
}

#[cfg(test)]
mod test {
    use pumpkin_data::{chunk::Biome, dimension::Dimension};
    use pumpkin_util::read_data_from_file;
    use serde::Deserialize;

    use crate::{
        ProtoChunk,
        chunk::palette::BIOME_NETWORK_MAX_BITS,
        generation::noise::router::multi_noise_sampler::{
            MultiNoiseSampler, MultiNoiseSamplerBuilderOptions,
        },
    };

    use super::{BiomeSupplier, MultiNoiseBiomeSupplier, hash_seed};

    #[test]
    fn biome_zoom_matches_java_1_21_4() {
        // Captured by invoking BiomeManager.getBiome in the official server jar
        // with a proxy NoiseBiomeSource that records the selected quart cell.
        let positions = [
            [0, 0, 0],
            [-1, -64, -1],
            [15, 63, 16],
            [16, 64, 15],
            [-17, 319, 31],
            [30000000, 70, -30000000],
        ];
        let cases = [
            (
                0,
                [
                    [-1, -1, -1],
                    [0, -16, -1],
                    [3, 15, 3],
                    [4, 15, 3],
                    [-5, 80, 7],
                    [7499999, 17, -7500001],
                ],
            ),
            (
                8794265229978523055,
                [
                    [-1, 0, -1],
                    [-1, -16, -1],
                    [3, 16, 4],
                    [3, 16, 4],
                    [-5, 79, 7],
                    [7500000, 17, -7500001],
                ],
            ),
            (
                -1087248400229165450,
                [
                    [-1, 0, -1],
                    [-1, -16, -1],
                    [3, 15, 4],
                    [4, 16, 3],
                    [-5, 79, 7],
                    [7500000, 17, -7500001],
                ],
            ),
        ];
        for (seed, expected) in cases {
            for (position, expected) in positions.into_iter().zip(expected) {
                assert_eq!(
                    super::zoomed_quart_position(seed, position),
                    expected,
                    "seed={seed} pos={position:?}"
                );
            }
        }
    }

    #[test]
    fn biome_desert() {
        use crate::generation::generator::{GeneratorInit, VanillaGenerator};
        use pumpkin_util::world_seed::Seed;
        let seed = 13579;
        let generator = VanillaGenerator::new(Seed(seed as u64), Dimension::OVERWORLD);
        let multi_noise_config = MultiNoiseSamplerBuilderOptions::new(1, 1, 1);
        let mut sampler =
            MultiNoiseSampler::generate(&generator.base_router.multi_noise, &multi_noise_config);
        let biome = MultiNoiseBiomeSupplier::OVERWORLD.biome(-24, 1, 8, &mut sampler);
        assert_eq!(biome, &Biome::DESERT);
    }

    #[test]
    fn wide_area_surface() {
        use crate::generation::generator::{GeneratorInit, VanillaGenerator, WorldGenerator};
        use crate::generation::noise::router::multi_noise_sampler::{
            MultiNoiseSampler, MultiNoiseSamplerBuilderOptions,
        };
        use crate::generation::{biome_coords, positions::chunk_pos};
        use pumpkin_util::world_seed::Seed;
        #[derive(Deserialize)]
        struct BiomeData {
            x: i32,
            z: i32,
            data: Vec<(i32, i32, i32, u8)>,
        }

        let expected_data: Vec<BiomeData> =
            read_data_from_file!("../../../../assets/tests/biome_no_blend_no_beard_0.json");

        let seed = 0;
        let world_gen = WorldGenerator::Noise(Box::new(VanillaGenerator::new(
            Seed(seed as u64),
            Dimension::OVERWORLD,
        )));
        let WorldGenerator::Noise(generator) = &world_gen else {
            unreachable!()
        };

        for data in expected_data {
            let chunk_x = data.x;
            let chunk_z = data.z;

            let mut chunk = ProtoChunk::new(chunk_x, chunk_z, &world_gen);

            // Create MultiNoiseSampler for populate_biomes

            let start_x = chunk_pos::start_block_x(chunk_x);
            let start_z = chunk_pos::start_block_z(chunk_z);

            let horizontal_biome_end = biome_coords::from_block(16);
            let multi_noise_config = MultiNoiseSamplerBuilderOptions::new(
                biome_coords::from_block(start_x),
                biome_coords::from_block(start_z),
                horizontal_biome_end as usize,
            );
            let mut multi_noise_sampler = MultiNoiseSampler::generate(
                &generator.base_router.multi_noise,
                &multi_noise_config,
            );

            chunk.populate_biomes(generator, &mut multi_noise_sampler);

            for (biome_x, biome_y, biome_z, biome_id) in data.data {
                let calculated_biome = chunk.get_biome(biome_x, biome_y, biome_z);

                assert_eq!(
                    biome_id,
                    calculated_biome.id,
                    "Expected {:?} was {:?} at {},{},{} ({},{})",
                    Biome::from_id(biome_id),
                    calculated_biome,
                    biome_x,
                    biome_y,
                    biome_z,
                    data.x,
                    data.z
                );
            }
        }
    }

    #[test]
    fn hash_seed_test() {
        let hashed_seed = hash_seed(0);
        assert_eq!(8794265229978523055, hashed_seed);

        let hashed_seed = hash_seed((-777i64) as u64);
        assert_eq!(-1087248400229165450, hashed_seed);
    }

    #[test]
    fn proper_network_bits_per_entry() {
        let id_to_test = 1 << BIOME_NETWORK_MAX_BITS;
        assert!(
            Biome::from_id(id_to_test).is_none(),
            "We need to update our constants!"
        );
    }
}
