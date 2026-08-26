use pumpkin_data::data_component::DataComponent;
use pumpkin_data::data_component_impl::{
    DataComponentImpl, EquipmentSlot, PotionContentsImpl, StatusEffectInstance,
};
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::Hand;
use rand::RngExt;
use std::borrow::Cow;
use std::sync::Arc;

use crate::entity::ai::goal::{Controls, Goal, GoalFuture};
use crate::entity::ai::pathfinder::NavigatorGoal;
use crate::entity::mob::Mob;
use crate::entity::projectile::arrow::{ArrowEntity, ArrowPickup};
use crate::entity::{Entity, EntityBase};

/// Ranged bow attack used by skeletons and their variants.
/// Mirrors vanilla `RangedBowAttackGoal`: the mob keeps its distance, draws the bow
/// and releases an arrow once it has been drawn long enough.
pub struct BowAttackGoal {
    goal_control: Controls,
    speed: f64,
    attack_interval: i32,
    squared_range: f64,
    cooldown: i32,
    draw_ticks: i32,
    drawing: bool,
    seen_ticks: i32,
    strafing_ticks: i32,
    strafing_clockwise: bool,
    strafing_backwards: bool,
}

impl BowAttackGoal {
    /// Ticks the bow has to be drawn before the arrow is released.
    const DRAW_TIME: i32 = 20;
    /// Vanilla arrow speed for mob shots.
    const ARROW_SPEED: f64 = 1.6;

    #[must_use]
    pub fn new(speed: f64, attack_interval: i32, range: f32) -> Self {
        Self {
            goal_control: Controls::MOVE | Controls::LOOK,
            speed,
            attack_interval,
            squared_range: f64::from(range * range),
            cooldown: -1,
            draw_ticks: 0,
            drawing: false,
            seen_ticks: 0,
            strafing_ticks: -1,
            strafing_clockwise: false,
            strafing_backwards: false,
        }
    }

    async fn main_hand_item(mob: &dyn Mob) -> ItemStack {
        mob.get_mob_entity()
            .living_entity
            .entity_equipment
            .lock()
            .await
            .get(&EquipmentSlot::MAIN_HAND)
    }

    async fn is_holding_bow(mob: &dyn Mob) -> bool {
        Self::main_hand_item(mob).await.item.id == Item::BOW.id
    }

    async fn stop_drawing(&mut self, mob: &dyn Mob) {
        if self.drawing {
            mob.get_mob_entity().living_entity.clear_active_hand().await;
            self.drawing = false;
            self.draw_ticks = 0;
        }
    }

    fn projectile_for(mob: &dyn Mob) -> ItemStack {
        Self::projectile_for_type(mob.get_entity().entity_type)
    }

    fn projectile_for_type(entity_type: &'static EntityType) -> ItemStack {
        let effect = if entity_type.id == EntityType::STRAY.id {
            Some(("minecraft:slowness", 4_800))
        } else if entity_type.id == EntityType::BOGGED.id {
            Some(("minecraft:poison", 800))
        } else {
            None
        };

        let Some((effect_id, duration)) = effect else {
            return ItemStack::new(1, &Item::ARROW);
        };

        let mut projectile = ItemStack::new(1, &Item::TIPPED_ARROW);
        projectile.patch.push((
            DataComponent::PotionContents,
            Some(
                PotionContentsImpl {
                    potion_id: None,
                    custom_color: None,
                    custom_effects: vec![StatusEffectInstance {
                        effect_id: Cow::Borrowed(effect_id),
                        amplifier: 0,
                        duration,
                        ambient: false,
                        show_particles: true,
                        show_icon: true,
                    }],
                    custom_name: None,
                }
                .to_dyn(),
            ),
        ));
        projectile
    }

    /// Spawns the arrow, matching vanilla `AbstractSkeleton::performRangedAttack`.
    async fn shoot(mob: &dyn Mob, target: &Arc<dyn EntityBase>) {
        let entity = mob.get_entity();
        let world = entity.world.load();

        let mut event =
            crate::plugin::api::events::entity::entity_shoot_bow::EntityShootBowEvent::new(
                entity.entity_id,
                "minecraft:bow".to_string(),
                1.0,
            );
        if let Some(server) = world.server.upgrade() {
            server.plugin_manager.fire(&server, &mut event).await;
        }
        if event.cancelled {
            return;
        }

        let arrow_entity = Entity::new(world.clone(), entity.pos.load(), &EntityType::ARROW);
        let projectile = Self::projectile_for(mob);
        let arrow = ArrowEntity::new_shot(arrow_entity, entity, &projectile, ArrowPickup::Allowed);

        let mob_pos = entity.pos.load();
        let target_entity = target.get_entity();
        let target_pos = target_entity.pos.load();

        let dx = target_pos.x - mob_pos.x;
        let dy = (target_pos.y + f64::from(target_entity.entity_dimension.load().height) / 3.0)
            - arrow.entity.pos.load().y;
        let dz = target_pos.z - mob_pos.z;
        let horizontal_distance = dx.hypot(dz);

        // Vanilla scales the spread with the world difficulty: 14 - difficulty * 4.
        let difficulty = world.level_info.load().difficulty as i32;
        let divergence = f64::from(14 - difficulty * 4);

        arrow.set_velocity(
            dx,
            horizontal_distance.mul_add(0.2, dy),
            dz,
            Self::ARROW_SPEED,
            divergence,
        );

        world.play_sound(Sound::EntityArrowShoot, SoundCategory::Hostile, &mob_pos);

        let arrow: Arc<dyn EntityBase> = Arc::new(arrow);
        world.spawn_entity(arrow).await;
    }
}

