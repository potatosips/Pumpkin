use crate::block::entities::BlockEntity;
use crate::world::World;
use crossbeam::atomic::AtomicCell;
use pumpkin_data::block_properties::HorizontalFacing;
use pumpkin_data::potion::Effect;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{EntityType as EntityTypeTag, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use std::any::Any;
use std::pin::Pin;
use std::sync::Arc;

pub struct BellBlockEntity {
    pub position: BlockPos,
    pub last_side_hit: AtomicCell<Option<HorizontalFacing>>,
    pub ring_ticks: AtomicCell<i32>,
    pub ringing: AtomicCell<bool>,
    resonating: AtomicCell<bool>,
    resonate_time: AtomicCell<i32>,
}

impl BellBlockEntity {
    pub const ID: &'static str = "minecraft:bell";
    const HEARING_DISTANCE: f64 = 32.0;
    const HIGHLIGHT_DISTANCE: f64 = 48.0;
    const RESONATE_TICKS: i32 = 40;
    const GLOWING_TICKS: i32 = 60;
    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            last_side_hit: AtomicCell::new(None),
            ring_ticks: AtomicCell::new(0),
            resonate_time: AtomicCell::new(0),
            resonating: AtomicCell::new(false),
            ringing: AtomicCell::new(false),
        }
    }
    pub fn activate(&self, direction: HorizontalFacing) {
        self.last_side_hit.store(Some(direction));
        if self.ringing.load() {
            self.ring_ticks.store(0);
        } else {
            self.ringing.store(true);
        }
    }
    fn raiders_in_range(
        &self,
        world: &Arc<World>,
        radius: f64,
    ) -> Vec<Arc<dyn crate::entity::EntityBase>> {
        let center = self.position.to_f64().add_raw(0.5, 0.5, 0.5);
        let bounds = BoundingBox::new(
            center.add_raw(-radius, -radius, -radius),
            center.add_raw(radius, radius, radius),
        );
        let radius_squared = radius * radius;
        world
            .get_entities_at_box(&bounds)
            .into_iter()
            .filter(|entity| {
                entity
                    .get_entity()
                    .entity_type
                    .has_tag(&EntityTypeTag::MINECRAFT_RAIDERS)
                    && (entity.get_entity().pos.load() - center).length_squared() <= radius_squared
                    && entity.get_entity().is_alive()
            })
            .collect()
    }

    fn raiders_hear_bell(&self, world: &Arc<World>) -> bool {
        !self
            .raiders_in_range(world, Self::HEARING_DISTANCE)
            .is_empty()
    }

    async fn highlight_raiders(&self, world: &Arc<World>) {
        for raider in self.raiders_in_range(world, Self::HIGHLIGHT_DISTANCE) {
            if let Some(living) = raider.get_living_entity() {
                living
                    .add_effect(Effect {
                        effect_type: &pumpkin_data::effect::StatusEffect::GLOWING,
                        duration: Self::GLOWING_TICKS,
                        amplifier: 0,
                        ambient: false,
                        show_particles: true,
                        show_icon: true,
                        blend: false,
                    })
                    .await;
            }
        }
    }
}

impl BlockEntity for BellBlockEntity {
    fn write_nbt<'a>(
        &'a self,
        _nbt: &'a mut NbtCompound,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {})
    }

    fn from_nbt(_nbt: &NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        Self::new(position)
    }

    fn tick<'a>(&'a self, world: &'a Arc<World>) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if self.ringing.load() {
                self.ring_ticks.fetch_add(1);
            }
            if self.ring_ticks.load() >= 50 {
                self.ringing.store(false);
                self.ring_ticks.store(0);
            }
            if self.ring_ticks.load() == 5
                && self.resonate_time.load() == 0
                && self.raiders_hear_bell(world)
            {
                self.resonating.store(true);
                world.play_sound_fine(
                    Sound::BlockBellResonate,
                    SoundCategory::Blocks,
                    &self.position.to_f64(),
                    1.0,
                    1.0,
                );
            }

            if self.resonating.load() {
                if self.resonate_time.load() < Self::RESONATE_TICKS {
                    self.resonate_time.fetch_add(1);
                } else {
                    self.resonating.store(false);
                    self.resonate_time.store(0);
                    self.highlight_raiders(world).await;
                }
            }
        })
    }

    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::entity::EntityType;

    #[test]
    fn vanilla_bell_raider_tag_excludes_unrelated_hostile_mobs() {
        assert!(EntityType::VINDICATOR.has_tag(&EntityTypeTag::MINECRAFT_RAIDERS));
        assert!(EntityType::RAVAGER.has_tag(&EntityTypeTag::MINECRAFT_RAIDERS));
        assert!(!EntityType::CREEPER.has_tag(&EntityTypeTag::MINECRAFT_RAIDERS));
    }

    #[test]
    fn vanilla_bell_distances_and_timing_are_preserved() {
        assert_eq!(BellBlockEntity::HEARING_DISTANCE, 32.0);
        assert_eq!(BellBlockEntity::HIGHLIGHT_DISTANCE, 48.0);
        assert_eq!(BellBlockEntity::RESONATE_TICKS, 40);
        assert_eq!(BellBlockEntity::GLOWING_TICKS, 60);
    }
}
