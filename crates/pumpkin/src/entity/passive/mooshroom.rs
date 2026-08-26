use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

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

use crate::entity::{
    Entity, EntityBaseFuture, NBTStorage, NbtFuture,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

/// Represents a Mooshroom, a fungal variant of cows that can be milked for mushroom stew.
///
/// Wiki: <https://minecraft.wiki/w/Mooshroom>
pub struct MooshroomEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    brown: AtomicBool,
}

impl MooshroomEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let mooshroom = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            brown: AtomicBool::new(false),
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
            goal_selector.add_goal(1, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                2,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(3, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    pub fn is_brown(&self) -> bool {
        self.brown.load(Ordering::Relaxed)
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
        self.animal_interact(player, item_stack, Sound::EntityCowAmbient)
    }
}
