use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::{
    entity::EntityType,
    item::Item,
    sound::{Sound, SoundCategory},
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::Metadata;
use pumpkin_util::version::JavaMinecraftVersion;

use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, NBTStorage, NbtFuture,
    ai::goal::{
        active_target::ActiveTargetGoal, creeper_ignite::CreeperIgniteGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal,
        melee_attack::MeleeAttackGoal, revenge::RevengeGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    area_effect_cloud::AreaEffectCloudEntity,
    mob::{Mob, MobEntity},
    player::Player,
};

const DEFAULT_FUSE_TIME: i32 = 30;
const DEFAULT_EXPLOSION_RADIUS: i32 = 3;
const EFFECT_CLOUD_DURATION: i32 = 300;
const EFFECT_CLOUD_RADIUS: f32 = 2.5;
const EFFECT_CLOUD_WAIT_TIME: i32 = 10;
const MAX_DROPPED_SKULLS: i32 = 1;

pub struct CreeperEntity {
    pub mob_entity: MobEntity,
    pub fuse_speed: AtomicI32,
    pub current_fuse_time: AtomicI32,
    pub last_fuse_time: AtomicI32,
    pub fuse_time: AtomicI32,
    pub explosion_radius: AtomicI32,
    pub ignited: AtomicBool,
    pub charged: AtomicBool,
    pub dropped_skulls: AtomicI32,
}

impl CreeperEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let entity = Self {
            mob_entity,
            fuse_speed: AtomicI32::new(-1),
            current_fuse_time: AtomicI32::new(0),
            last_fuse_time: AtomicI32::new(0),
            fuse_time: AtomicI32::new(DEFAULT_FUSE_TIME),
            explosion_radius: AtomicI32::new(DEFAULT_EXPLOSION_RADIUS),
            ignited: AtomicBool::new(false),
            charged: AtomicBool::new(false),
            dropped_skulls: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(entity);
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

            goal_selector.add_goal(1, Box::new(SwimGoal::default()));
            goal_selector.add_goal(2, Box::new(CreeperIgniteGoal::new(mob_arc.clone())));
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.0, false)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(0.8)));

            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(6, Box::new(RandomLookAroundGoal::default()));

            target_selector.add_goal(
                1,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
            target_selector.add_goal(2, Box::new(RevengeGoal::new(true)));
        };

        mob_arc
    }

    pub fn set_fuse_speed(&self, speed: i32) {
        if self.fuse_speed.swap(speed, Ordering::Relaxed) == speed {
            return;
        }
        self.mob_entity.living_entity.entity.send_meta_data(
            &[Metadata::new(
                pumpkin_data::tracked_data::creeper::FUSE_ID,
                VarInt(speed),
            )],
            None,
        );
    }

    async fn explode(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        let radius = self.explosion_radius.load(Ordering::Relaxed) as f32;
        let multiplier = if self.charged.load(Ordering::Relaxed) {
            2.0
        } else {
            1.0
        };
        self.mob_entity
            .living_entity
            .dead
            .store(true, Ordering::Relaxed);
        let world = entity.world.load();
        let pos = entity.pos.load();
        world
            .explode_mob(pos, radius * multiplier, entity.entity_id)
            .await;
        self.spawn_effect_cloud(&world, pos).await;
        entity.remove().await;
    }

    async fn spawn_effect_cloud(
        &self,
        world: &Arc<crate::world::World>,
        pos: pumpkin_util::math::vector3::Vector3<f64>,
    ) {
        let effects = self.mob_entity.living_entity.active_effects.lock().await;
        if effects.is_empty() {
            return;
        }
        let cloud_effects = effects
            .values()
            .map(|effect| {
                (
                    effect.effect_type,
                    effect.duration,
                    effect.amplifier,
                    effect.ambient,
                    effect.show_particles,
                    effect.show_icon,
                )
            })
            .collect();
        drop(effects);

        let cloud_entity = Entity::new(world.clone(), pos, &EntityType::AREA_EFFECT_CLOUD);
        let cloud = AreaEffectCloudEntity::create(
            cloud_entity,
            ItemStack::new(0, &Item::GLASS_BOTTLE),
            cloud_effects,
            EFFECT_CLOUD_DURATION,
            EFFECT_CLOUD_RADIUS,
            20,
            EFFECT_CLOUD_WAIT_TIME,
            -0.5,
            0,
        );
        world.spawn_entity(cloud).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_creeper_effect_cloud_uses_exact_geometry_and_timing() {
        assert_eq!(EFFECT_CLOUD_DURATION, 300);
        assert_eq!(EFFECT_CLOUD_RADIUS, 2.5);
        assert_eq!(EFFECT_CLOUD_WAIT_TIME, 10);
    }

    #[test]
    fn vanilla_charged_creeper_can_create_only_one_mob_head() {
        assert_eq!(MAX_DROPPED_SKULLS, 1);
        assert_eq!(
            pumpkin_data::item_id_remap::remap_item_id_for_version(
                Item::CREEPER_HEAD.id,
                JavaMinecraftVersion::V_1_21_4,
            ),
            1158
        );
    }

    #[test]
    fn creeper_fuse_metadata_uses_varint_payload() {
        let mut bytes = Vec::new();
        Metadata::new(pumpkin_data::tracked_data::creeper::FUSE_ID, VarInt(-1))
            .write(&mut bytes, &JavaMinecraftVersion::V_1_21_4)
            .unwrap();

        // Tracker index 16, INT serializer 1, then Minecraft's five-byte VarInt(-1).
        assert_eq!(bytes, [16, 1, 255, 255, 255, 255, 15]);
    }
}

