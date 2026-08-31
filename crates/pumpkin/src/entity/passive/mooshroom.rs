use std::borrow::Cow;
use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

use pumpkin_data::data_component_impl::{SuspiciousStewEffect, SuspiciousStewEffectsImpl};
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::meta_data_type::MetaDataType;
use pumpkin_data::sound::Sound;
use pumpkin_data::tracked_data;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::Metadata;
use pumpkin_util::version::JavaMinecraftVersion;
use tokio::sync::Mutex;
use uuid::Uuid;

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

const TEMPT_ITEMS: &[&Item] = &[&Item::WHEAT];

const fn offspring_is_brown(
    first_brown: bool,
    second_brown: bool,
    choose_second: bool,
    mutate: bool,
) -> bool {
    if first_brown == second_brown {
        first_brown != mutate
    } else if choose_second {
        second_brown
    } else {
        first_brown
    }
}

/// Represents a Mooshroom, a fungal variant of cows that can be milked for mushroom stew.
///
/// Wiki: <https://minecraft.wiki/w/Mooshroom>
pub struct MooshroomEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    brown: AtomicBool,
    stew_effect: Mutex<Option<SuspiciousStewEffect>>,
    last_lightning_bolt: Mutex<Option<Uuid>>,
}

impl MooshroomEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let mooshroom = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            brown: AtomicBool::new(false),
            stew_effect: Mutex::new(None),
            last_lightning_bolt: Mutex::new(None),
        };
        let mob_arc = Arc::new(mooshroom);
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
            goal_selector.add_goal(1, EscapeDangerGoal::new(2.0));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.25, TEMPT_ITEMS)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.25)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    pub fn is_brown(&self) -> bool {
        self.brown.load(Ordering::Relaxed)
    }

    fn flower_effect(item: &Item) -> Option<SuspiciousStewEffect> {
        let (effect, duration) = match item.id {
            id if id == Item::DANDELION.id || id == Item::BLUE_ORCHID.id => {
                ("minecraft:saturation", 7)
            }
            id if id == Item::POPPY.id || id == Item::TORCHFLOWER.id => {
                ("minecraft:night_vision", 100)
            }
            id if id == Item::ALLIUM.id => ("minecraft:fire_resistance", 80),
            id if id == Item::AZURE_BLUET.id => ("minecraft:blindness", 160),
            id if id == Item::RED_TULIP.id
                || id == Item::ORANGE_TULIP.id
                || id == Item::WHITE_TULIP.id
                || id == Item::PINK_TULIP.id =>
            {
                ("minecraft:weakness", 180)
            }
            id if id == Item::OXEYE_DAISY.id => ("minecraft:regeneration", 160),
            id if id == Item::CORNFLOWER.id => ("minecraft:jump_boost", 120),
            id if id == Item::LILY_OF_THE_VALLEY.id => ("minecraft:poison", 240),
            id if id == Item::WITHER_ROSE.id => ("minecraft:wither", 160),
            _ => return None,
        };
        Some(SuspiciousStewEffect {
            effect: Cow::Borrowed(effect),
            duration,
        })
    }
}

impl AgeableMob for MooshroomEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for MooshroomEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.id == Item::WHEAT.id
    }
}

impl NBTStorage for MooshroomEntity {
    fn write_nbt<'a>(&'a self, nbt: &'a mut NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.write_nbt(nbt).await;
            self.write_ageable_nbt(nbt);
            self.write_animal_nbt(nbt);
            nbt.put_string(
                "Type",
                if self.is_brown() { "brown" } else { "red" }.to_string(),
            );
            if let Some(effect) = self.stew_effect.lock().await.as_ref() {
                nbt.put_string("stew_effect", effect.effect.to_string());
                nbt.put_int("stew_effect_duration", effect.duration);
            }
        })
    }

    fn read_nbt_non_mut<'a>(&'a self, nbt: &'a NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            self.read_ageable_nbt(nbt);
            self.read_animal_nbt(nbt);
            if let Some(kind) = nbt.get_string("Type") {
                self.brown.store(kind == "brown", Ordering::Relaxed);
            }
            if let Some(effect) = nbt.get_string("stew_effect") {
                *self.stew_effect.lock().await = Some(SuspiciousStewEffect {
                    effect: Cow::Owned(effect.to_owned()),
                    duration: nbt.get_int("stew_effect_duration").unwrap_or(160),
                });
            }
        })
    }
}

