use std::sync::Arc;

use pumpkin_data::{
    attributes::Attributes,
    entity::{EntityStatus, EntityType},
    item::Item,
    item_stack::ItemStack,
    particle::Particle,
    sound::{Sound, SoundCategory},
};
use pumpkin_protocol::bedrock::server::actor_event::ActorEventType;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::{EntityBase, ageable::AgeableMob, living::LivingEntity, player::Player};

pub(super) fn taming_succeeds(temper: i32, max_temper: i32, roll: i32) -> bool {
    roll < temper.clamp(0, max_temper.max(1))
}

pub(super) fn horse_family_offspring_type(
    first: &'static EntityType,
    second: &'static EntityType,
) -> Option<&'static EntityType> {
    match (first.id, second.id) {
        (a, b) if a == EntityType::HORSE.id && b == EntityType::HORSE.id => {
            Some(&EntityType::HORSE)
        }
        (a, b) if a == EntityType::DONKEY.id && b == EntityType::DONKEY.id => {
            Some(&EntityType::DONKEY)
        }
        (a, b)
            if (a == EntityType::HORSE.id && b == EntityType::DONKEY.id)
                || (a == EntityType::DONKEY.id && b == EntityType::HORSE.id) =>
        {
            Some(&EntityType::MULE)
        }
        _ => None,
    }
}

fn inherited_attribute(first: f64, second: f64, min: f64, max: f64, random_offset: f64) -> f64 {
    let first = first.clamp(min, max);
    let second = second.clamp(min, max);
    let spread = (first - second).abs() + 0.3 * (max - min);
    let value = (first + second) / 2.0 + spread * random_offset.clamp(-0.5, 0.5);
    if value > max {
        2.0 * max - value
    } else if value < min {
        2.0 * min - value
    } else {
        value
    }
}

pub(super) fn configure_bred_equine_attributes(
    first: &LivingEntity,
    mate: &dyn EntityBase,
    child: &Arc<dyn EntityBase>,
) {
    let (Some(second), Some(child)) = (mate.get_living_entity(), child.get_living_entity()) else {
        return;
    };
    let mut rng = rand::rng();
    for (attribute, min, max) in [
        (&Attributes::MAX_HEALTH, 15.0, 30.0),
        (&Attributes::JUMP_STRENGTH, 0.4, 1.0),
        (&Attributes::MOVEMENT_SPEED, 0.1125, 0.3375),
    ] {
        let offset = (rng.random::<f64>() + rng.random::<f64>() + rng.random::<f64>()) / 3.0 - 0.5;
        child.set_attribute_base(
            attribute,
            inherited_attribute(
                first.get_attribute_base(attribute),
                second.get_attribute_base(attribute),
                min,
                max,
                offset,
            ),
        );
    }
    child
        .health
        .store(child.get_attribute_value(&Attributes::MAX_HEALTH) as f32);
}

pub(super) trait Equine: AgeableMob {
    fn temper(&self) -> i32 {
        100
    }
    fn add_temper(&self, _amount: i32) {}
    fn set_tamed(&self, _tamed: bool, _owner: Option<Uuid>) {}
    fn can_breed(&self) -> bool {
        true
    }
    fn max_temper(&self) -> i32 {
        100
    }
    fn food_effect(&self, item: &Item) -> Option<FoodEffect> {
        horse_food_effect(item)
    }
}

