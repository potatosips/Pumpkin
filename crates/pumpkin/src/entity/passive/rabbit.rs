use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, Ordering},
};

use pumpkin_data::{
    entity::EntityType,
    item::Item,
    item_stack::ItemStack,
    sound::Sound,
    tag::{self, Taggable},
    tracked_data,
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::java::client::play::Metadata;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, NBTStorage, NbtFuture,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        breed::BreedGoal, escape_danger::EscapeDangerGoal, follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        tempt::TemptGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

const TEMPT_ITEMS: &[&Item] = &[&Item::CARROT, &Item::GOLDEN_CARROT, &Item::DANDELION];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum RabbitVariant {
    Brown = 0,
    White = 1,
    Black = 2,
    WhiteSplotched = 3,
    Gold = 4,
    Salt = 5,
    Killer = 99,
}

impl RabbitVariant {
    fn from_id(id: i32) -> Self {
        match id {
            1 => Self::White,
            2 => Self::Black,
            3 => Self::WhiteSplotched,
            4 => Self::Gold,
            5 => Self::Salt,
            99 => Self::Killer,
            _ => Self::Brown,
        }
    }

    fn natural_for_biome(biome: &str, roll: f32) -> Self {
        if biome == "minecraft:desert" {
            return Self::Gold;
        }
        if matches!(
            biome,
            "minecraft:snowy_plains"
                | "minecraft:snowy_taiga"
                | "minecraft:snowy_slopes"
                | "minecraft:frozen_peaks"
                | "minecraft:jagged_peaks"
                | "minecraft:grove"
        ) {
            return if roll < 0.8 {
                Self::White
            } else {
                Self::WhiteSplotched
            };
        }
        if roll < 0.5 {
            Self::Brown
        } else if roll < 0.9 {
            Self::Salt
        } else {
            Self::Black
        }
    }
}

pub struct RabbitEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    variant: AtomicI32,
}

impl RabbitEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let biome = mob_entity
            .living_entity
            .entity
            .world
            .load()
            .get_biome(&mob_entity.living_entity.entity.block_pos.load())
            .registry_id;
        let variant = RabbitVariant::natural_for_biome(biome, rand::random());
        let rabbit = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(variant as i32),
        };
        let mob_arc = Arc::new(rabbit);
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

            goal_selector.add_goal(1, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, EscapeDangerGoal::new(2.2));
            goal_selector.add_goal(2, BreedGoal::new(0.8));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.0, TEMPT_ITEMS)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(0.8)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(0.6)));
            goal_selector.add_goal(
                11,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 10.0),
            );
            goal_selector.add_goal(11, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    pub fn variant(&self) -> RabbitVariant {
        RabbitVariant::from_id(self.variant.load(Ordering::Relaxed))
    }

    pub fn set_variant(&self, variant: RabbitVariant) {
        self.variant.store(variant as i32, Ordering::Relaxed);
        self.get_entity().send_meta_data(
            &[Metadata::new(
                tracked_data::rabbit::DATA_TYPE_ID,
                variant as i32,
            )],
            None,
        );
    }
}

impl NBTStorage for RabbitEntity {
    fn write_nbt<'a>(&'a self, nbt: &'a mut NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.write_nbt(nbt).await;
            self.write_ageable_nbt(nbt);
            self.write_animal_nbt(nbt);
            nbt.put_int("RabbitType", self.variant.load(Ordering::Relaxed));
        })
    }

    fn read_nbt_non_mut<'a>(&'a self, nbt: &'a NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            self.read_ageable_nbt(nbt);
            self.read_animal_nbt(nbt);
            if let Some(variant) = nbt.get_int("RabbitType") {
                self.set_variant(RabbitVariant::from_id(variant));
            }
        })
    }
}

impl AgeableMob for RabbitEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for RabbitEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack
            .get_item()
            .has_tag(&tag::Item::MINECRAFT_RABBIT_FOOD)
            || TEMPT_ITEMS.iter().any(|item| item.id == item_stack.item.id)
    }
}

impl Mob for RabbitEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_rabbit(&self) -> Option<&RabbitEntity> {
        Some(self)
    }

    fn configure_bred_child<'a>(
        &'a self,
        mate: &'a dyn EntityBase,
        child: &'a Arc<dyn EntityBase>,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            let (Some(mate), Some(child)) = (
                mate.get_mob().and_then(Mob::get_rabbit),
                child.get_mob().and_then(Mob::get_rabbit),
            ) else {
                return;
            };
            let variant = if self.get_random().random_range(0..20) == 0 {
                let entity = self.get_entity();
                let biome = entity
                    .world
                    .load()
                    .get_biome(&entity.block_pos.load())
                    .registry_id;
                RabbitVariant::natural_for_biome(biome, rand::random())
            } else if self.get_random().random_bool(0.5) {
                self.variant()
            } else {
                mate.variant()
            };
            child.set_variant(variant);
        })
    }

    fn mob_tick<'a>(&'a self, _caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move { self.ageable_ai_step() })
    }

    fn mob_interact<'a>(
        &'a self,
        player: &'a Arc<Player>,
        item_stack: &'a mut ItemStack,
    ) -> EntityBaseFuture<'a, bool> {
        Box::pin(async move {
            self.animal_interact(player, item_stack, Sound::EntityRabbitAmbient)
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_variant_distribution_boundaries() {
        assert_eq!(
            RabbitVariant::natural_for_biome("minecraft:desert", 0.0),
            RabbitVariant::Gold
        );
        assert_eq!(
            RabbitVariant::natural_for_biome("minecraft:snowy_plains", 0.79),
            RabbitVariant::White
        );
        assert_eq!(
            RabbitVariant::natural_for_biome("minecraft:snowy_plains", 0.8),
            RabbitVariant::WhiteSplotched
        );
        assert_eq!(
            RabbitVariant::natural_for_biome("minecraft:plains", 0.49),
            RabbitVariant::Brown
        );
        assert_eq!(
            RabbitVariant::natural_for_biome("minecraft:plains", 0.5),
            RabbitVariant::Salt
        );
        assert_eq!(
            RabbitVariant::natural_for_biome("minecraft:plains", 0.9),
            RabbitVariant::Black
        );
    }
}