impl Mob for MooshroomEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_mooshroom(&self) -> Option<&MooshroomEntity> {
        Some(self)
    }

    fn mob_tick<'a>(&'a self, _caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            self.ageable_ai_step();
        })
    }

    fn configure_bred_child<'a>(
        &'a self,
        mate: &'a dyn EntityBase,
        child: &'a Arc<dyn EntityBase>,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            let Some(mate) = mate.get_mob().and_then(Mob::get_mooshroom) else {
                return;
            };
            let Some(child) = child.get_mob().and_then(Mob::get_mooshroom) else {
                return;
            };
            let brown = offspring_is_brown(
                self.is_brown(),
                mate.is_brown(),
                rand::random::<bool>(),
                rand::random_range(0..1024) == 0,
            );
            child.brown.store(brown, Ordering::Relaxed);
        })
    }

    fn mob_on_lightning_strike<'a>(
        &'a self,
        caller: &'a dyn crate::entity::EntityBase,
        lightning: &'a crate::entity::lightning::LightningBoltEntity,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            let bolt_uuid = lightning.get_entity().entity_uuid;
            let mut last = self.last_lightning_bolt.lock().await;
            if last.as_ref() != Some(&bolt_uuid) {
                *last = Some(bolt_uuid);
                let brown = !self.is_brown();
                self.brown.store(brown, Ordering::Relaxed);
                self.mob_entity.living_entity.entity.send_meta_data(
                    &[Metadata::new_raw(
                        tracked_data::mooshroom::DATA_TYPE.id,
                        MetaDataType::STRING,
                        if brown { "brown" } else { "red" }.to_string(),
                    )],
                    None,
                );
            }
            drop(last);
            self.mob_entity
                .living_entity
                .on_lightning_strike(caller, lightning)
                .await;
        })
    }

    fn mob_java_spawn_metadata(
        &self,
        version: JavaMinecraftVersion,
    ) -> EntityBaseFuture<'_, Option<Box<[u8]>>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            if version == JavaMinecraftVersion::V_1_21_4 {
                Metadata::new_raw(
                    tracked_data::mooshroom::DATA_TYPE.id,
                    MetaDataType::STRING,
                    if self.is_brown() {
                        "brown".to_string()
                    } else {
                        "red".to_string()
                    },
                )
                .write(&mut bytes, &version)
                .ok()?;
            } else {
                Metadata::new(
                    tracked_data::mooshroom::DATA_TYPE,
                    VarInt(i32::from(self.is_brown())),
                )
                .write(&mut bytes, &version)
                .ok()?;
            }
            bytes.push(255);
            Some(bytes.into_boxed_slice())
        })
    }

    fn mob_interact<'a>(
        &'a self,
        player: &'a Arc<Player>,
        item_stack: &'a mut ItemStack,
    ) -> EntityBaseFuture<'a, bool> {
        Box::pin(async move {
            if self.is_brown()
                && let Some(effect) = Self::flower_effect(item_stack.item)
            {
                let mut stored = self.stew_effect.lock().await;
                if stored.is_none() {
                    *stored = Some(effect);
                    item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                    let entity = &self.mob_entity.living_entity.entity;
                    entity.world.load().play_sound(
                        Sound::EntityMooshroomEat,
                        pumpkin_data::sound::SoundCategory::Neutral,
                        &entity.pos.load(),
                    );
                }
                return true;
            }
            if !self.is_baby() && item_stack.item == &Item::BUCKET {
                let entity = &self.mob_entity.living_entity.entity;
                entity.world.load().play_sound(
                    Sound::EntityCowMilk,
                    pumpkin_data::sound::SoundCategory::Neutral,
                    &entity.pos.load(),
                );
                super::cow::exchange_empty_container(player, item_stack, &Item::MILK_BUCKET).await;
                return true;
            }
            if !self.is_baby() && item_stack.item == &Item::BOWL {
                let entity = &self.mob_entity.living_entity.entity;
                let effect = self.stew_effect.lock().await.take();
                entity.world.load().play_sound(
                    if effect.is_some() {
                        Sound::EntityMooshroomSuspiciousMilk
                    } else {
                        Sound::EntityMooshroomMilk
                    },
                    pumpkin_data::sound::SoundCategory::Neutral,
                    &entity.pos.load(),
                );
                if let Some(effect) = effect {
                    let mut stew = ItemStack::new(1, &Item::SUSPICIOUS_STEW);
                    stew.set_data_component(SuspiciousStewEffectsImpl {
                        effects: Cow::Owned(vec![effect]),
                    });
                    super::cow::exchange_empty_container_stack(player, item_stack, stew).await;
                } else {
                    super::cow::exchange_empty_container(player, item_stack, &Item::MUSHROOM_STEW)
                        .await;
                }
                return true;
            }
            self.animal_interact(player, item_stack, Sound::EntityCowAmbient)
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::offspring_is_brown;

    #[test]
    fn vanilla_mooshroom_variant_inheritance() {
        assert!(!offspring_is_brown(false, false, false, false));
        assert!(offspring_is_brown(false, false, false, true));
        assert!(offspring_is_brown(true, true, false, false));
        assert!(!offspring_is_brown(true, true, false, true));
        assert!(!offspring_is_brown(false, true, false, false));
        assert!(offspring_is_brown(false, true, true, false));
    }
}
