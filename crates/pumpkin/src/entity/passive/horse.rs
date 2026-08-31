use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::{entity::EntityType, item_stack::ItemStack, sound::Sound, tracked_data};
use pumpkin_protocol::java::client::play::Metadata;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, NBTStorage, NbtFuture,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    player::Player,
};

/// Represents a Horse, a passive mob that can be tamed and ridden.
///
/// Wiki: <https://minecraft.wiki/w/Horse>
pub struct HorseEntity {
    pub mob_entity: MobEntity,
    ageable_data: AgeableData,
    variant: AtomicI32,
    tamed: std::sync::atomic::AtomicBool,
    temper: AtomicI32,
    owner: AtomicCell<Option<Uuid>>,
}

impl HorseEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let horse = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(random_variant()),
            tamed: std::sync::atomic::AtomicBool::new(false),
            temper: AtomicI32::new(0),
            owner: AtomicCell::new(None),
        };
        let mob_arc = Arc::new(horse);
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
            goal_selector.add_goal(1, Box::new(WanderAroundGoal::new(0.7)));
            goal_selector.add_goal(
                2,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(3, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    pub fn variant(&self) -> i32 {
        self.variant.load(Ordering::Relaxed)
    }

    pub fn set_variant(&self, variant: i32) {
        let variant = normalize_variant(variant);
        self.variant.store(variant, Ordering::Relaxed);
        self.get_entity().send_meta_data(
            &[Metadata::new(
                tracked_data::horse::DATA_ID_TYPE_VARIANT,
                variant,
            )],
            None,
        );
    }

    pub fn set_tamed(&self, tamed: bool, owner: Option<Uuid>) {
        self.tamed.store(tamed, Ordering::Relaxed);
        self.owner.store(if tamed { owner } else { None });
        let flags = if tamed { 0x02u8 } else { 0u8 };
        self.get_entity().send_meta_data(
            &[Metadata::new(
                tracked_data::abstract_horse::DATA_ID_FLAGS,
                flags as i8,
            )],
            None,
        );
    }

    pub fn add_temper(&self, amount: i32) {
        self.temper
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some((value + amount).clamp(0, 100))
            })
            .ok();
    }
}

fn encode_variant(color: i32, markings: i32) -> i32 {
    color.clamp(0, 6) | (markings.clamp(0, 4) << 8)
}

fn normalize_variant(variant: i32) -> i32 {
    encode_variant(variant & 0xff, (variant >> 8) & 0xff)
}

fn random_variant() -> i32 {
    let mut rng = rand::rng();
    encode_variant(rng.random_range(0..=6), rng.random_range(0..=4))
}

impl AgeableMob for HorseEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl NBTStorage for HorseEntity {
    fn write_nbt<'a>(
        &'a self,
        nbt: &'a mut pumpkin_nbt::compound::NbtCompound,
    ) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.write_nbt(nbt).await;
            self.write_ageable_nbt(nbt);
            nbt.put_int("Variant", self.variant());
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
            if let Some(variant) = nbt.get_int("Variant") {
                self.set_variant(variant);
            }
            self.temper.store(
                nbt.get_int("Temper").unwrap_or(0).clamp(0, 100),
                Ordering::Relaxed,
            );
            self.set_tamed(nbt.get_bool("Tame").unwrap_or(false), nbt.get_uuid("Owner"));
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horse_variant_encoding_and_clamping() {
        assert_eq!(encode_variant(0, 0), 0);
        assert_eq!(encode_variant(6, 4), 1030);
        assert_eq!(normalize_variant(2 | (3 << 8)), 770);
        assert_eq!(normalize_variant(255 | (255 << 8)), 1030);
    }
}

impl Mob for HorseEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }
    fn is_tame(&self) -> bool {
        self.tamed.load(Ordering::Relaxed)
    }
    fn mob_tick<'a>(&'a self, _caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move { self.ageable_ai_step() })
    }
    fn mob_interact<'a>(
        &'a self,
        player: &'a Arc<Player>,
        stack: &'a mut ItemStack,
    ) -> EntityBaseFuture<'a, bool> {
        Box::pin(async move {
            if super::horse_food::feed_equine(self, player, stack, Sound::EntityHorseEat).await {
                return true;
            }
            self.mob_entity.mob_interact(player, stack).await
        })
    }
}
