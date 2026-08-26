use std::sync::Arc;

use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};

use super::{Controls, Goal, GoalFuture};
use crate::entity::ai::pathfinder::NavigatorGoal;
use crate::entity::mob::Mob;
use crate::entity::projectile::arrow::ArrowPickup;
use crate::entity::projectile::trident::TridentEntity;
use crate::entity::{Entity, EntityBase};

pub struct TridentAttackGoal {
    cooldown: i32,
}

impl TridentAttackGoal {
    const RANGE_SQUARED: f64 = 100.0;
    const ATTACK_INTERVAL: i32 = 40;

    #[must_use]
    pub const fn new() -> Self {
        Self { cooldown: 0 }
    }

    async fn held_trident(mob: &dyn Mob) -> Option<ItemStack> {
        let stack = mob
            .get_mob_entity()
            .living_entity
            .entity_equipment
            .lock()
            .await
            .get(&EquipmentSlot::MAIN_HAND);
        (stack.item.id == Item::TRIDENT.id).then_some(stack)
    }

    async fn shoot(mob: &dyn Mob, target: &Arc<dyn EntityBase>, stack: ItemStack) {
        let shooter = mob.get_entity();
        let world = shooter.world.load();
        let projectile_entity =
            Entity::new(world.clone(), shooter.pos.load(), &EntityType::TRIDENT);
        let trident = TridentEntity::new_shot(
            projectile_entity,
            shooter,
            stack.copy_with_count(1),
            ArrowPickup::Disallowed,
        );

        let origin = trident.entity.pos.load();
        let target_entity = target.get_entity();
        let target_pos = target_entity.pos.load();
        let dx = target_pos.x - origin.x;
        let dz = target_pos.z - origin.z;
        let horizontal = dx.hypot(dz);
        let dy =
            target_pos.y + f64::from(target_entity.entity_dimension.load().height) / 3.0 - origin.y;
        let difficulty = world.level_info.load().difficulty as i32;
        let divergence = f64::from(14 - difficulty * 4);
        trident.set_velocity(dx, horizontal.mul_add(0.2, dy), dz, 1.6, divergence);

        world.play_sound(Sound::ItemTridentThrow, SoundCategory::Hostile, &origin);
        world.spawn_entity(Arc::new(trident)).await;
    }
}

impl Default for TridentAttackGoal {
    fn default() -> Self {
        Self::new()
    }
}

impl Goal for TridentAttackGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let target_alive = mob
                .get_mob_entity()
                .target
                .lock()
                .await
                .as_ref()
                .is_some_and(|target| target.get_entity().is_alive());
            target_alive && Self::held_trident(mob).await.is_some()
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let target_alive = mob
                .get_mob_entity()
                .target
                .lock()
                .await
                .as_ref()
                .is_some_and(|target| target.get_entity().is_alive());
            target_alive && Self::held_trident(mob).await.is_some()
        })
    }

    fn tick<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.cooldown = (self.cooldown - 1).max(0);
            let Some(target) = mob.get_mob_entity().target.lock().await.clone() else {
                return;
            };
            let Some(stack) = Self::held_trident(mob).await else {
                return;
            };

            let mob_pos = mob.get_entity().pos.load();
            let target_pos = target.get_entity().pos.load();
            let distance_squared = mob_pos.squared_distance_to_vec(&target_pos);
            mob.get_mob_entity()
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .look_at_entity_with_range(&target, 30.0, 30.0);

            if distance_squared > Self::RANGE_SQUARED {
                mob.get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .set_progress(NavigatorGoal {
                        current_progress: mob_pos,
                        destination: target_pos,
                        speed: 1.0,
                    });
                return;
            }

            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stop();
            let world = mob.get_entity().world.load();
            let can_see = world
                .raycast(
                    mob.get_entity().get_eye_pos(),
                    target.get_entity().get_eye_pos(),
                    async |block_pos, world| world.get_block_state(block_pos).is_solid(),
                )
                .await
                .is_none();
            if self.cooldown == 0 && can_see {
                Self::shoot(mob, &target, stack).await;
                self.cooldown = Self::ATTACK_INTERVAL;
            }
        })
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}