impl NBTStorage for CreeperEntity {
    fn write_nbt<'a>(&'a self, nbt: &'a mut NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async {
            self.mob_entity.write_nbt(nbt).await;
            nbt.put_bool("powered", self.charged.load(Ordering::Relaxed));
            nbt.put_short("Fuse", self.fuse_time.load(Ordering::Relaxed) as i16);
            nbt.put_byte(
                "ExplosionRadius",
                self.explosion_radius.load(Ordering::Relaxed) as i8,
            );
            nbt.put_bool("ignited", self.ignited.load(Ordering::Relaxed));
        })
    }

    fn read_nbt_non_mut<'a>(&'a self, nbt: &'a NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            if let Some(powered) = nbt.get_bool("powered") {
                self.charged.store(powered, Ordering::Relaxed);
            }
            if let Some(fuse) = nbt.get_short("Fuse") {
                self.fuse_time.store(i32::from(fuse), Ordering::Relaxed);
            }
            if let Some(radius) = nbt.get_byte("ExplosionRadius") {
                self.explosion_radius
                    .store(i32::from(radius), Ordering::Relaxed);
            }
            if let Some(ignited) = nbt.get_bool("ignited") {
                self.ignited.store(ignited, Ordering::Relaxed);
            }
        })
    }
}

impl Mob for CreeperEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_init_data_tracker(&self) -> EntityBaseFuture<'_, ()> {
        Box::pin(async move {
            let entity = &self.mob_entity.living_entity.entity;
            entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::creeper::FUSE_ID,
                    VarInt(self.fuse_speed.load(Ordering::Relaxed)),
                )],
                None,
            );
            entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::creeper::CHARGED,
                    self.charged.load(Ordering::Relaxed),
                )],
                None,
            );
            entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::creeper::IS_IGNITED,
                    self.ignited.load(Ordering::Relaxed),
                )],
                None,
            );
        })
    }

    fn mob_on_death<'a>(&'a self, cause: Option<&'a dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            let Some(killer) =
                cause.and_then(|entity| entity.cast_any().downcast_ref::<CreeperEntity>())
            else {
                return;
            };
            let world = self.mob_entity.living_entity.entity.world.load();
            if !world.level_info.load().game_rules.mob_drops
                || !killer.charged.load(Ordering::Relaxed)
                || killer
                    .dropped_skulls
                    .compare_exchange(0, MAX_DROPPED_SKULLS, Ordering::Relaxed, Ordering::Relaxed)
                    .is_err()
            {
                return;
            }

            world
                .drop_stack(
                    &self.mob_entity.living_entity.entity.block_pos.load(),
                    ItemStack::new(1, &Item::CREEPER_HEAD),
                )
                .await;
        })
    }

    fn mob_java_spawn_metadata(
        &self,
        version: JavaMinecraftVersion,
    ) -> EntityBaseFuture<'_, Option<Box<[u8]>>> {
        Box::pin(async move {
            let mut metadata = Vec::new();
            Metadata::new(
                pumpkin_data::tracked_data::creeper::FUSE_ID,
                VarInt(self.fuse_speed.load(Ordering::Relaxed)),
            )
            .write(&mut metadata, &version)
            .ok()?;
            Metadata::new(
                pumpkin_data::tracked_data::creeper::CHARGED,
                self.charged.load(Ordering::Relaxed),
            )
            .write(&mut metadata, &version)
            .ok()?;
            Metadata::new(
                pumpkin_data::tracked_data::creeper::IS_IGNITED,
                self.ignited.load(Ordering::Relaxed),
            )
            .write(&mut metadata, &version)
            .ok()?;
            metadata.push(255);
            Some(metadata.into_boxed_slice())
        })
    }

    fn mob_on_lightning_strike<'a>(
        &'a self,
        caller: &'a dyn EntityBase,
        lightning: &'a crate::entity::lightning::LightningBoltEntity,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            self.charged.store(true, Ordering::Relaxed);
            self.mob_entity.living_entity.entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::creeper::CHARGED,
                    true,
                )],
                None,
            );
            self.mob_entity
                .living_entity
                .on_lightning_strike(caller, lightning)
                .await;
        })
    }

    fn mob_tick<'a>(&'a self, _caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            let entity = &self.mob_entity.living_entity.entity;
            if !entity.is_alive() {
                return;
            }

            self.last_fuse_time.store(
                self.current_fuse_time.load(Ordering::Relaxed),
                Ordering::Relaxed,
            );

            if self.ignited.load(Ordering::Relaxed) {
                self.set_fuse_speed(1);
            }

            let fuse_speed = self.fuse_speed.load(Ordering::Relaxed);
            let current = self.current_fuse_time.load(Ordering::Relaxed);

            if fuse_speed > 0 && current == 0 {
                let world = entity.world.load();
                world.play_sound_fine(
                    Sound::EntityCreeperPrimed,
                    SoundCategory::Hostile,
                    &entity.pos.load(),
                    1.0,
                    0.5,
                );
            }

            let fuse_time = self.fuse_time.load(Ordering::Relaxed);
            let new_fuse = (current + fuse_speed).max(0);
            self.current_fuse_time.store(new_fuse, Ordering::Relaxed);

            if new_fuse >= fuse_time {
                self.current_fuse_time.store(fuse_time, Ordering::Relaxed);
                self.explode().await;
            }
        })
    }

    fn mob_interact<'a>(
        &'a self,
        player: &'a Arc<Player>,
        item_stack: &'a mut ItemStack,
    ) -> EntityBaseFuture<'a, bool> {
        Box::pin(async move {
            if item_stack.item.id != Item::FLINT_AND_STEEL.id {
                return self.mob_entity.mob_interact(player, item_stack).await;
            }

            let entity = &self.mob_entity.living_entity.entity;
            let world = entity.world.load();
            let pos = entity.pos.load();

            world.play_sound_fine(
                Sound::ItemFlintandsteelUse,
                SoundCategory::Hostile,
                &pos,
                1.0,
                rand::random::<f32>() * 0.4 + 0.8,
            );

            self.ignited.store(true, Ordering::Relaxed);
            entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::creeper::IS_IGNITED,
                    true,
                )],
                None,
            );

            if player.gamemode.load() != pumpkin_util::GameMode::Creative {
                let _ = item_stack.damage_item(1);
            }

            player
                .increment_stat(
                    crate::entity::player::statistics::StatisticCategory::Used,
                    Item::FLINT_AND_STEEL.id as i32,
                    1,
                )
                .await;

            true
        })
    }
}
