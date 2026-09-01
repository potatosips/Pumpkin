use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::{
    attributes::Attributes, data_component_impl::EquipmentSlot, entity::EntityType, item::Item,
    item_stack::ItemStack, sound::Sound, tracked_data,
};
use pumpkin_protocol::java::client::play::Metadata;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, LightningBoltEntity, NBTStorage, NbtFuture,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    mob::skeleton::skeleton::SkeletonEntity,
    mob::{Mob, MobEntity},
    player::Player,
};

pub struct SkeletonHorseEntity {
    pub mob_entity: MobEntity,
    ageable_data: AgeableData,
    tamed: AtomicBool,
    temper: AtomicI32,
    owner: AtomicCell<Option<Uuid>>,
    saddled: AtomicBool,
    skeleton_trap: AtomicBool,
    skeleton_trap_time: AtomicI32,
    rider_control: super::horse_food::EquineRiderControl,
    animation_state: super::horse_food::EquineAnimationState,
}

impl SkeletonHorseEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mut rng = rand::rng();
        let horse = Self {
            mob_entity: MobEntity::new(entity),
            ageable_data: AgeableData::default(),
            tamed: AtomicBool::new(false),
            temper: AtomicI32::new(0),
            owner: AtomicCell::new(None),
            saddled: AtomicBool::new(false),
            skeleton_trap: AtomicBool::new(false),
            skeleton_trap_time: AtomicI32::new(0),
            rider_control: super::horse_food::EquineRiderControl::default(),
            animation_state: super::horse_food::EquineAnimationState::default(),
        };
        horse.mob_entity.living_entity.set_attribute_base(
            &Attributes::JUMP_STRENGTH,
            randomized_undead_jump_strength(
                rng.random::<f64>(),
                rng.random::<f64>(),
                rng.random::<f64>(),
            ),
        );
        let mob_arc = Arc::new(horse);
        let mob_weak: Weak<dyn Mob> = {
            let mob: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob)
        };
        {
            let mut goals = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            goals.add_goal(0, Box::new(SwimGoal::default()));
            goals.add_goal(1, Box::new(WanderAroundGoal::new(0.7)));
            goals.add_goal(
                2,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goals.add_goal(3, Box::new(RandomLookAroundGoal::default()));
        }
        mob_arc
    }

    fn set_tamed(&self, tamed: bool, owner: Option<Uuid>) {
        self.tamed.store(tamed, Ordering::Relaxed);
        self.owner.store(if tamed { owner } else { None });
        self.sync_horse_flags();
    }

    fn sync_horse_flags(&self) {
        let flags = horse_flags(
            self.tamed.load(Ordering::Relaxed),
            self.saddled.load(Ordering::Relaxed),
        ) | self.animation_state.flags();
        self.get_entity().send_meta_data(
            &[Metadata::new(
                tracked_data::abstract_horse::DATA_ID_FLAGS,
                flags as i8,
            )],
            None,
        );
    }

    pub fn set_skeleton_trap(&self, trap: bool) {
        if self.get_entity().entity_type == &EntityType::SKELETON_HORSE {
            self.skeleton_trap.store(trap, Ordering::Relaxed);
        }
    }

    async fn create_trap_rider(
        world: &Arc<crate::world::World>,
        position: Vector3<f64>,
        difficulty: &crate::entity::mob::equipment::RegionalDifficulty,
    ) -> Arc<SkeletonEntity> {
        let rider =
            SkeletonEntity::new(Entity::new(world.clone(), position, &EntityType::SKELETON));
        rider.get_mob_entity().set_persistence_required(true);
        rider
            .get_mob_entity()
            .living_entity
            .hurt_cooldown
            .store(60, Ordering::Relaxed);
        world.spawn_entity(rider.clone()).await;
        let living = &rider.get_mob_entity().living_entity;
        let mut bow = ItemStack::new(1, &Item::BOW);
        let mut helmet = ItemStack::new(1, &Item::IRON_HELMET);
        crate::entity::mob::equipment::apply_vanilla_enchantments(
            &mut bow,
            &EquipmentSlot::MAIN_HAND,
            difficulty.special_multiplier,
        );
        crate::entity::mob::equipment::apply_vanilla_enchantments(
            &mut helmet,
            &EquipmentSlot::HEAD,
            difficulty.special_multiplier,
        );
        {
            let mut equipment = living.entity_equipment.lock().await;
            equipment.put(&EquipmentSlot::MAIN_HAND, bow.clone());
            equipment.put(&EquipmentSlot::HEAD, helmet.clone());
        }
        living
            .equipment_drop_chances
            .lock()
            .await
            .insert(EquipmentSlot::HEAD, 0.0);
        living.send_equipment_changes(&[
            (EquipmentSlot::MAIN_HAND, bow),
            (EquipmentSlot::HEAD, helmet),
        ]);
        rider
    }

    async fn activate_skeleton_trap(&self, caller: &Arc<dyn EntityBase>) {
        self.set_skeleton_trap(false);
        self.set_tamed(true, None);
        self.set_age(0);
        self.mob_entity.set_persistence_required(true);

        let entity = self.get_entity();
        let world = entity.world.load();
        let position = entity.pos.load();
        let difficulty = crate::entity::mob::equipment::RegionalDifficulty::at(&world, position);

        let lightning = Arc::new(LightningBoltEntity::new(Entity::new(
            world.clone(),
            position,
            &EntityType::LIGHTNING_BOLT,
        )));
        lightning.set_visual_only(true);
        world.spawn_entity(lightning).await;

        let original_rider = Self::create_trap_rider(&world, position, &difficulty).await;
        entity
            .add_passenger(caller.clone(), original_rider as Arc<dyn EntityBase>)
            .await;

        for _ in 0..3 {
            let horse = Self::new(Entity::new(
                world.clone(),
                position,
                &EntityType::SKELETON_HORSE,
            ));
            horse.set_tamed(true, None);
            horse.set_age(0);
            horse.get_mob_entity().set_persistence_required(true);
            horse
                .get_mob_entity()
                .living_entity
                .hurt_cooldown
                .store(60, Ordering::Relaxed);
            horse.get_entity().velocity.store(Vector3::new(
                (rand::random::<f64>() - rand::random::<f64>()) * 1.1485,
                0.0,
                (rand::random::<f64>() - rand::random::<f64>()) * 1.1485,
            ));
            horse
                .get_entity()
                .velocity_dirty
                .store(true, Ordering::Relaxed);
            world.spawn_entity(horse.clone()).await;

            let rider = Self::create_trap_rider(&world, position, &difficulty).await;
            horse
                .get_entity()
                .add_passenger(horse.clone(), rider as Arc<dyn EntityBase>)
                .await;
        }
    }
}

