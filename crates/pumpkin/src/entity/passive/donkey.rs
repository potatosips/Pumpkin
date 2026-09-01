use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::{
    entity::EntityType, item::Item, item_stack::ItemStack, sound::Sound, tracked_data,
};
use pumpkin_protocol::java::client::play::Metadata;
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

/// Represents a Donkey, a passive mob that can be tamed and equipped with chests.
///
/// Wiki: <https://minecraft.wiki/w/Donkey>
pub struct DonkeyEntity {
    pub mob_entity: MobEntity,
    ageable_data: AgeableData,
    tamed: AtomicBool,
    temper: AtomicI32,
    owner: AtomicCell<Option<Uuid>>,
    saddled: AtomicBool,
}

impl DonkeyEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let donkey = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            tamed: AtomicBool::new(false),
            temper: AtomicI32::new(0),
            owner: AtomicCell::new(None),
            saddled: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(donkey);
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

    fn set_tamed(&self, tamed: bool, owner: Option<Uuid>) {
        self.tamed.store(tamed, Ordering::Relaxed);
        self.owner.store(if tamed { owner } else { None });
        self.sync_flags();
    }

    fn sync_flags(&self) {
        let flags = (if self.tamed.load(Ordering::Relaxed) {
            0x02
        } else {
            0
        }) | (if self.saddled.load(Ordering::Relaxed) {
            0x04
        } else {
            0
        });
        self.get_entity().send_meta_data(
            &[Metadata::new(
                tracked_data::abstract_horse::DATA_ID_FLAGS,
                flags as i8,
            )],
            None,
        );
    }
}

impl AgeableMob for DonkeyEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}
impl super::horse_food::Equine for DonkeyEntity {
    fn temper(&self) -> i32 {
        self.temper.load(Ordering::Relaxed)
    }

    fn add_temper(&self, amount: i32) {
        self.temper
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some((value + amount).clamp(0, 100))
            })
            .ok();
    }

    fn set_tamed(&self, tamed: bool, owner: Option<Uuid>) {
        DonkeyEntity::set_tamed(self, tamed, owner);
    }
}

impl super::animal::Animal for DonkeyEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        matches!(
            item_stack.item.id,
            id if id == Item::GOLDEN_CARROT.id
                || id == Item::GOLDEN_APPLE.id
                || id == Item::ENCHANTED_GOLDEN_APPLE.id
        )
    }
}
impl NBTStorage for DonkeyEntity {
    fn write_nbt<'a>(
        &'a self,
        nbt: &'a mut pumpkin_nbt::compound::NbtCompound,
    ) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.write_nbt(nbt).await;
            self.write_ageable_nbt(nbt);
            super::animal::Animal::write_animal_nbt(self, nbt);
            nbt.put_bool("Tame", self.tamed.load(Ordering::Relaxed));
            nbt.put_int("Temper", self.temper.load(Ordering::Relaxed));
            if let Some(owner) = self.owner.load() {
                nbt.put_uuid("Owner", owner);
            }
            if self.saddled.load(Ordering::Relaxed) {
                let mut saddle = pumpkin_nbt::compound::NbtCompound::new();
                ItemStack::new(1, &Item::SADDLE).write_item_stack(&mut saddle);
                nbt.put_compound("SaddleItem", saddle);
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
            self.temper.store(
                nbt.get_int("Temper").unwrap_or(0).clamp(0, 100),
                Ordering::Relaxed,
            );
            self.set_tamed(nbt.get_bool("Tame").unwrap_or(false), nbt.get_uuid("Owner"));
            self.set_saddled(
                nbt.get_compound("SaddleItem")
                    .and_then(ItemStack::read_item_stack)
                    .is_some_and(|stack| stack.item == &Item::SADDLE),
            );
        })
    }
}

impl Mob for DonkeyEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }
    fn is_tame(&self) -> bool {
        self.tamed.load(Ordering::Relaxed)
    }
    fn can_mate_with(&self, mate: &dyn EntityBase) -> bool {
        super::horse_food::horse_family_offspring_type(
            &EntityType::DONKEY,
            mate.get_entity().entity_type,
        )
        .is_some()
    }
    fn breeding_offspring_type(&self, mate: &dyn EntityBase) -> &'static EntityType {
        super::horse_food::horse_family_offspring_type(
            &EntityType::DONKEY,
            mate.get_entity().entity_type,
        )
        .unwrap_or(&EntityType::DONKEY)
    }
    fn configure_bred_child<'a>(
        &'a self,
        mate: &'a dyn EntityBase,
        child: &'a Arc<dyn EntityBase>,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            super::horse_food::configure_bred_equine_attributes(
                &self.mob_entity.living_entity,
                mate,
                child,
            );
        })
    }
    fn is_saddled(&self) -> bool {
        self.saddled.load(Ordering::Relaxed)
    }
    fn can_be_saddled(&self) -> bool {
        self.get_entity().is_alive() && self.is_tame() && !self.is_baby()
    }
    fn set_saddled(&self, saddled: bool) {
        self.saddled.store(saddled, Ordering::Relaxed);
        self.sync_flags();
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
            if super::horse_food::feed_equine(self, player, stack, Sound::EntityHorseEat).await {
                return true;
            }
            if super::horse_food::mount_equine(self, player, stack).await {
                return true;
            }
            self.mob_entity.mob_interact(player, stack).await
        })
    }
}
