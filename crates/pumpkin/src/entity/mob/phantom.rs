use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex, Weak};

use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{
    Entity, EntityBase, NBTStorage, NbtFuture,
    ai::goal::{
        Controls, Goal, GoalFuture, active_target::ActiveTargetGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal,
    },
    mob::{Mob, MobEntity},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhantomAttackPhase {
    Circle,
    Swoop,
}

pub struct PhantomEntity {
    pub mob_entity: MobEntity,
    pub size: AtomicI32,
    pub attack_phase: Mutex<PhantomAttackPhase>,
    pub phase_ticks: AtomicI32,
    pub anchor_point: Mutex<Vector3<f64>>,
    pub circling_angle: Mutex<f32>,
}

impl PhantomEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let spawn_pos = entity.pos.load();
        let mob_entity = MobEntity::new(entity);
        let phantom = Self {
            mob_entity,
            size: AtomicI32::new(0),
            attack_phase: Mutex::new(PhantomAttackPhase::Circle),
            phase_ticks: AtomicI32::new(0),
            anchor_point: Mutex::new(Vector3::new(spawn_pos.x, spawn_pos.y + 20.0, spawn_pos.z)),
            circling_angle: Mutex::new(0.0),
        };
        let mob_arc = Arc::new(phantom);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let phantom_weak = Arc::downgrade(&mob_arc);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(1, Box::new(PhantomFlightAttackGoal::new(phantom_weak)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 16.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));
        };

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            target_selector.add_goal(
                1,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
        }

        mob_arc
    }

    pub fn get_size(&self) -> i32 {
        self.size.load(Ordering::Relaxed)
    }

    pub fn set_size(&self, size: i32) {
        self.size.store(size.clamp(0, 64), Ordering::Relaxed);
    }

    #[must_use]
    pub const fn calculate_attack_damage(size: i32) -> f32 {
        6.0 + if size < 0 {
            0.0
        } else if size > 64 {
            64.0
        } else {
            size as f32
        }
    }
}

impl NBTStorage for PhantomEntity {
    fn write_nbt<'a>(&'a self, nbt: &'a mut NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.write_nbt(nbt).await;
            nbt.put_int("Size", self.get_size());
            let anchor = *self.anchor_point.lock().unwrap();
            nbt.put_int("AX", anchor.x.floor() as i32);
            nbt.put_int("AY", anchor.y.floor() as i32);
            nbt.put_int("AZ", anchor.z.floor() as i32);
        })
    }

    fn read_nbt_non_mut<'a>(&'a self, nbt: &'a NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            if let Some(size) = nbt.get_int("Size") {
                self.set_size(size);
            }
            if let (Some(ax), Some(ay), Some(az)) =
                (nbt.get_int("AX"), nbt.get_int("AY"), nbt.get_int("AZ"))
            {
                *self.anchor_point.lock().unwrap() =
                    Vector3::new(f64::from(ax), f64::from(ay), f64::from(az));
            }
        })
    }
}

impl Mob for PhantomEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_mob_gravity(&self) -> f64 {
        0.0
    }

    fn get_attack_damage(&self) -> f32 {
        Self::calculate_attack_damage(self.get_size())
    }
}

pub struct PhantomFlightAttackGoal {
    phantom: Weak<PhantomEntity>,
}

impl PhantomFlightAttackGoal {
    #[must_use]
    pub const fn new(phantom: Weak<PhantomEntity>) -> Self {
        Self { phantom }
    }
}

