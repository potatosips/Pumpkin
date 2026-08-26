use std::pin::Pin;
use std::sync::Arc;

use pumpkin_data::block_properties::BlockProperties;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;

use crate::world::{BlockFlags, World};

use super::BlockEntity;

type DaylightDetectorProperties = pumpkin_data::block_properties::DaylightDetectorLikeProperties;

pub struct DaylightDetectorBlockEntity {
    pub position: BlockPos,
}

impl BlockEntity for DaylightDetectorBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(_nbt: &NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        Self { position }
    }

    fn write_nbt<'a>(
        &'a self,
        _nbt: &'a mut NbtCompound,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async {})
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn tick<'a>(&'a self, world: &'a Arc<World>) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async {
            if world.get_world_age().await % 20 == 0 && world.dimension.has_skylight {
                Self::update_power(world, &self.position).await;
            }
        })
    }
}

impl DaylightDetectorBlockEntity {
    pub const ID: &'static str = "minecraft:daylight_detector";

    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self { position }
    }

    pub fn calculate_power_raw(
        time_of_day: i64,
        rain_level: f32,
        thunder_level: f32,
        sky_light_level: u8,
        inverted: bool,
    ) -> u8 {
        use std::f32::consts::PI;

        let rain_level = rain_level.clamp(0.0, 1.0);
        let thunder_level = thunder_level.clamp(0.0, 1.0);

        // Sun Angle f
        let sun_angle_fraction = (time_of_day as f32 / 24000.0) - 0.25;
        let mut sun_angle_radians = sun_angle_fraction * (PI * 2.0);

        // Vanilla DimensionType/Level getSkyDarken()
        let cos_val = sun_angle_radians.cos();
        let mut dark = (1.0 - (cos_val * 2.0 + 0.5).clamp(0.0, 1.0)) * 11.0;
        dark += rain_level * (1.0 - dark / 11.0) * 5.0;
        dark += thunder_level * (1.0 - dark / 11.0) * 5.0;
        let sky_darken = dark.round() as u8;

        let mut power = sky_light_level.saturating_sub(sky_darken) as i32;

        if inverted {
            power = 15 - power;
        } else if power > 0 {
            let transition_offset = if sun_angle_radians < PI {
                0.0
            } else {
                PI * 2.0
            };

            sun_angle_radians += (transition_offset - sun_angle_radians) * 0.2;
            power = ((power as f32) * sun_angle_radians.cos()).round() as i32;
        }

        power.clamp(0, 15) as u8
    }

    pub async fn update_power(world: &Arc<World>, block_pos: &BlockPos) {
        let (block, state) = world.get_block_and_state(block_pos);
        if block.id != pumpkin_data::Block::DAYLIGHT_DETECTOR.id {
            return;
        }
        let mut props = DaylightDetectorProperties::from_state_id(state.id, block);

        let level = world.level.clone();
        let inverted = props.inverted;
        let time_of_day = world.get_time_of_day().await;

        let (rain_level, thunder_level) = {
            let weather = world.weather.lock().await;
            (weather.rain_level, weather.thunder_level)
        };

        let sky_light_level = level.light_engine.get_sky_light_level(&level, block_pos);
        let power = Self::calculate_power_raw(
            time_of_day,
            rain_level,
            thunder_level,
            sky_light_level,
            inverted,
        );

        if power != props.power {
            props.power = power;
            let state = props.to_state_id(block);
            world
                .clone()
                .set_block_state(block_pos, state, BlockFlags::NOTIFY_ALL)
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vanilla_daylight_detector_noon_full_power() {
        // At tick 6000 (noon), full daylight gives max power 15
        let power = DaylightDetectorBlockEntity::calculate_power_raw(6000, 0.0, 0.0, 15, false);
        assert_eq!(power, 15);

        // Inverted at noon gives 0 power
        let inverted_power =
            DaylightDetectorBlockEntity::calculate_power_raw(6000, 0.0, 0.0, 15, true);
        assert_eq!(inverted_power, 0);
    }

    #[test]
    fn test_vanilla_daylight_detector_midnight_zero_power() {
        // At tick 18000 (midnight), regular daylight detector outputs 0
        let power = DaylightDetectorBlockEntity::calculate_power_raw(18000, 0.0, 0.0, 15, false);
        assert_eq!(power, 0);

        // Inverted at midnight gives 11 power (since sky_darken is 11, power is 15 - 4 = 11)
        let inverted_power =
            DaylightDetectorBlockEntity::calculate_power_raw(18000, 0.0, 0.0, 15, true);
        assert_eq!(inverted_power, 11);
    }

    #[test]
    fn test_vanilla_daylight_detector_storm_reduces_power() {
        // Heavy rain/thunder at noon darkens sky and reduces power
        let clear_power =
            DaylightDetectorBlockEntity::calculate_power_raw(6000, 0.0, 0.0, 15, false);
        let storm_power =
            DaylightDetectorBlockEntity::calculate_power_raw(6000, 1.0, 1.0, 15, false);
        assert!(storm_power < clear_power);
    }
}