pub(super) async fn tick_untamed_riding<T: Equine>(equine: &T) {
    if equine.is_tame() {
        return;
    }
    let entity = equine.get_entity();
    let passenger = entity.passengers.lock().await.first().cloned();
    let Some(passenger) = passenger else {
        return;
    };
    let Some(player) = passenger.get_player() else {
        return;
    };
    if rand::rng().random_range(0..50) != 0 {
        return;
    }
    let max_temper = equine.max_temper().max(1);
    let success = taming_succeeds(
        equine.temper(),
        max_temper,
        rand::rng().random_range(0..max_temper),
    );
    if success {
        let player = entity.world.load().get_player_by_id(player.entity_id());
        let Some(player) = player else {
            return;
        };
        let mut event = crate::plugin::api::events::entity::entity_tame::EntityTameEvent::new(
            entity.entity_id,
            player.clone(),
        );
        if let Some(server) = entity.world.load().server.upgrade() {
            server.plugin_manager.fire(&server, &mut event).await;
        }
        if !event.cancelled {
            equine.set_tamed(true, Some(player.gameprofile.id));
            entity.world.load().send_entity_status(
                entity,
                EntityStatus::TamingSucceeded,
                Some(ActorEventType::TamingSucceeded),
            );
            return;
        }
    }

    equine.add_temper(5);
    entity
        .remove_passenger(passenger.get_entity().entity_id)
        .await;
    entity.world.load().send_entity_status(
        entity,
        EntityStatus::TamingFailed,
        Some(ActorEventType::TamingFailed),
    );
}

pub(super) async fn mount_equine<T: Equine>(
    equine: &T,
    player: &Arc<Player>,
    stack: &ItemStack,
) -> bool {
    if !stack.is_empty() || equine.is_baby() || equine.get_entity().has_passengers().await {
        return false;
    }
    let world = player.world();
    let entity = equine.get_entity();
    if let Some(vehicle) = world.get_entity_by_id(entity.entity_id)
        && let Some(passenger) = world.get_player_by_id(player.entity_id())
    {
        entity
            .add_passenger(vehicle, passenger as Arc<dyn EntityBase>)
            .await;
        return true;
    }
    false
}

#[derive(Debug, PartialEq)]
pub(super) struct FoodEffect {
    pub healing: f32,
    pub growth_seconds: i32,
    pub temper: i32,
    pub breeds: bool,
}

fn horse_food_effect(item: &Item) -> Option<FoodEffect> {
    match item.id {
        id if id == Item::SUGAR.id => Some(FoodEffect {
            healing: 1.0,
            growth_seconds: 30,
            temper: 3,
            breeds: false,
        }),
        id if id == Item::WHEAT.id => Some(FoodEffect {
            healing: 2.0,
            growth_seconds: 20,
            temper: 3,
            breeds: false,
        }),
        id if id == Item::APPLE.id => Some(FoodEffect {
            healing: 3.0,
            growth_seconds: 60,
            temper: 3,
            breeds: false,
        }),
        id if id == Item::GOLDEN_CARROT.id => Some(FoodEffect {
            healing: 4.0,
            growth_seconds: 60,
            temper: 5,
            breeds: true,
        }),
        id if id == Item::GOLDEN_APPLE.id || id == Item::ENCHANTED_GOLDEN_APPLE.id => {
            Some(FoodEffect {
                healing: 10.0,
                growth_seconds: 240,
                temper: 10,
                breeds: true,
            })
        }
        id if id == Item::HAY_BLOCK.id => Some(FoodEffect {
            healing: 20.0,
            growth_seconds: 180,
            temper: 0,
            breeds: false,
        }),
        _ => None,
    }
}