impl Goal for PhantomFlightAttackGoal {
    fn can_start<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move { self.phantom.upgrade().is_some() })
    }

    fn should_continue<'a>(&'a self, _mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move { self.phantom.upgrade().is_some() })
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }

    fn tick<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            let Some(phantom) = self.phantom.upgrade() else {
                return;
            };

            let entity = phantom.get_entity();
            let pos = entity.pos.load();
            let world = entity.world.load();

            let target = phantom.mob_entity.target.lock().await.clone();
            let current_phase = *phantom.attack_phase.lock().unwrap();
            let ticks = phantom.phase_ticks.fetch_add(1, Ordering::Relaxed);

            // Flap sound periodically
            if ticks % 60 == 0 {
                world.play_sound(Sound::EntityPhantomFlap, SoundCategory::Hostile, &pos);
            }

            match current_phase {
                PhantomAttackPhase::Circle => {
                    let mut anchor = *phantom.anchor_point.lock().unwrap();

                    // If we have a living player target, keep anchor point hovering above them
                    if let Some(target) = &target
                        && target.get_entity().is_alive()
                    {
                        let target_pos = target.get_entity().pos.load();
                        anchor = Vector3::new(target_pos.x, target_pos.y + 20.0, target_pos.z);
                        *phantom.anchor_point.lock().unwrap() = anchor;

                        // After 200 ticks of circling with a valid target, swoop down!
                        if ticks >= 200 {
                            *phantom.attack_phase.lock().unwrap() = PhantomAttackPhase::Swoop;
                            phantom.phase_ticks.store(0, Ordering::Relaxed);
                            world.play_sound(
                                Sound::EntityPhantomSwoop,
                                SoundCategory::Hostile,
                                &pos,
                            );
                            return;
                        }
                    }

                    // Circle around anchor point
                    let mut angle = *phantom.circling_angle.lock().unwrap();
                    angle += 0.04;
                    if angle > std::f32::consts::TAU {
                        angle -= std::f32::consts::TAU;
                    }
                    *phantom.circling_angle.lock().unwrap() = angle;

                    let radius = 16.0;
                    let target_x = anchor.x + (radius * f64::from(angle.cos()));
                    let target_y = anchor.y;
                    let target_z = anchor.z + (radius * f64::from(angle.sin()));
                    let wanted_pos = Vector3::new(target_x, target_y, target_z);

                    let dir = wanted_pos.sub(&pos);
                    let dist = dir.length();
                    if dist > 0.1 {
                        let velo = dir.normalize().multiply(0.35, 0.35, 0.35);
                        entity.velocity.store(velo);
                        entity.velocity_dirty.store(true, Ordering::Relaxed);
                    }
                }
                PhantomAttackPhase::Swoop => {
                    if let Some(target) = &target
                        && target.get_entity().is_alive()
                    {
                        let target_pos = target.get_entity().pos.load();
                        let dir = target_pos.sub(&pos);
                        let dist_sq = pos.squared_distance_to_vec(&target_pos);

                        if dist_sq <= 4.0 {
                            // Melee bite attack
                            world.play_sound(
                                Sound::EntityPhantomBite,
                                SoundCategory::Hostile,
                                &pos,
                            );
                            let _ = phantom
                                .mob_entity
                                .try_attack(phantom.as_ref(), target.as_ref())
                                .await;

                            // Reset to circle mode above player
                            *phantom.anchor_point.lock().unwrap() =
                                Vector3::new(target_pos.x, target_pos.y + 20.0, target_pos.z);
                            *phantom.attack_phase.lock().unwrap() = PhantomAttackPhase::Circle;
                            phantom.phase_ticks.store(0, Ordering::Relaxed);
                            return;
                        }

                        // Swoop speed
                        let velo = dir.normalize().multiply(0.65, 0.65, 0.65);
                        entity.velocity.store(velo);
                        entity.velocity_dirty.store(true, Ordering::Relaxed);

                        // Timeout swoop after 100 ticks
                        if ticks >= 100 {
                            *phantom.anchor_point.lock().unwrap() =
                                Vector3::new(pos.x, pos.y + 20.0, pos.z);
                            *phantom.attack_phase.lock().unwrap() = PhantomAttackPhase::Circle;
                            phantom.phase_ticks.store(0, Ordering::Relaxed);
                        }
                    } else {
                        // Target lost, return to circle
                        *phantom.attack_phase.lock().unwrap() = PhantomAttackPhase::Circle;
                        phantom.phase_ticks.store(0, Ordering::Relaxed);
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_phantom_damage_scales_with_size() {
        assert_eq!(PhantomEntity::calculate_attack_damage(0), 6.0);
        assert_eq!(PhantomEntity::calculate_attack_damage(1), 7.0);
        assert_eq!(PhantomEntity::calculate_attack_damage(3), 9.0);
        assert_eq!(PhantomEntity::calculate_attack_damage(64), 70.0);
        assert_eq!(PhantomEntity::calculate_attack_damage(-1), 6.0);
    }
}
