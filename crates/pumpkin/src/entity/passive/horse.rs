use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::{
    attributes::Attributes,
    data_component_impl::EquipmentSlot,
    entity::EntityType,
    item::Item,
    item_stack::ItemStack,
    sound::Sound,
    tag::{self, Taggable},
    tracked_data,
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
    saddled: AtomicBool,
    rider_control: super::horse_food::EquineRiderControl,
    animation_state: super::horse_food::EquineAnimationState,
}

impl HorseEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let mut rng = rand::rng();
        let max_health =
            15.0 + f64::from(rng.random_range(0..8)) + f64::from(rng.random_range(0..9));
        let movement_speed = (0.449_999_988_079_071_04
            + rng.random::<f64>() * 0.3
            + rng.random::<f64>() * 0.3
            + rng.random::<f64>() * 0.3)
            * 0.25;
        let jump_strength = 0.400_000_005_960_464_5
            + rng.random::<f64>() * 0.2
            + rng.random::<f64>() * 0.2
            + rng.random::<f64>() * 0.2;
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MAX_HEALTH, max_health);
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MOVEMENT_SPEED, movement_speed);
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::JUMP_STRENGTH, jump_strength);
        mob_entity.living_entity.health.store(max_health as f32);
        let horse = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(random_variant()),
            tamed: std::sync::atomic::AtomicBool::new(false),
            temper: AtomicI32::new(0),
            owner: AtomicCell::new(None),
            saddled: AtomicBool::new(false),
            rider_control: super::horse_food::EquineRiderControl::default(),
            animation_state: super::horse_food::EquineAnimationState::default(),
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
        self.sync_horse_flags();
    }

    fn sync_horse_flags(&self) {
        let mut flags = 0u8;
        if self.tamed.load(Ordering::Relaxed) {
            flags |= 0x02;
        }
        if self.saddled.load(Ordering::Relaxed) {
            flags |= 0x04;
        }
        flags |= self.animation_state.flags();
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

impl super::horse_food::Equine for HorseEntity {
    fn animation_state(&self) -> Option<&super::horse_food::EquineAnimationState> {
        Some(&self.animation_state)
    }

    fn sync_equine_flags(&self) {
        self.sync_horse_flags();
    }

    fn temper(&self) -> i32 {
        self.temper.load(Ordering::Relaxed)
    }

    fn add_temper(&self, amount: i32) {
        HorseEntity::add_temper(self, amount);
    }

    fn set_tamed(&self, tamed: bool, owner: Option<Uuid>) {
        HorseEntity::set_tamed(self, tamed, owner);
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

fn inherited_variant(first: i32, second: i32, color_roll: i32, markings_roll: i32) -> i32 {
    let pick = |first: i32, second: i32, roll: i32, random_max: i32| {
        if roll < 4 {
            first
        } else if roll < 8 {
            second
        } else {
            rand::rng().random_range(0..=random_max)
        }
    };
    encode_variant(
        pick(first & 0xff, second & 0xff, color_roll, 6),
        pick((first >> 8) & 0xff, (second >> 8) & 0xff, markings_roll, 4),
    )
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
            super::animal::Animal::write_animal_nbt(self, nbt);
            nbt.put_int("Variant", self.variant());
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
                super::chested_horse::MountBodySlotKind::HorseArmor,
            )
            .await;
            self.read_ageable_nbt(nbt);
            super::animal::Animal::read_animal_nbt(self, nbt);
            if let Some(variant) = nbt.get_int("Variant") {
                self.set_variant(variant);
            }
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

    #[test]
    fn horse_taming_uses_vanilla_temper_probability() {
        assert!(!crate::entity::passive::horse_food::taming_succeeds(
            0, 100, 0
        ));
        assert!(crate::entity::passive::horse_food::taming_succeeds(
            1, 100, 0
        ));
        assert!(!crate::entity::passive::horse_food::taming_succeeds(
            1, 100, 1
        ));
        assert!(crate::entity::passive::horse_food::taming_succeeds(
            100, 100, 99
        ));
    }

    #[test]
    fn horse_variant_inheritance_prefers_parents_eight_ninths() {
        let first = encode_variant(1, 2);
        let second = encode_variant(5, 4);
        assert_eq!(inherited_variant(first, second, 0, 0), first);
        assert_eq!(inherited_variant(first, second, 4, 4), second);
        let mixed = inherited_variant(first, second, 0, 4);
        assert_eq!(mixed, encode_variant(1, 4));
    }
}

impl super::animal::Animal for HorseEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        matches!(
            item_stack.item.id,
            id if id == Item::GOLDEN_CARROT.id
                || id == Item::GOLDEN_APPLE.id
                || id == Item::ENCHANTED_GOLDEN_APPLE.id
        )
    }
}

impl Mob for HorseEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }
    fn is_tame(&self) -> bool {
        self.tamed.load(Ordering::Relaxed)
    }
    fn get_horse(&self) -> Option<&HorseEntity> {
        Some(self)
    }
    fn can_mate_with(&self, mate: &dyn EntityBase) -> bool {
        super::horse_food::horse_family_offspring_type(
            &EntityType::HORSE,
            mate.get_entity().entity_type,
        )
        .is_some()
    }
    fn breeding_offspring_type(&self, mate: &dyn EntityBase) -> &'static EntityType {
        super::horse_food::horse_family_offspring_type(
            &EntityType::HORSE,
            mate.get_entity().entity_type,
        )
        .unwrap_or(&EntityType::HORSE)
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
            let (Some(mate), Some(child)) = (
                mate.get_mob().and_then(Mob::get_horse),
                child.get_mob().and_then(Mob::get_horse),
            ) else {
                return;
            };
            child.set_variant(inherited_variant(
                self.variant(),
                mate.variant(),
                rand::rng().random_range(0..9),
                rand::rng().random_range(0..9),
            ));
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
                super::horse_food::make_equine_mad(self, Sound::EntityHorseAngry);
                return true;
            }
            if self.is_tame() && !self.is_baby() && stack.item.has_tag(&tag::Item::C_ARMORS_HORSE) {
                let living = &self.mob_entity.living_entity;
                let mut equipment = living.entity_equipment.lock().await;
                if equipment.get(&EquipmentSlot::BODY).is_empty() {
                    let armor = stack.copy_with_count(1);
                    equipment.put(&EquipmentSlot::BODY, armor.clone());
                    drop(equipment);
                    stack.decrement_unless_creative(player.gamemode.load(), 1);
                    living.send_equipment_changes(&[(EquipmentSlot::BODY, armor)]);
                    let entity = self.get_entity();
                    entity.world.load().play_sound(
                        Sound::EntityHorseArmor,
                        pumpkin_data::sound::SoundCategory::Neutral,
                        &entity.pos.load(),
                    );
                    return true;
                }
            }
            if super::horse_food::mount_equine(self, player, stack).await {
                return true;
            }
            self.mob_entity.mob_interact(player, stack).await
        })
    }
}
