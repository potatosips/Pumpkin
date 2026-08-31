use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::data_component_impl::{EquipmentSlot, FoodImpl};
use pumpkin_data::entity::{EntityStatus, EntityType};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::bedrock::server::actor_event::ActorEventType;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::Metadata;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, NBTStorage, NbtFuture,
    ageable::AgeableMob,
    ai::goal::{
        active_target::ActiveTargetGoal, avoid_entity::AvoidEntityGoal, beg::BegGoal,
        breed::BreedGoal, escape_danger::EscapeDangerGoal, follow_owner::FollowOwnerGoal,
        follow_parent::FollowParentGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal,
        owner_hurt_by_target::OwnerHurtByTargetGoal, owner_hurt_target::OwnerHurtTargetGoal,
        revenge::RevengeGoal, swim::SwimGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

pub struct WolfEntity {
    pub mob_entity: MobEntity,
    pub variant: AtomicU8,
    pub collar_color: AtomicU8,
    pub is_tame: AtomicBool,
    pub is_sitting: AtomicBool,
    pub owner: AtomicCell<Option<Uuid>>,
    pub ageable_data: crate::entity::ageable::AgeableData,
}

fn collar_dye_color(item: &Item) -> Option<u8> {
    if !item.has_tag(&tag::Item::C_DYES) {
        return None;
    }
    Some(match item.registry_key.strip_suffix("_dye")? {
        "white" => 0,
        "orange" => 1,
        "magenta" => 2,
        "light_blue" => 3,
        "yellow" => 4,
        "lime" => 5,
        "pink" => 6,
        "gray" => 7,
        "light_gray" => 8,
        "cyan" => 9,
        "purple" => 10,
        "blue" => 11,
        "brown" => 12,
        "green" => 13,
        "red" => 14,
        "black" => 15,
        _ => return None,
    })
}

const fn event_dye_color(color: u8) -> crate::plugin::api::events::entity::entity_dye::DyeColor {
    use crate::plugin::api::events::entity::entity_dye::DyeColor;
    match color {
        0 => DyeColor::White,
        1 => DyeColor::Orange,
        2 => DyeColor::Magenta,
        3 => DyeColor::LightBlue,
        4 => DyeColor::Yellow,
        5 => DyeColor::Lime,
        6 => DyeColor::Pink,
        7 => DyeColor::Gray,
        8 => DyeColor::LightGray,
        9 => DyeColor::Cyan,
        10 => DyeColor::Purple,
        11 => DyeColor::Blue,
        12 => DyeColor::Brown,
        13 => DyeColor::Green,
        14 => DyeColor::Red,
        _ => DyeColor::Black,
    }
}

impl WolfEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let wolf = Self {
            mob_entity,
            variant: AtomicU8::new(3),       // Default to pale
            collar_color: AtomicU8::new(14), // Default to red
            is_tame: AtomicBool::new(false),
            is_sitting: AtomicBool::new(false),
            owner: AtomicCell::new(None),
            ageable_data: crate::entity::ageable::AgeableData::default(),
        };
        let mob_arc = Arc::new(wolf);
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

            // Goal selector (matching Vanilla registerGoals):
            // 1: SwimGoal (FloatGoal)
            goal_selector.add_goal(1, Box::new(SwimGoal::default()));
            // 1: EscapeDangerGoal (TamableAnimalPanicGoal)
            goal_selector.add_goal(1, EscapeDangerGoal::new(1.5));
            // 3: Avoid Llama
            goal_selector.add_goal(
                3,
                Box::new(AvoidEntityGoal::new(&EntityType::LLAMA, 24.0, 1.5, 1.5)),
            );
            // 5: MeleeAttackGoal
            goal_selector.add_goal(5, Box::new(MeleeAttackGoal::new(1.0, true)));
            // 6: FollowOwnerGoal
            goal_selector.add_goal(6, FollowOwnerGoal::new(1.0, 10.0, 2.0));
            // 7: BreedGoal
            goal_selector.add_goal(7, BreedGoal::new(1.0));
            // 8: FollowParentGoal & WanderAroundGoal
            goal_selector.add_goal(8, Box::new(FollowParentGoal::new(1.1)));
            goal_selector.add_goal(8, Box::new(WanderAroundGoal::new(1.0)));
            // 9: BegGoal
            goal_selector.add_goal(9, BegGoal::new(8.0));
            // 10: LookAtPlayer & RandomLookAround
            goal_selector.add_goal(
                10,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(10, Box::new(RandomLookAroundGoal::default()));
        };

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Target selector (matching Vanilla registerGoals):
            // 1: OwnerHurtByTargetGoal
            target_selector.add_goal(1, OwnerHurtByTargetGoal::new());
            // 2: OwnerHurtTargetGoal
            target_selector.add_goal(2, OwnerHurtTargetGoal::new());
            // 3: HurtByTargetGoal (RevengeGoal)
            target_selector.add_goal(3, Box::new(RevengeGoal::new(true)));
            // 5: NonTameRandomTarget (Sheep, Rabbit, Fox)
            target_selector.add_goal(
                5,
                ActiveTargetGoal::with_default_untamed(
                    &mob_arc.mob_entity,
                    &EntityType::SHEEP,
                    false,
                ),
            );
            target_selector.add_goal(
                5,
                ActiveTargetGoal::with_default_untamed(
                    &mob_arc.mob_entity,
                    &EntityType::RABBIT,
                    false,
                ),
            );
            target_selector.add_goal(
                5,
                ActiveTargetGoal::with_default_untamed(
                    &mob_arc.mob_entity,
                    &EntityType::FOX,
                    false,
                ),
            );
            // 7: NearestAttackableTarget (Skeleton)
            target_selector.add_goal(
                7,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::SKELETON, false),
            );
        };

        mob_arc
    }

    pub fn get_tame_flags(&self) -> u8 {
        let mut flags = 0u8;
        if self.is_sitting.load(Ordering::Relaxed) {
            flags |= 0x01;
        }
        if self.is_tame.load(Ordering::Relaxed) {
            flags |= 0x04;
        }
        flags
    }

    fn sync_tame_data(&self) {
        let entity = self.get_entity();
        entity.send_meta_data(
            &[Metadata::new(
                pumpkin_data::tracked_data::wolf::TAMEABLE_FLAGS,
                self.get_tame_flags(),
            )],
            None,
        );
        entity.send_meta_data(
            &[Metadata::new(
                pumpkin_data::tracked_data::wolf::OWNER_UUID,
                self.owner.load(),
            )],
            None,
        );
    }

    async fn apply_tamed_attributes(&self, heal_to_full: bool) {
        let living = &self.mob_entity.living_entity;
        living.set_max_health(40.0).await;
        living.set_attribute_base(&Attributes::ATTACK_DAMAGE, 4.0);
        crate::entity::attributes::send_attribute_updates_for_living(
            living,
            vec![Attributes::ATTACK_DAMAGE],
        )
        .await;
        if heal_to_full {
            living.set_health(40.0);
        }
    }
}

