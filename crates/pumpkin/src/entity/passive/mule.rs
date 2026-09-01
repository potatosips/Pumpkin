use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::{
    data_component_impl::EquipmentSlot, entity::EntityType, item::Item, item_stack::ItemStack,
    sound::Sound, tracked_data,
};
use pumpkin_protocol::java::client::play::Metadata;
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

/// Represents a Mule, a passive mob created by breeding a horse and a donkey.
///
/// Wiki: <https://minecraft.wiki/w/Mule>
pub struct MuleEntity {
    pub mob_entity: MobEntity,
    ageable_data: AgeableData,
    tamed: AtomicBool,
    temper: AtomicI32,
    owner: AtomicCell<Option<Uuid>>,
    saddled: AtomicBool,
    rider_control: super::horse_food::EquineRiderControl,
    animation_state: super::horse_food::EquineAnimationState,
    pub chested_horse: super::chested_horse::ChestedHorseData,
}

impl MuleEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let mule = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            tamed: AtomicBool::new(false),
            temper: AtomicI32::new(0),
            owner: AtomicCell::new(None),
            saddled: AtomicBool::new(false),
            rider_control: super::horse_food::EquineRiderControl::default(),
            animation_state: super::horse_food::EquineAnimationState::default(),
            chested_horse: super::chested_horse::ChestedHorseData::default(),
        };
        let mob_arc = Arc::new(mule);
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
        }) | self.animation_state.flags();
        self.get_entity().send_meta_data(
            &[Metadata::new(
                tracked_data::abstract_horse::DATA_ID_FLAGS,
                flags as i8,
            )],
            None,
        );
    }
}

impl AgeableMob for MuleEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}
impl super::horse_food::Equine for MuleEntity {
    fn animation_state(&self) -> Option<&super::horse_food::EquineAnimationState> {
        Some(&self.animation_state)
    }

    fn sync_equine_flags(&self) {
        self.sync_flags();
    }

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
        MuleEntity::set_tamed(self, tamed, owner);
    }

    fn can_breed(&self) -> bool {
        false
    }
}
impl NBTStorage for MuleEntity {
    fn write_nbt<'a>(
        &'a self,
        nbt: &'a mut pumpkin_nbt::compound::NbtCompound,
    ) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.write_nbt(nbt).await;
            self.write_ageable_nbt(nbt);
            nbt.put_bool("Tame", self.tamed.load(Ordering::Relaxed));
            nbt.put_int("Temper", self.temper.load(Ordering::Relaxed));
            if let Some(owner) = self.owner.load() {
                nbt.put_uuid("Owner", owner);
            }
            let saddle = self.saddle_stack().await;
            if !saddle.is_empty() {
                let mut saddle_nbt = pumpkin_nbt::compound::NbtCompound::new();
                saddle.write_item_stack(&mut saddle_nbt);
                nbt.put_compound("SaddleItem", saddle_nbt);
            }
            self.chested_horse.write_nbt(nbt).await;
        })
    }
    fn read_nbt_non_mut<'a>(
        &'a self,
        nbt: &'a pumpkin_nbt::compound::NbtCompound,
    ) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            super::chested_horse::sanitize_body_equipment(
                &self.mob_entity,
                super::chested_horse::MountBodySlotKind::None,
            )
            .await;
            self.read_ageable_nbt(nbt);
            self.temper.store(
                nbt.get_int("Temper").unwrap_or(0).clamp(0, 100),
                Ordering::Relaxed,
            );
            self.set_tamed(nbt.get_bool("Tame").unwrap_or(false), nbt.get_uuid("Owner"));
            let saddle = nbt
                .get_compound("SaddleItem")
                .and_then(ItemStack::read_item_stack)
                .filter(|stack| stack.item == &Item::SADDLE)
                .unwrap_or_else(|| ItemStack::EMPTY.clone());
            self.set_saddle_stack(saddle).await;
            self.chested_horse.read_nbt(self, nbt).await;
        })
    }
}

impl Mob for MuleEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }
    fn is_tame(&self) -> bool {
        self.tamed.load(Ordering::Relaxed)
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
    fn saddle_stack(&self) -> EntityBaseFuture<'_, ItemStack> {
        Box::pin(async move {
            self.mob_entity
                .living_entity
                .entity_equipment
                .lock()
                .await
                .get(&EquipmentSlot::SADDLE)
        })
    }
    fn set_saddle_stack(&self, stack: ItemStack) -> EntityBaseFuture<'_, ()> {
        Box::pin(async move {
            let saddled = stack.item == &Item::SADDLE && !stack.is_empty();
            self.mob_entity
                .living_entity
                .entity_equipment
                .lock()
                .await
                .put(
                    &EquipmentSlot::SADDLE,
                    if saddled {
                        stack
                    } else {
                        ItemStack::EMPTY.clone()
                    },
                );
            self.set_saddled(saddled);
        })
    }
    fn create_mount_inventory(
        &self,
        entity: Arc<dyn EntityBase>,
    ) -> Option<Arc<dyn pumpkin_world::inventory::Inventory>> {
        Some(Arc::new(super::chested_horse::MountInventory::new(
            entity,
            self.chested_horse
                .has_chest()
                .then(|| self.chested_horse.inventory.clone()),
            if self.chested_horse.has_chest() {
                15
            } else {
                0
            },
        )))
    }
    fn mob_on_death<'a>(&'a self, _cause: Option<&'a dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            super::chested_horse::drop_mount_inventory_on_death(
                &self.mob_entity,
                Some(&self.chested_horse),
            )
            .await;
        })
    }
    fn set_rider_jump_power(&self, power: i32) {
        self.rider_control.set_jump_power(power);
    }
    fn mob_before_living_tick<'a>(
        &'a self,
        _caller: &'a Arc<dyn EntityBase>,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            super::horse_food::tick_ridden_equine(self, &self.rider_control).await;
        })
    }
    fn mob_tick<'a>(&'a self, _caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            self.ageable_ai_step();
            super::horse_food::tick_equine_animations(self);
            super::horse_food::tick_equine_natural_regeneration(self);
            super::horse_food::tick_untamed_riding(self).await;
        })
    }
    fn mob_interact<'a>(
        &'a self,
        player: &'a Arc<Player>,
        stack: &'a mut ItemStack,
    ) -> EntityBaseFuture<'a, bool> {
        Box::pin(async move {
            if super::horse_food::open_equine_inventory(self, player).await {
                return true;
            }
            if super::horse_food::feed_equine(self, player, stack, Sound::EntityHorseEat).await {
                return true;
            }
            if !stack.is_empty() && !self.is_tame() {
                super::horse_food::make_equine_mad(self, Sound::EntityDonkeyAngry);
                return true;
            }
            if self.is_tame()
                && !self.is_baby()
                && self
                    .chested_horse
                    .try_attach(self, player, stack, Sound::EntityMuleChest)
                    .await
            {
                return true;
            }
            if super::horse_food::mount_equine(self, player, stack).await {
                return true;
            }
            self.mob_entity.mob_interact(player, stack).await
        })
    }
}