#[cfg(test)]
mod tests {
    use super::BowAttackGoal;
    use pumpkin_data::data_component_impl::PotionContentsImpl;
    use pumpkin_data::entity::EntityType;
    use pumpkin_data::item::Item;

    #[test]
    fn skeleton_variants_create_vanilla_effect_arrows() {
        let stray = BowAttackGoal::projectile_for_type(&EntityType::STRAY);
        assert_eq!(stray.item.id, Item::TIPPED_ARROW.id);
        let stray_effects = &stray
            .get_data_component::<PotionContentsImpl>()
            .expect("stray arrow must contain potion data")
            .custom_effects;
        assert_eq!(stray_effects[0].effect_id, "minecraft:slowness");
        assert_eq!(stray_effects[0].duration, 4_800);

        let bogged = BowAttackGoal::projectile_for_type(&EntityType::BOGGED);
        assert_eq!(bogged.item.id, Item::TIPPED_ARROW.id);
        let bogged_effects = &bogged
            .get_data_component::<PotionContentsImpl>()
            .expect("bogged arrow must contain potion data")
            .custom_effects;
        assert_eq!(bogged_effects[0].effect_id, "minecraft:poison");
        assert_eq!(bogged_effects[0].duration, 800);

        let skeleton = BowAttackGoal::projectile_for_type(&EntityType::SKELETON);
        assert_eq!(skeleton.item.id, Item::ARROW.id);
    }
}

impl Goal for BowAttackGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let target = mob.get_mob_entity().target.lock().await.clone();
            let Some(target) = target else {
                return false;
            };
            if !target.get_entity().is_alive() {
                return false;
            }
            Self::is_holding_bow(mob).await
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let target = mob.get_mob_entity().target.lock().await.clone();
            let Some(target) = target else {
                return false;
            };
            target.get_entity().is_alive() && Self::is_holding_bow(mob).await
        })
    }

    fn start<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.cooldown = -1;
            self.draw_ticks = 0;
            self.drawing = false;
            self.seen_ticks = 0;
            self.strafing_ticks = -1;
        })
    }

    fn stop<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.stop_drawing(mob).await;
            self.cooldown = -1;
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stop();
        })
    }

    fn tick<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            let target = mob.get_mob_entity().target.lock().await.clone();
            let Some(target) = target else {
                return;
            };

            let mob_pos = mob.get_entity().pos.load();
            let target_pos = target.get_entity().pos.load();
            let distance_sq = mob_pos.squared_distance_to_vec(&target_pos);
            let world = mob.get_entity().world.load();
            let can_see = world
                .raycast(
                    mob.get_entity().get_eye_pos(),
                    target.get_entity().get_eye_pos(),
                    async |block_pos, world| world.get_block_state(block_pos).is_solid(),
                )
                .await
                .is_none();

            if can_see {
                self.seen_ticks = self.seen_ticks.max(0) + 1;
            } else {
                self.seen_ticks = self.seen_ticks.min(0) - 1;
            }

            mob.get_mob_entity()
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .look_at_entity_with_range(&target, 30.0, 30.0);

            // Vanilla holds position only after seeing the target continuously, then
            // strafes instead of standing still.
            {
                let mut navigator = mob
                    .get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if distance_sq > self.squared_range || self.seen_ticks < 20 {
                    navigator.set_progress(NavigatorGoal {
                        current_progress: mob_pos,
                        destination: target_pos,
                        speed: self.speed,
                    });
                    self.strafing_ticks = -1;
                } else {
                    navigator.stop();
                    self.strafing_ticks += 1;
                }
            }

            if self.strafing_ticks >= 20 {
                if mob.get_random().random_range(0.0..1.0) < 0.3 {
                    self.strafing_clockwise = !self.strafing_clockwise;
                }
                if mob.get_random().random_range(0.0..1.0) < 0.3 {
                    self.strafing_backwards = !self.strafing_backwards;
                }
                self.strafing_ticks = 0;
            }

            if self.strafing_ticks > -1 {
                if distance_sq > self.squared_range * 0.75 {
                    self.strafing_backwards = false;
                } else if distance_sq < self.squared_range * 0.25 {
                    self.strafing_backwards = true;
                }
                mob.get_mob_entity()
                    .move_control
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .strafe(
                        if self.strafing_backwards { -0.5 } else { 0.5 },
                        if self.strafing_clockwise { 0.5 } else { -0.5 },
                    );
            }

            if self.drawing {
                if !can_see && self.seen_ticks < -60 {
                    self.stop_drawing(mob).await;
                    return;
                }
                self.draw_ticks += 1;
                if self.draw_ticks >= Self::DRAW_TIME && can_see {
                    self.stop_drawing(mob).await;
                    Self::shoot(mob, &target).await;
                    self.cooldown = self.attack_interval;
                }
                return;
            }

            self.cooldown -= 1;
            if self.cooldown <= 0 && distance_sq <= self.squared_range && can_see {
                let stack = Self::main_hand_item(mob).await;
                mob.get_mob_entity()
                    .living_entity
                    .set_active_hand(Hand::Right, stack, i32::MAX)
                    .await;
                self.drawing = true;
                self.draw_ticks = 0;
            }
        })
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        self.goal_control
    }
}
