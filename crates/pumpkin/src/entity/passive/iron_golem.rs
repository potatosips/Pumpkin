use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use pumpkin_data::entity::{EntityStatus, EntityType};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::java::client::play::Metadata;
use pumpkin_util::GameMode;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, NBTStorage, NbtFuture,
    ai::goal::{
        defend_village_target::DefendVillageTargetGoal,
        iron_golem_hostile_target::IronGolemHostileTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal,
        offer_flower::OfferFlowerGoal, revenge::RevengeGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    player::Player,
};

/// Represents an Iron Golem, a powerful neutral mob that protects villagers and players.
///
/// Wiki: <https://minecraft.wiki/w/Iron_Golem>
pub struct IronGolemEntity {
    pub mob_entity: MobEntity,
    pub player_created: AtomicBool,
    pub attack_animation_tick: AtomicI32,
    pub offer_flower_tick: AtomicI32,
}

impl IronGolemEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let iron_golem = Self {
            mob_entity,
            player_created: AtomicBool::new(false),
            attack_animation_tick: AtomicI32::new(0),
            offer_flower_tick: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(iron_golem);
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

            goal_selector.add_goal(1, Box::new(MeleeAttackGoal::new(1.0, true)));
            goal_selector.add_goal(5, Box::new(OfferFlowerGoal::new(mob_arc.clone())));
            goal_selector.add_goal(6, Box::new(WanderAroundGoal::new(0.6)));
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));

            target_selector.add_goal(1, Box::new(DefendVillageTargetGoal::new(mob_arc.clone())));
            target_selector.add_goal(2, Box::new(RevengeGoal::new(false)));
            target_selector.add_goal(3, Box::new(IronGolemHostileTargetGoal::new()));
        };

        mob_arc
    }

    #[must_use]
    pub fn is_player_created(&self) -> bool {
        self.player_created.load(Ordering::Relaxed)
    }

    pub fn set_player_created(&self, value: bool) {
        self.player_created.store(value, Ordering::Relaxed);
        let entity = self.get_entity();
        let flag: u8 = u8::from(value);
        entity.send_meta_data(
            &[Metadata::new(
                pumpkin_data::tracked_data::iron_golem::FLAGS_ID,
                flag,
            )],
            None,
        );
    }

    pub fn set_offering_flower(&self, offering: bool) {
        let entity = self.get_entity();
        if offering {
            self.offer_flower_tick.store(400, Ordering::Relaxed);
            entity
                .world
                .load()
                .send_entity_status(entity, EntityStatus::OfferFlower, None);
        } else {
            self.offer_flower_tick.store(0, Ordering::Relaxed);
            entity
                .world
                .load()
                .send_entity_status(entity, EntityStatus::StopOfferFlower, None);
        }
    }
}

fn iron_golem_attack_damage(base_damage: f32, random_roll: i32) -> f32 {
    if base_damage > 0.0 {
        base_damage / 2.0 + random_roll as f32
    } else {
        base_damage
    }
}

fn iron_golem_upward_knockback(knockback_resistance: f64) -> f64 {
    0.4 * (1.0 - knockback_resistance).max(0.0)
}

impl NBTStorage for IronGolemEntity {
    fn write_nbt<'a>(&'a self, nbt: &'a mut NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.write_nbt(nbt).await;
            nbt.put_bool("PlayerCreated", self.is_player_created());
        })
    }

    fn read_nbt_non_mut<'a>(&'a self, nbt: &'a NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            if let Some(created) = nbt.get_bool("PlayerCreated") {
                self.set_player_created(created);
            }
        })
    }
}

impl Mob for IronGolemEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick<'a>(&'a self, _caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            let attack_tick = self.attack_animation_tick.load(Ordering::Relaxed);
            if attack_tick > 0 {
                self.attack_animation_tick.fetch_sub(1, Ordering::Relaxed);
            }

            let flower_tick = self.offer_flower_tick.load(Ordering::Relaxed);
            if flower_tick > 0 {
                self.offer_flower_tick.fetch_sub(1, Ordering::Relaxed);
            }
        })
    }

    fn get_attack_damage(&self) -> f32 {
        let base_damage = self
            .mob_entity
            .living_entity
            .get_attribute_value(&pumpkin_data::attributes::Attributes::ATTACK_DAMAGE)
            as f32;
        let roll = if base_damage > 0.0 {
            rand::rng().random_range(0..base_damage as i32)
        } else {
            0
        };
        iron_golem_attack_damage(base_damage, roll)
    }

    fn on_attack_attempt<'a>(&'a self, _target: &'a dyn EntityBase) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            self.attack_animation_tick.store(10, Ordering::Relaxed);

            let entity = self.get_entity();
            let world = entity.world.load();
            world.send_entity_status(entity, EntityStatus::StartAttacking, None);
            world.play_sound(
                Sound::EntityIronGolemAttack,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
        })
    }

    fn on_successful_attack<'a>(&'a self, target: &'a dyn EntityBase) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            let target_entity = target.get_entity();
            let mut velocity = target_entity.velocity.load();
            let knockback_resistance = target.get_living_entity().map_or(0.0, |living| {
                living.get_attribute_value(
                    &pumpkin_data::attributes::Attributes::KNOCKBACK_RESISTANCE,
                )
            });
            velocity.y += iron_golem_upward_knockback(knockback_resistance);
            target_entity.velocity.store(velocity);
            target_entity.velocity_dirty.store(true, Ordering::Relaxed);
        })
    }

    fn mob_init_data_tracker(&self) -> EntityBaseFuture<'_, ()> {
        Box::pin(async move {
            let entity = self.get_entity();
            let flag: u8 = u8::from(self.is_player_created());
            entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::iron_golem::FLAGS_ID,
                    flag,
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
            if item_stack.item.id == Item::IRON_INGOT.id {
                let living = &self.mob_entity.living_entity;
                let current_health = living.health.load();
                let max_health = living.get_max_health();
                if current_health < max_health {
                    living.set_health((current_health + 25.0).min(max_health));
                    let entity = self.get_entity();
                    let world = entity.world.load();
                    let pos = entity.pos.load();
                    // Status 11 is interpreted as repair particles for iron
                    // golems (and as the offer-flower animation for villagers).
                    world.send_entity_status(entity, EntityStatus::OfferFlower, None);
                    world.play_sound(Sound::EntityIronGolemRepair, SoundCategory::Neutral, &pos);
                    if player.gamemode.load() != GameMode::Creative {
                        item_stack.item_count = item_stack.item_count.saturating_sub(1);
                    }
                    return true;
                }
            }
            false
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{iron_golem_attack_damage, iron_golem_upward_knockback};

    #[test]
    fn vanilla_iron_golem_damage_uses_half_base_plus_random_integer() {
        assert_eq!(iron_golem_attack_damage(15.0, 0), 7.5);
        assert_eq!(iron_golem_attack_damage(15.0, 7), 14.5);
        assert_eq!(iron_golem_attack_damage(15.0, 14), 21.5);
        assert_eq!(iron_golem_attack_damage(0.0, 0), 0.0);
    }

    #[test]
    fn vanilla_iron_golem_launch_scales_with_knockback_resistance() {
        assert!((iron_golem_upward_knockback(0.0) - 0.4).abs() < f64::EPSILON);
        assert!((iron_golem_upward_knockback(0.5) - 0.2).abs() < f64::EPSILON);
        assert_eq!(iron_golem_upward_knockback(1.0), 0.0);
        assert_eq!(iron_golem_upward_knockback(2.0), 0.0);
    }
}