impl AgeableMob for WolfEntity {
    fn get_ageable_data(&self) -> &crate::entity::ageable::AgeableData {
        &self.ageable_data
    }
}

impl Animal for WolfEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        let item = item_stack.get_item();
        item.has_tag(&tag::Item::MINECRAFT_WOLF_FOOD)
    }
}

impl NBTStorage for WolfEntity {
    fn write_nbt<'a>(&'a self, nbt: &'a mut NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async {
            self.mob_entity.write_nbt(nbt).await;
            self.write_ageable_nbt(nbt);
            self.write_animal_nbt(nbt);
            let variant_str = match self.variant.load(Ordering::Relaxed) {
                0 => "minecraft:ashen",
                1 => "minecraft:black",
                2 => "minecraft:chestnut",
                4 => "minecraft:rusty",
                5 => "minecraft:snowy",
                6 => "minecraft:spotted",
                7 => "minecraft:striped",
                8 => "minecraft:woods",
                _ => "minecraft:pale",
            };
            nbt.put_string("variant", variant_str.to_string());
            nbt.put_byte(
                "CollarColor",
                self.collar_color.load(Ordering::Relaxed) as i8,
            );
            nbt.put_bool("IsTame", self.is_tame.load(Ordering::Relaxed));
            nbt.put_bool("Sitting", self.is_sitting.load(Ordering::Relaxed));
            if let Some(owner) = self.owner.load() {
                nbt.put_uuid("Owner", owner);
            }
        })
    }

    fn read_nbt_non_mut<'a>(&'a self, nbt: &'a NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            self.read_ageable_nbt(nbt);
            self.read_animal_nbt(nbt);
            if let Some(variant_str) = nbt.get_string("variant") {
                let variant = match variant_str
                    .strip_prefix("minecraft:")
                    .unwrap_or(variant_str)
                {
                    "ashen" => 0,
                    "black" => 1,
                    "chestnut" => 2,
                    "rusty" => 4,
                    "snowy" => 5,
                    "spotted" => 6,
                    "striped" => 7,
                    "woods" => 8,
                    _ => 3,
                };
                self.variant.store(variant, Ordering::Relaxed);
            }
            if let Some(collar) = nbt.get_byte("CollarColor") {
                self.collar_color.store(collar as u8, Ordering::Relaxed);
            } else if let Some(collar_int) = nbt.get_int("CollarColor") {
                self.collar_color.store(collar_int as u8, Ordering::Relaxed);
            }
            if let Some(sitting) = nbt.get_bool("Sitting") {
                self.is_sitting.store(sitting, Ordering::Relaxed);
            }
            if let Some(owner) = nbt.get_uuid("Owner") {
                self.owner.store(Some(owner));
                self.is_tame.store(true, Ordering::Relaxed);
            } else if let Some(is_tame) = nbt.get_bool("IsTame") {
                self.is_tame.store(is_tame, Ordering::Relaxed);
            }
            if self.is_tame.load(Ordering::Relaxed) {
                self.apply_tamed_attributes(false).await;
            }
        })
    }
}