pub async fn feed_equine<T: Equine>(
    equine: &T,
    player: &Arc<Player>,
    stack: &mut ItemStack,
    sound: Sound,
) -> bool {
    let Some(effect) = equine.food_effect(stack.item) else {
        return false;
    };
    let living = &equine.get_mob_entity().living_entity;
    let can_heal = living.health.load() < living.get_max_health();
    let can_grow = equine.is_baby();
    let can_gain_temper =
        !equine.is_tame() && effect.temper > 0 && equine.temper() < equine.max_temper();
    let can_breed = effect.breeds
        && equine.can_breed()
        && equine.is_tame()
        && equine.get_age() == 0
        && !equine.get_mob_entity().is_in_love();
    if !can_heal && !can_grow && !can_gain_temper && !can_breed {
        return false;
    }
    stack.decrement_unless_creative(player.gamemode.load(), 1);
    if can_heal {
        living.heal(effect.healing);
    }
    if can_grow {
        equine.age_up(effect.growth_seconds, true);
    }
    if can_gain_temper {
        equine.add_temper(effect.temper);
    }
    let entity = equine.get_entity();
    let pos = entity.pos.load();
    let world = entity.world.load();
    world.play_sound(sound, SoundCategory::Neutral, &pos);
    world.spawn_particle(
        pos + Vector3::new(0.0, f64::from(entity.height()), 0.0),
        Vector3::new(0.5, 0.5, 0.5),
        1.0,
        7,
        Particle::HappyVillager,
    );
    if can_breed {
        equine
            .get_mob_entity()
            .set_love_ticks(600, Some(player.gameprofile.id));
        world.send_entity_status(
            entity,
            EntityStatus::InLoveHearts,
            Some(ActorEventType::InLoveHearts),
        );
        world.spawn_particle(
            pos + Vector3::new(0.0, f64::from(entity.height()), 0.0),
            Vector3::new(0.5, 0.5, 0.5),
            1.0,
            7,
            Particle::Heart,
        );
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_foal_growth_values() {
        let wheat = horse_food_effect(&Item::WHEAT).unwrap();
        assert_eq!(
            (wheat.healing, wheat.growth_seconds, wheat.temper),
            (2.0, 20, 3)
        );
        let sugar = horse_food_effect(&Item::SUGAR).unwrap();
        assert_eq!(
            (sugar.healing, sugar.growth_seconds, sugar.temper),
            (1.0, 30, 3)
        );
        let apple = horse_food_effect(&Item::APPLE).unwrap();
        assert_eq!(
            (apple.healing, apple.growth_seconds, apple.temper),
            (3.0, 60, 3)
        );
        let carrot = horse_food_effect(&Item::GOLDEN_CARROT).unwrap();
        assert_eq!(
            (
                carrot.healing,
                carrot.growth_seconds,
                carrot.temper,
                carrot.breeds
            ),
            (4.0, 60, 5, true)
        );
        let hay = horse_food_effect(&Item::HAY_BLOCK).unwrap();
        assert_eq!(
            (hay.healing, hay.growth_seconds, hay.temper),
            (20.0, 180, 0)
        );
        let golden_apple = horse_food_effect(&Item::GOLDEN_APPLE).unwrap();
        assert_eq!(
            (
                golden_apple.healing,
                golden_apple.growth_seconds,
                golden_apple.temper,
                golden_apple.breeds
            ),
            (10.0, 240, 10, true)
        );
    }

    #[test]
    fn horse_and_donkey_pairings_produce_vanilla_offspring() {
        assert_eq!(
            horse_family_offspring_type(&EntityType::HORSE, &EntityType::HORSE),
            Some(&EntityType::HORSE)
        );
        assert_eq!(
            horse_family_offspring_type(&EntityType::DONKEY, &EntityType::DONKEY),
            Some(&EntityType::DONKEY)
        );
        assert_eq!(
            horse_family_offspring_type(&EntityType::HORSE, &EntityType::DONKEY),
            Some(&EntityType::MULE)
        );
        assert_eq!(
            horse_family_offspring_type(&EntityType::DONKEY, &EntityType::HORSE),
            Some(&EntityType::MULE)
        );
        assert_eq!(
            horse_family_offspring_type(&EntityType::HORSE, &EntityType::LLAMA),
            None
        );
    }

    #[test]
    fn horse_attribute_inheritance_averages_and_reflects_into_range() {
        assert_eq!(inherited_attribute(20.0, 24.0, 15.0, 30.0, 0.0), 22.0);
        let high = inherited_attribute(30.0, 30.0, 15.0, 30.0, 0.5);
        assert!((high - 27.75).abs() < f64::EPSILON);
        let low = inherited_attribute(15.0, 15.0, 15.0, 30.0, -0.5);
        assert!((low - 17.25).abs() < f64::EPSILON);
    }
}