const fn horse_flags(tamed: bool, saddled: bool) -> u8 {
    (if tamed { 0x02 } else { 0 }) | (if saddled { 0x04 } else { 0 })
}

fn randomized_undead_jump_strength(first: f64, second: f64, third: f64) -> f64 {
    0.400_000_005_960_464_5 + first * 0.2 + second * 0.2 + third * 0.2
}

impl AgeableMob for SkeletonHorseEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl super::horse_food::Equine for SkeletonHorseEntity {
    fn animation_state(&self) -> Option<&super::horse_food::EquineAnimationState> {
        Some(&self.animation_state)
    }

    fn sync_equine_flags(&self) {
        self.sync_horse_flags();
    }

    fn temper(&self) -> i32 {
        self.temper.load(Ordering::Relaxed)
    }
    fn set_tamed(&self, tamed: bool, owner: Option<Uuid>) {
        SkeletonHorseEntity::set_tamed(self, tamed, owner);
    }
    fn can_breed(&self) -> bool {
        false
    }
}

impl NBTStorage for SkeletonHorseEntity {
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
            if self.get_entity().entity_type == &EntityType::SKELETON_HORSE {
                nbt.put_bool("SkeletonTrap", self.skeleton_trap.load(Ordering::Relaxed));
                nbt.put_int(
                    "SkeletonTrapTime",
                    self.skeleton_trap_time.load(Ordering::Relaxed),
                );
            }
            let saddle = self.saddle_stack().await;
            if !saddle.is_empty() {
                let mut saddle_nbt = pumpkin_nbt::compound::NbtCompound::new();
                saddle.write_item_stack(&mut saddle_nbt);
                nbt.put_compound("SaddleItem", saddle_nbt);
            }
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
            if self.get_entity().entity_type == &EntityType::SKELETON_HORSE {
                self.set_skeleton_trap(nbt.get_bool("SkeletonTrap").unwrap_or(false));
                self.skeleton_trap_time.store(
                    nbt.get_int("SkeletonTrapTime").unwrap_or(0).max(0),
                    Ordering::Relaxed,
                );
            }
            let saddle = nbt
                .get_compound("SaddleItem")
                .and_then(ItemStack::read_item_stack)
                .filter(|stack| stack.item == &Item::SADDLE)
                .unwrap_or_else(|| ItemStack::EMPTY.clone());
            self.set_saddle_stack(saddle).await;
        })
    }
}