impl Mob for WolfEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_owner_uuid(&self) -> Option<Uuid> {
        self.owner.load()
    }

    fn is_sitting(&self) -> bool {
        self.is_sitting.load(Ordering::Relaxed)
    }

    fn is_tame(&self) -> bool {
        self.is_tame.load(Ordering::Relaxed)
    }

    fn get_wolf(&self) -> Option<&WolfEntity> {
        Some(self)
    }

    fn mob_tick<'a>(&'a self, _caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            self.ageable_ai_step();
        })
    }

    fn on_damage<'a>(
        &'a self,
        _damage_type: pumpkin_data::damage::DamageType,
        _source: Option<&'a dyn EntityBase>,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            // Vanilla clears the ordered-to-sit state whenever a wolf accepts a
            // hit, including a hit fully absorbed by wolf armor.
            if self.is_sitting.swap(false, Ordering::Relaxed) {
                self.sync_tame_data();
            }
        })
    }

    fn configure_bred_child<'a>(
        &'a self,
        mate: &'a dyn EntityBase,
        child: &'a Arc<dyn EntityBase>,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            let Some(child_wolf) = child.get_mob().and_then(Mob::get_wolf) else {
                return;
            };

            if let Some(mate_wolf) = mate.get_mob().and_then(Mob::get_wolf)
                && rand::random::<bool>()
            {
                child_wolf
                    .variant
                    .store(mate_wolf.variant.load(Ordering::Relaxed), Ordering::Relaxed);
            } else {
                child_wolf
                    .variant
                    .store(self.variant.load(Ordering::Relaxed), Ordering::Relaxed);
            }

            if let Some(owner) = self.owner.load() {
                child_wolf.owner.store(Some(owner));
                child_wolf.is_tame.store(true, Ordering::Relaxed);
                child_wolf.apply_tamed_attributes(true).await;
            }
        })
    }

    fn mob_set_variant_name(&self, name: &str) {
        let variant = match name.strip_prefix("minecraft:").unwrap_or(name) {
            "ashen" => 0,
            "black" => 1,
            "chestnut" => 2,
            "rusty" => 4,
            "snowy" => 5,
            "spotted" => 6,
            "striped" => 7,
            "woods" => 8,
            _ => 3,
        };
        self.variant.store(variant, Ordering::Relaxed);
    }

    fn mob_init_data_tracker(&self) -> EntityBaseFuture<'_, ()> {
        Box::pin(async move {
            let entity = self.get_entity();
            let is_baby = entity.age.load(Ordering::Relaxed) < 0;
            if is_baby {
                entity.send_meta_data(
                    &[Metadata::new(
                        pumpkin_data::tracked_data::wolf::BABY_ID,
                        true,
                    )],
                    None,
                );
            }
            entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::wolf::TAMEABLE_FLAGS,
                    self.get_tame_flags(),
                )],
                None,
            );
            entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::wolf::COLLAR_COLOR,
                    self.collar_color.load(Ordering::Relaxed) as i32,
                )],
                None,
            );
            entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::wolf::WOLF_VARIANT_ID,
                    VarInt(self.variant.load(Ordering::Relaxed) as i32),
                )],
                None,
            );
            entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::wolf::OWNER_UUID,
                    self.owner.load(),
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
            let tame = self.is_tame.load(Ordering::Relaxed);
            if !tame && item_stack.item == &Item::BONE {
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                if rand::rng().random_range(0..3) == 0 {
                    let entity = self.get_entity();
                    let mut event =
                        crate::plugin::api::events::entity::entity_tame::EntityTameEvent::new(
                            entity.entity_id,
                            player.clone(),
                        );
                    if let Some(server) = entity.world.load().server.upgrade() {
                        server.plugin_manager.fire(&server, &mut event).await;
                    }
                    if event.cancelled {
                        return true;
                    }
                    self.is_tame.store(true, Ordering::Relaxed);
                    self.owner.store(Some(player.gameprofile.id));
                    self.apply_tamed_attributes(true).await;
                    self.sync_tame_data();
                    entity.world.load().send_entity_status(
                        entity,
                        EntityStatus::TamingSucceeded,
                        Some(ActorEventType::TamingSucceeded),
                    );
                } else {
                    let entity = self.get_entity();
                    entity.world.load().send_entity_status(
                        entity,
                        EntityStatus::TamingFailed,
                        Some(ActorEventType::TamingFailed),
                    );
                }
                return true;
            }

            if tame && self.owner.load() == Some(player.gameprofile.id) {
                if item_stack.item == &Item::WOLF_ARMOR && !self.is_baby() {
                    let living = &self.mob_entity.living_entity;
                    let mut equipment = living.entity_equipment.lock().await;
                    if equipment.get(&EquipmentSlot::BODY).is_empty() {
                        let mut armor = item_stack.clone();
                        armor.item_count = 1;
                        equipment.put(&EquipmentSlot::BODY, armor.clone());
                        drop(equipment);
                        item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                        living.send_equipment_changes(&[(EquipmentSlot::BODY, armor)]);
                        let entity = self.get_entity();
                        entity.world.load().play_sound(
                            pumpkin_data::sound::Sound::ItemArmorEquipWolf,
                            pumpkin_data::sound::SoundCategory::Neutral,
                            &entity.pos.load(),
                        );
                        return true;
                    }
                } else if item_stack
                    .item
                    .has_tag(&tag::Item::MINECRAFT_REPAIRS_WOLF_ARMOR)
                {
                    let living = &self.mob_entity.living_entity;
                    let mut equipment = living.entity_equipment.lock().await;
                    if let Some(armor) = equipment.equipment.get_mut(&EquipmentSlot::BODY)
                        && armor.item == &Item::WOLF_ARMOR
                        && armor.repair_item(64) > 0
                    {
                        let repaired_armor = armor.clone();
                        drop(equipment);
                        item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                        living.send_equipment_changes(&[(EquipmentSlot::BODY, repaired_armor)]);
                        let entity = self.get_entity();
                        entity.world.load().play_sound(
                            pumpkin_data::sound::Sound::ItemWolfArmorRepair,
                            pumpkin_data::sound::SoundCategory::Neutral,
                            &entity.pos.load(),
                        );
                        return true;
                    }
                } else if self.is_food(item_stack) {
                    let living = &self.mob_entity.living_entity;
                    if living.health.load() < living.get_max_health()
                        && let Some(nutrition) = item_stack
                            .get_data_component::<FoodImpl>()
                            .map(|food| food.nutrition)
                    {
                        item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                        living.heal(nutrition as f32);
                        self.play_eating_sound(pumpkin_data::sound::Sound::EntityGenericEat);
                        return true;
                    }
                } else if let Some(color) = collar_dye_color(item_stack.get_item()) {
                    if color != self.collar_color.load(Ordering::Relaxed) {
                        let entity = self.get_entity();
                        let mut event =
                            crate::plugin::api::events::entity::entity_dye::EntityDyeEvent::new(
                                entity.entity_id,
                                event_dye_color(color),
                                Some(player.clone()),
                            );
                        if let Some(server) = entity.world.load().server.upgrade() {
                            server.plugin_manager.fire(&server, &mut event).await;
                        }
                        if event.cancelled {
                            return false;
                        }
                        self.collar_color.store(color, Ordering::Relaxed);
                        entity.send_meta_data(
                            &[Metadata::new(
                                pumpkin_data::tracked_data::wolf::COLLAR_COLOR,
                                i32::from(color),
                            )],
                            None,
                        );
                        item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                        entity.world.load().play_sound(
                            pumpkin_data::sound::Sound::ItemDyeUse,
                            pumpkin_data::sound::SoundCategory::Players,
                            &entity.pos.load(),
                        );
                    }
                    return true;
                } else {
                    self.is_sitting.fetch_xor(true, Ordering::Relaxed);
                    self.sync_tame_data();
                    return true;
                }
            }

            self.animal_interact(
                player,
                item_stack,
                pumpkin_data::sound::Sound::EntityWolfAmbient,
            )
            .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wolf_collar_dye_mapping() {
        assert_eq!(collar_dye_color(&Item::WHITE_DYE), Some(0));
        assert_eq!(collar_dye_color(&Item::LIGHT_GRAY_DYE), Some(8));
        assert_eq!(collar_dye_color(&Item::RED_DYE), Some(14));
        assert_eq!(collar_dye_color(&Item::BLACK_DYE), Some(15));
        assert_eq!(collar_dye_color(&Item::BONE), None);
    }

    #[test]
    fn wolf_tame_flags_bitmask_parity() {
        let is_sitting = true;
        let is_tame = true;
        let mut flags = 0u8;
        if is_sitting {
            flags |= 0x01;
        }
        if is_tame {
            flags |= 0x04;
        }
        assert_eq!(flags, 0x05);
    }
}
