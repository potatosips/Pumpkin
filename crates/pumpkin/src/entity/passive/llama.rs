use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::{
    entity::EntityType, item::Item, item_stack::ItemStack, sound::Sound, tracked_data,
};
use pumpkin_protocol::java::client::play::Metadata;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, NBTStorage, NbtFuture,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        breed::BreedGoal, look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal,
        swim::SwimGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    player::Player,
};

/// Represents a Llama, a neutral mob that can be used for carrying items and spits at enemies.
///
/// Wiki: <https://minecraft.wiki/w/Llama>
pub struct LlamaEntity {
    pub mob_entity: MobEntity,
    ageable_data: AgeableData,
    variant: AtomicI32,
    strength: AtomicI32,
    tamed: AtomicBool,
    temper: AtomicI32,
    owner: AtomicCell<Option<Uuid>>,
}

impl LlamaEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let mut rng = rand::rng();
        let strength_max = if rng.random_bool(0.04) { 5 } else { 3 };
        let llama = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(rng.random_range(0..=3)),
            strength: AtomicI32::new(rng.random_range(1..=strength_max)),
            tamed: AtomicBool::new(false),
            temper: AtomicI32::new(0),
            owner: AtomicCell::new(None),
        };
        let mob_arc = Arc::new(llama);
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
            goal_selector.add_goal(1, BreedGoal::new(1.0));
            goal_selector.add_goal(2, Box::new(WanderAroundGoal::new(0.7)));
            goal_selector.add_goal(
                3,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(4, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    pub fn set_variant(&self, variant: i32) {
        let variant = variant.clamp(0, 3);
        self.variant.store(variant, Ordering::Relaxed);
        self.get_entity().send_meta_data(
            &[Metadata::new(tracked_data::llama::DATA_VARIANT_ID, variant)],
            None,
        );
    }

    pub fn set_strength(&self, strength: i32) {
        let strength = strength.clamp(1, 5);
        self.strength.store(strength, Ordering::Relaxed);
        self.get_entity().send_meta_data(
            &[Metadata::new(
                tracked_data::llama::DATA_STRENGTH_ID,
                strength,
            )],
            None,
        );
    }

    fn set_tamed(&self, tamed: bool, owner: Option<Uuid>) {
        self.tamed.store(tamed, Ordering::Relaxed);
        self.owner.store(if tamed { owner } else { None });
        self.get_entity().send_meta_data(
            &[Metadata::new(
                tracked_data::abstract_horse::DATA_ID_FLAGS,
                if tamed { 0x02i8 } else { 0i8 },
            )],
            None,
        );
    }
}

impl AgeableMob for LlamaEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}
impl super::horse_food::Equine for LlamaEntity {
    fn temper(&self) -> i32 {
        self.temper.load(Ordering::Relaxed)
    }

    fn add_temper(&self, amount: i32) {
        self.temper
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some((value + amount).clamp(0, 30))
            })
            .ok();
    }

    fn set_tamed(&self, tamed: bool, owner: Option<Uuid>) {
        LlamaEntity::set_tamed(self, tamed, owner);
    }

    fn max_temper(&self) -> i32 {
        30
    }

    fn food_effect(&self, item: &Item) -> Option<super::horse_food::FoodEffect> {
        match item.id {
            id if id == Item::WHEAT.id => Some(super::horse_food::FoodEffect {
                healing: 2.0,
                growth_seconds: 10,
                temper: 3,
                breeds: false,
            }),
            id if id == Item::HAY_BLOCK.id => Some(super::horse_food::FoodEffect {
                healing: 10.0,
                growth_seconds: 90,
                temper: 6,
                breeds: true,
            }),
            _ => None,
        }
    }
}

impl super::animal::Animal for LlamaEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item == &Item::HAY_BLOCK
    }
}
impl NBTStorage for LlamaEntity {
    fn write_nbt<'a>(
        &'a self,
        nbt: &'a mut pumpkin_nbt::compound::NbtCompound,
    ) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.write_nbt(nbt).await;
            self.write_ageable_nbt(nbt);
            super::animal::Animal::write_animal_nbt(self, nbt);
            nbt.put_int("Variant", self.variant.load(Ordering::Relaxed));
            nbt.put_int("Strength", self.strength.load(Ordering::Relaxed));
            nbt.put_bool("Tame", self.tamed.load(Ordering::Relaxed));
            nbt.put_int("Temper", self.temper.load(Ordering::Relaxed));
            if let Some(owner) = self.owner.load() {
                nbt.put_uuid("Owner", owner);
            }
        })
    }
    fn read_nbt_non_mut<'a>(
        &'a self,
        nbt: &'a pumpkin_nbt::compound::NbtCompound,
    ) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            self.read_ageable_nbt(nbt);
            super::animal::Animal::read_animal_nbt(self, nbt);
            if let Some(variant) = nbt.get_int("Variant") {
                self.set_variant(variant);
            }
            if let Some(strength) = nbt.get_int("Strength") {
                self.set_strength(strength);
            }
            self.temper.store(
                nbt.get_int("Temper").unwrap_or(0).clamp(0, 30),
                Ordering::Relaxed,
            );
            self.set_tamed(nbt.get_bool("Tame").unwrap_or(false), nbt.get_uuid("Owner"));
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn llama_state_ranges() {
        assert_eq!(0.clamp(0, 3), 0);
        assert_eq!(9.clamp(0, 3), 3);
        assert_eq!((-2).clamp(1, 5), 1);
        assert_eq!(9.clamp(1, 5), 5);
    }
}

impl Mob for LlamaEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }
    fn is_tame(&self) -> bool {
        self.tamed.load(Ordering::Relaxed)
    }
    fn mob_tick<'a>(&'a self, _caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            self.ageable_ai_step();
            super::horse_food::tick_untamed_riding(self).await;
        })
    }
    fn mob_interact<'a>(
        &'a self,
        player: &'a Arc<Player>,
        stack: &'a mut ItemStack,
    ) -> EntityBaseFuture<'a, bool> {
        Box::pin(async move {
            if super::horse_food::feed_equine(self, player, stack, Sound::EntityLlamaEat).await {
                return true;
            }
            if super::horse_food::mount_equine(self, player, stack).await {
                return true;
            }
            self.mob_entity.mob_interact(player, stack).await
        })
    }
}
