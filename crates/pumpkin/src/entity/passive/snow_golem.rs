use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

use pumpkin_data::Block;
use pumpkin_data::entity::EntityType;
use pumpkin_data::tracked_data;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::java::client::play::Metadata;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::version::JavaMinecraftVersion;
use pumpkin_world::world::BlockFlags;

use crate::entity::{
    Entity, EntityBaseFuture, NBTStorage, NbtFuture,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
};

pub struct SnowGolemEntity {
    pub mob_entity: MobEntity,
    has_pumpkin: AtomicBool,
}

impl SnowGolemEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let snow_golem = Self {
            mob_entity,
            has_pumpkin: AtomicBool::new(true),
        };
        let mob_arc = Arc::new(snow_golem);
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

            goal_selector.add_goal(
                1,
                Box::new(
                    crate::entity::ai::goal::snowball_attack::SnowballAttackGoal::new(
                        1.25, 20, 10.0,
                    ),
                ),
            );
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(6, Box::new(RandomLookAroundGoal::default()));

            target_selector.add_goal(
                1,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::ZOMBIE, true),
            );
            target_selector.add_goal(
                1,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::DROWNED, true),
            );
            target_selector.add_goal(
                1,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::HUSK, true),
            );
            target_selector.add_goal(
                1,
                ActiveTargetGoal::with_default(
                    &mob_arc.mob_entity,
                    &EntityType::ZOMBIE_VILLAGER,
                    true,
                ),
            );
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::SKELETON, true),
            );
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::STRAY, true),
            );
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(
                    &mob_arc.mob_entity,
                    &EntityType::WITHER_SKELETON,
                    true,
                ),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::SPIDER, true),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::CAVE_SPIDER, true),
            );
            target_selector.add_goal(
                4,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::SLIME, true),
            );
            target_selector.add_goal(
                4,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::MAGMA_CUBE, true),
            );
            target_selector.add_goal(
                5,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::BLAZE, true),
            );
            target_selector.add_goal(
                5,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::WITCH, true),
            );
            target_selector.add_goal(
                6,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PILLAGER, true),
            );
            target_selector.add_goal(
                6,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::VINDICATOR, true),
            );
            target_selector.add_goal(
                6,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::RAVAGER, true),
            );
            target_selector.add_goal(
                6,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::EVOKER, true),
            );
            target_selector.add_goal(
                7,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::ENDERMITE, true),
            );
            target_selector.add_goal(
                7,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::SILVERFISH, true),
            );
        };

        mob_arc
    }

    pub fn has_pumpkin(&self) -> bool {
        self.has_pumpkin.load(Ordering::Relaxed)
    }

    pub fn set_pumpkin(&self, pumpkin: bool) {
        self.has_pumpkin.store(pumpkin, Ordering::Relaxed);
        let flags: i8 = if pumpkin { 0x10 } else { 0 };
        self.mob_entity.living_entity.entity.send_meta_data(
            &[Metadata::new(tracked_data::snow_golem::PUMPKIN, flags)],
            None,
        );
    }
}

impl NBTStorage for SnowGolemEntity {
    fn write_nbt<'a>(&'a self, nbt: &'a mut NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.write_nbt(nbt).await;
            nbt.put_bool("Pumpkin", self.has_pumpkin());
        })
    }

    fn read_nbt_non_mut<'a>(&'a self, nbt: &'a NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            if let Some(value) = nbt.get_bool("Pumpkin") {
                self.has_pumpkin.store(value, Ordering::Relaxed);
            }
        })
    }
}

impl Mob for SnowGolemEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_snow_golem(&self) -> Option<&SnowGolemEntity> {
        Some(self)
    }

    fn mob_java_spawn_metadata(
        &self,
        version: JavaMinecraftVersion,
    ) -> EntityBaseFuture<'_, Option<Box<[u8]>>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            let flags: i8 = if self.has_pumpkin() { 0x10 } else { 0 };
            Metadata::new(tracked_data::snow_golem::PUMPKIN, flags)
                .write(&mut bytes, &version)
                .ok()?;
            bytes.push(255);
            Some(bytes.into_boxed_slice())
        })
    }

    fn post_tick(&self) -> EntityBaseFuture<'_, ()> {
        Box::pin(async move {
            let entity = &self.mob_entity.living_entity.entity;
            let world = entity.world.load();

            // Check game rules for mob griefing
            if !world.level_info.load().game_rules.mob_griefing {
                return;
            }

            let pos = entity.pos.load();
            // Place snow layer in the 2x2 footprint of the snow golem like vanilla
            for i in 0..4 {
                let offset_x = f64::from(i % 2 * 2 - 1) * 0.25;
                let offset_z = f64::from(i / 2 * 2 - 1) * 0.25;
                let block_pos = BlockPos::new(
                    (pos.x + offset_x).floor() as i32,
                    pos.y.floor() as i32,
                    (pos.z + offset_z).floor() as i32,
                );

                let (current_block, current_state) = world.get_block_and_state(&block_pos);
                let is_replaceable = current_state.is_air()
                    || current_block.id == Block::SHORT_GRASS.id
                    || current_block.id == Block::FERN.id
                    || current_block.id == Block::DEAD_BUSH.id;

                if is_replaceable {
                    let block_below_pos =
                        BlockPos::new(block_pos.0.x, block_pos.0.y - 1, block_pos.0.z);
                    let (block_below, state_below) = world.get_block_and_state(&block_below_pos);
                    // Snow layers can survive on solid full blocks, not air, not liquid, not barrier/structure
                    if (state_below.is_solid() || block_below.is_solid())
                        && !state_below.is_liquid()
                        && block_below.id != Block::BARRIER.id
                        && block_below.id != Block::STRUCTURE_VOID.id
                    {
                        world
                            .set_block_state(
                                &block_pos,
                                Block::SNOW.default_state.id,
                                BlockFlags::NOTIFY_ALL,
                            )
                            .await;
                    }
                }
            }
        })
    }
}
