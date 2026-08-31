use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::Metadata;

use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, NBTStorage, NbtFuture,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        breed::BreedGoal, lay_frog_spawn::LayFrogSpawnGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, swim::SwimGoal, tempt::TemptGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

pub const FROG_FOOD: &[&Item] = &[&Item::SLIME_BALL];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum FrogVariant {
    Cold = 0,
    #[default]
    Temperate = 1,
    Warm = 2,
}

impl FrogVariant {
    pub const fn for_temperature(temperature: f32) -> Self {
        if temperature < 0.5 {
            Self::Cold
        } else if temperature > 1.0 {
            Self::Warm
        } else {
            Self::Temperate
        }
    }

    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            0 => Self::Cold,
            2 => Self::Warm,
            _ => Self::Temperate,
        }
    }

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cold => "minecraft:cold",
            Self::Temperate => "minecraft:temperate",
            Self::Warm => "minecraft:warm",
        }
    }

    #[must_use]
    pub fn from_name(s: &str) -> Self {
        match s {
            "minecraft:cold" | "cold" => Self::Cold,
            "minecraft:warm" | "warm" => Self::Warm,
            _ => Self::Temperate,
        }
    }
}

/// Represents a Frog, an amphibious mob that can eat small slimes and magma cubes.
///
/// Wiki: <https://minecraft.wiki/w/Frog>
pub struct FrogEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub variant: AtomicI32,
    pub tongue_target_id: AtomicI32,
    pregnant: AtomicBool,
}

impl FrogEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let frog = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(FrogVariant::Temperate.id()),
            tongue_target_id: AtomicI32::new(-1),
            pregnant: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(frog);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, Box::new(LayFrogSpawnGoal::new(mob_arc.clone())));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(1, Box::new(TemptGoal::new(1.0, FROG_FOOD)));
            goal_selector.add_goal(3, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                3,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(4, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> FrogVariant {
        FrogVariant::from_id(self.variant.load(Ordering::Relaxed))
    }

    pub fn set_variant(&self, variant: FrogVariant) {
        self.variant.store(variant.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.send_meta_data(
            &[Metadata::new(
                pumpkin_data::tracked_data::frog::VARIANT,
                VarInt(variant.id()),
            )],
            None,
        );
    }

    pub fn is_pregnant(&self) -> bool {
        self.pregnant.load(Ordering::Relaxed)
    }

    pub fn set_pregnant(&self, pregnant: bool) {
        self.pregnant.store(pregnant, Ordering::Relaxed);
    }
}

impl AgeableMob for FrogEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for FrogEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_FROG_FOOD)
            || FROG_FOOD.iter().any(|i| i.id == item_stack.item.id)
    }
}

impl NBTStorage for FrogEntity {
    fn write_nbt<'a>(&'a self, nbt: &'a mut NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.write_nbt(nbt).await;
            self.write_ageable_nbt(nbt);
            self.write_animal_nbt(nbt);
            nbt.put_string("variant", self.get_variant().as_str().to_string());
        })
    }

    fn read_nbt_non_mut<'a>(&'a self, nbt: &'a NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            self.read_ageable_nbt(nbt);
            self.read_animal_nbt(nbt);
            if let Some(variant_str) = nbt.get_string("variant") {
                self.set_variant(FrogVariant::from_name(variant_str));
            }
        })
    }
}

impl Mob for FrogEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn can_breed_now(&self) -> bool {
        !self.is_pregnant()
    }

    fn spawn_breeding_offspring<'a>(
        &'a self,
        _mate: &'a dyn EntityBase,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move { self.set_pregnant(true) })
    }

    fn mob_set_variant_name(&self, name: &str) {
        self.set_variant(FrogVariant::from_name(name));
    }

    fn mob_tick<'a>(&'a self, _caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            self.ageable_ai_step();
        })
    }

    fn mob_init_data_tracker(&self) -> EntityBaseFuture<'_, ()> {
        Box::pin(async move {
            let entity = self.get_entity();
            let is_baby = entity.age.load(Ordering::Relaxed) < 0;
            if is_baby {
                entity.send_meta_data(
                    &[Metadata::new(
                        pumpkin_data::tracked_data::frog::BABY_ID,
                        true,
                    )],
                    None,
                );
            }
            entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::frog::VARIANT,
                    VarInt(self.get_variant().id()),
                )],
                None,
            );
        })
    }

    fn mob_interact<'a>(
        &'a self,
        player: &'a Arc<Player>,
        item_stack: &'a mut ItemStack,
    ) -> EntityBaseFuture<'a, bool> {
        Box::pin(async move {
            self.animal_interact(player, item_stack, Sound::EntityFrogAmbient)
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frog_variant_id_and_name_parity() {
        assert_eq!(FrogVariant::from_id(0), FrogVariant::Cold);
        assert_eq!(FrogVariant::from_id(1), FrogVariant::Temperate);
        assert_eq!(FrogVariant::from_id(2), FrogVariant::Warm);

        assert_eq!(FrogVariant::Cold.as_str(), "minecraft:cold");
        assert_eq!(FrogVariant::Temperate.as_str(), "minecraft:temperate");
        assert_eq!(FrogVariant::Warm.as_str(), "minecraft:warm");

        assert_eq!(FrogVariant::from_name("cold"), FrogVariant::Cold);
        assert_eq!(FrogVariant::from_name("warm"), FrogVariant::Warm);
        assert_eq!(FrogVariant::from_name("unknown"), FrogVariant::Temperate);
        assert_eq!(FrogVariant::for_temperature(0.49), FrogVariant::Cold);
        assert_eq!(FrogVariant::for_temperature(0.5), FrogVariant::Temperate);
        assert_eq!(FrogVariant::for_temperature(1.0), FrogVariant::Temperate);
        assert_eq!(FrogVariant::for_temperature(1.01), FrogVariant::Warm);
    }
}