impl Mob for SkeletonHorseEntity {
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
        self.sync_horse_flags();
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
            entity, None, 0,
        )))
    }

    fn mob_on_death<'a>(&'a self, _cause: Option<&'a dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            super::chested_horse::drop_mount_inventory_on_death(&self.mob_entity, None).await;
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
    fn mob_tick<'a>(&'a self, caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            if self.skeleton_trap.load(Ordering::Relaxed) {
                if self
                    .get_entity()
                    .world
                    .load()
                    .get_closest_player(self.get_entity().pos.load(), 10.0)
                    .is_some()
                {
                    self.activate_skeleton_trap(caller).await;
                    return;
                }
                let current = self.skeleton_trap_time.load(Ordering::Relaxed);
                self.skeleton_trap_time
                    .store(current.saturating_add(1), Ordering::Relaxed);
                if skeleton_trap_expires(current) {
                    self.get_entity().remove().await;
                    return;
                }
            }
            self.ageable_ai_step();
            super::horse_food::tick_equine_animations(self);
            super::horse_food::tick_equine_natural_regeneration(self);
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
            if self.is_tame()
                && super::horse_food::feed_equine(self, player, stack, Sound::EntityHorseEat).await
            {
                return true;
            }
            if self.is_tame() && super::horse_food::mount_equine(self, player, stack).await {
                return true;
            }
            self.mob_entity.mob_interact(player, stack).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{horse_flags, randomized_undead_jump_strength, skeleton_trap_expires};

    #[test]
    fn skeleton_horse_tame_and_saddle_metadata_bits_match_abstract_horse() {
        assert_eq!(horse_flags(false, false), 0);
        assert_eq!(horse_flags(true, false), 0x02);
        assert_eq!(horse_flags(false, true), 0x04);
        assert_eq!(horse_flags(true, true), 0x06);
    }

    #[test]
    fn skeleton_trap_expires_after_vanilla_fifteen_minute_counter() {
        assert!(!skeleton_trap_expires(17_999));
        assert!(skeleton_trap_expires(18_000));
    }

    #[test]
    fn undead_horse_jump_strength_uses_three_vanilla_random_terms() {
        assert_eq!(
            randomized_undead_jump_strength(0.0, 0.0, 0.0),
            0.400_000_005_960_464_5
        );
        assert!(
            (randomized_undead_jump_strength(1.0, 1.0, 1.0) - 1.000_000_005_960_464_5).abs()
                < 1.0e-12
        );
    }
}

const fn skeleton_trap_expires(current_trap_time: i32) -> bool {
    current_trap_time >= 18_000
}
