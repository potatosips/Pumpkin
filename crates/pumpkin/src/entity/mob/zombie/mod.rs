use super::{Mob, MobEntity};
use crate::entity::ai::goal::destroy_egg::DestroyEggGoal;
use crate::entity::ai::goal::look_around::RandomLookAroundGoal;
use crate::entity::ai::goal::revenge::RevengeGoal;
use crate::entity::ai::goal::swim::SwimGoal;
use crate::entity::ai::goal::wander_around::WanderAroundGoal;
use crate::entity::ai::goal::zombie_attack::ZombieAttackGoal;
use crate::entity::{
    Entity, NBTStorage, NbtFuture,
    ai::goal::{active_target::ActiveTargetGoal, look_at_entity::LookAtEntityGoal},
};
use pumpkin_data::entity::EntityType;
use pumpkin_data::{tag, tag::Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::java::client::play::CWorldEvent;
use pumpkin_util::math::position::BlockPos;
use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, Ordering},
};

pub mod drowned;
pub mod husk;
#[allow(clippy::module_inception)]
pub mod zombie;
pub mod zombie_villager;

pub struct ZombieEntityBase {
    pub mob_entity: MobEntity,
    in_water_time: AtomicI32,
    conversion_time: AtomicI32,
}

impl ZombieEntityBase {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let zombie = Self {
            mob_entity,
            in_water_time: AtomicI32::new(-1),
            conversion_time: AtomicI32::new(-1),
        };
        let mob_arc = Arc::new(zombie);
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
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(2, ZombieAttackGoal::new(1.0, false));
            goal_selector.add_goal(4, DestroyEggGoal::new(1.0, 3));
            goal_selector.add_goal(7, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));

            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::VILLAGER, true),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::IRON_GOLEM, true),
            );
            target_selector.add_goal(
                5,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::TURTLE, true),
            );
        };

        mob_arc
    }

    pub async fn tick_underwater_conversion(
        &self,
        caller: &Arc<dyn crate::entity::EntityBase>,
        target_type: &'static EntityType,
    ) -> bool {
        let entity = &self.mob_entity.living_entity.entity;
        let eye = entity.get_eye_pos();
        let eye_pos = BlockPos::floored(eye.x, eye.y, eye.z);
        let eye_submerged_by_block = entity
            .world
            .load()
            .get_fluid(&eye_pos)
            .has_tag(&tag::Fluid::MINECRAFT_WATER);
        // Fluid collision already computes the water surface relative to the
        // entity's feet. Use it as well so conversion is not lost while a mob
        // is moving across a chunk/block boundary between fluid-state updates.
        let eye_submerged = eye_submerged_by_block
            || (entity.touching_water.load(Ordering::Relaxed)
                && entity.water_height.load()
                    > f64::from(entity.entity_dimension.load().eye_height));
        if !eye_submerged {
            self.in_water_time.store(-1, Ordering::Relaxed);
            self.conversion_time.store(-1, Ordering::Relaxed);
            return false;
        }

        let submerged_ticks = self.in_water_time.fetch_add(1, Ordering::Relaxed) + 1;
        if submerged_ticks < 600 {
            return false;
        }

        let remaining = self.conversion_time.load(Ordering::Relaxed);
        if remaining < 0 {
            self.conversion_time.store(300, Ordering::Relaxed);
            return false;
        }
        if remaining > 1 {
            self.conversion_time.fetch_sub(1, Ordering::Relaxed);
            return false;
        }

        let mut nbt = NbtCompound::new();
        self.mob_entity.write_nbt(&mut nbt).await;
        let equipment = self
            .mob_entity
            .living_entity
            .entity_equipment
            .lock()
            .await
            .clone();
        let source = caller.get_entity();
        let world = source.world.load();
        let replacement = crate::entity::r#type::from_type(
            target_type,
            source.pos.load(),
            &world,
            uuid::Uuid::new_v4(),
        );
        if let Some(living) = replacement.get_living_entity() {
            living.read_nbt_non_mut(&nbt).await;
            *living.entity_equipment.lock().await = equipment.clone();
        }
        replacement
            .get_entity()
            .age
            .store(source.age.load(Ordering::Relaxed), Ordering::Relaxed);
        let replacement_id = replacement.get_entity().entity_id;
        world.spawn_entity(replacement.clone()).await;
        if world.get_entity_by_id(replacement_id).is_none() {
            return false;
        }
        if let Some(living) = replacement.get_living_entity() {
            let changes = equipment.equipment.into_iter().collect::<Vec<_>>();
            if !changes.is_empty() {
                living.send_equipment_changes(&changes);
            }
        }

        let event_id = if target_type.id == EntityType::DROWNED.id {
            1040
        } else {
            1041
        };
        world.broadcast_to_chunk(
            source.chunk_pos.load(),
            &CWorldEvent::new(event_id, source.block_pos.load(), 0, false),
        );
        source.remove().await;
        true
    }
}

impl NBTStorage for ZombieEntityBase {
    fn write_nbt<'a>(&'a self, nbt: &'a mut NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.write_nbt(nbt).await;
            nbt.put_int("InWaterTime", self.in_water_time.load(Ordering::Relaxed));
            nbt.put_int(
                "DrownedConversionTime",
                self.conversion_time.load(Ordering::Relaxed),
            );
        })
    }

    fn read_nbt_non_mut<'a>(&'a self, nbt: &'a NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            if let Some(in_water_time) = nbt.get_int("InWaterTime") {
                self.in_water_time.store(in_water_time, Ordering::Relaxed);
            }
            if let Some(conversion_time) = nbt.get_int("DrownedConversionTime") {
                self.conversion_time
                    .store(conversion_time, Ordering::Relaxed);
            }
        })
    }
}

impl Mob for ZombieEntityBase {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }
}
