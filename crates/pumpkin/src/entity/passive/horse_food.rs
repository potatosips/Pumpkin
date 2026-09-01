use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicU8, Ordering};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::{
    attributes::Attributes,
    entity::{EntityStatus, EntityType},
    item::Item,
    item_stack::ItemStack,
    particle::Particle,
    sound::{Sound, SoundCategory},
};
use pumpkin_protocol::bedrock::server::actor_event::ActorEventType;
use pumpkin_protocol::java::server::play::SPlayerInput;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;
use uuid::Uuid;

pub(super) struct EquineRiderControl {
    jump_scale: AtomicCell<f32>,
}

/// Server-owned portion of AbstractHorse's client-visible animation flags.
/// Mouth animation lasts 30 ticks and rearing lasts 20 grounded ticks in
/// Vanilla 1.21.4.
pub(super) struct EquineAnimationState {
    flags: AtomicU8,
    mouth_counter: AtomicI32,
    stand_counter: AtomicI32,
}

impl Default for EquineAnimationState {
    fn default() -> Self {
        Self {
            flags: AtomicU8::new(0),
            mouth_counter: AtomicI32::new(0),
            stand_counter: AtomicI32::new(0),
        }
    }
}

impl EquineAnimationState {
    pub fn flags(&self) -> u8 {
        self.flags.load(Ordering::Relaxed)
    }

    fn open_mouth(&self) {
        self.mouth_counter.store(1, Ordering::Relaxed);
        self.flags.fetch_or(0x40, Ordering::Relaxed);
    }

    fn stand(&self) {
        self.stand_counter.store(1, Ordering::Relaxed);
        self.flags
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |flags| {
                Some((flags & !0x10) | 0x20)
            })
            .ok();
    }

    /// Returns whether a metadata flag expired and clients need an update.
    fn tick(&self, on_ground: bool) -> bool {
        let mut changed = false;
        let mouth = self.mouth_counter.load(Ordering::Relaxed);
        if mouth > 0 {
            let next = mouth + 1;
            if next > 30 {
                self.mouth_counter.store(0, Ordering::Relaxed);
                self.flags.fetch_and(!0x40, Ordering::Relaxed);
                changed = true;
            } else {
                self.mouth_counter.store(next, Ordering::Relaxed);
            }
        }
        let standing = self.stand_counter.load(Ordering::Relaxed);
        if on_ground && standing > 0 {
            let next = standing + 1;
            if next > 20 {
                self.stand_counter.store(0, Ordering::Relaxed);
                self.flags.fetch_and(!0x20, Ordering::Relaxed);
                changed = true;
            } else {
                self.stand_counter.store(next, Ordering::Relaxed);
            }
        }
        changed
    }
}

impl Default for EquineRiderControl {
    fn default() -> Self {
        Self {
            jump_scale: AtomicCell::new(0.0),
        }
    }
}

fn jump_scale_from_power(power: i32) -> f32 {
    if power >= 90 {
        1.0
    } else {
        0.4 + 0.4 * power.max(0) as f32 / 90.0
    }
}

impl EquineRiderControl {
    pub fn set_jump_power(&self, power: i32) {
        self.jump_scale.store(jump_scale_from_power(power));
    }

    fn take_jump_scale(&self) -> f32 {
        self.jump_scale.swap(0.0)
    }
}

pub(super) async fn tick_ridden_equine<T: Equine>(equine: &T, control: &EquineRiderControl) {
    if !equine.is_saddled() {
        return;
    }
    let living = &equine.get_mob_entity().living_entity;
    let passenger = living.entity.passengers.lock().await.first().cloned();
    let Some(passenger) = passenger else {
        return;
    };
    let Some(player) = passenger.get_player() else {
        return;
    };

    let rider = player.get_entity();
    let yaw = rider.yaw.load();
    living.entity.yaw.store(yaw);
    living.entity.head_yaw.store(yaw);
    living.entity.body_yaw.store(yaw);
    living.entity.pitch.store(rider.pitch.load() * 0.5);

    let input = player.last_input.load(std::sync::atomic::Ordering::Relaxed);
    let sideways = if input & SPlayerInput::LEFT != 0 {
        0.5
    } else if input & SPlayerInput::RIGHT != 0 {
        -0.5
    } else {
        0.0
    };
    let forward = if input & SPlayerInput::FORWARD != 0 {
        1.0
    } else if input & SPlayerInput::BACKWARD != 0 {
        -0.25
    } else {
        0.0
    };
    living
        .movement_input
        .store(Vector3::new(sideways, 0.0, forward));
    living
        .jumping
        .store(false, std::sync::atomic::Ordering::Relaxed);

    if living
        .entity
        .on_ground
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        let jump_scale = control.take_jump_scale();
        if jump_scale <= 0.0 {
            return;
        }
        let mut velocity = living.entity.velocity.load();
        velocity.y = living.get_jump_velocity(f64::from(jump_scale)).await;
        if forward > 0.0 {
            let yaw = f64::from(yaw).to_radians();
            velocity.x += -0.4 * f64::from(jump_scale) * yaw.sin();
            velocity.z += 0.4 * f64::from(jump_scale) * yaw.cos();
        }
        living.entity.velocity.store(velocity);
        living
            .entity
            .velocity_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

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
    fn animation_state(&self) -> Option<&EquineAnimationState> {
        None
    }
    fn sync_equine_flags(&self) {}
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

pub(super) fn open_equine_mouth<T: Equine>(equine: &T) {
    if let Some(animation) = equine.animation_state() {
        animation.open_mouth();
        equine.sync_equine_flags();
    }
}

pub(super) fn make_equine_mad<T: Equine>(equine: &T, sound: Sound) {
    if let Some(animation) = equine.animation_state() {
        if animation.flags() & 0x20 == 0 {
            animation.stand();
            equine.sync_equine_flags();
            let entity = equine.get_entity();
            entity
                .world
                .load()
                .play_sound(sound, SoundCategory::Neutral, &entity.pos.load());
        }
    }
}

pub(super) fn tick_equine_animations<T: Equine>(equine: &T) {
    let Some(animation) = equine.animation_state() else {
        return;
    };
    if animation.tick(
        equine
            .get_entity()
            .on_ground
            .load(std::sync::atomic::Ordering::Relaxed),
    ) {
        equine.sync_equine_flags();
    }
}

pub(super) async fn open_equine_inventory<T: Equine>(equine: &T, player: &Arc<Player>) -> bool {
    if !equine.is_tame() || equine.is_baby() || !player.get_entity().is_sneaking() {
        return false;
    }
    let entity = equine.get_entity();
    let Some(entity) = entity.world.load().get_entity_by_id(entity.entity_id) else {
        return false;
    };
    player.open_mount_screen(entity).await.is_some()
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

/// Abstract horses have a one-in-900 chance each AI tick to recover one health.
pub(super) fn tick_equine_natural_regeneration<T: Equine>(equine: &T) {
    let living = &equine.get_mob_entity().living_entity;
    if living.health.load() > 0.0
        && living.health.load() < living.get_max_health()
        && natural_regeneration_triggers(rand::rng().random_range(0..900))
    {
        living.heal(1.0);
    }
}

const fn natural_regeneration_triggers(roll: i32) -> bool {
    roll == 0
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
    open_equine_mouth(equine);
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
    fn equine_natural_regeneration_uses_vanilla_one_in_900_roll() {
        assert!(natural_regeneration_triggers(0));
        assert!(!natural_regeneration_triggers(1));
        assert!(!natural_regeneration_triggers(899));
    }

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

    #[test]
    fn ridden_jump_charge_matches_vanilla_curve() {
        assert!((jump_scale_from_power(-1) - 0.4).abs() < f32::EPSILON);
        assert!((jump_scale_from_power(0) - 0.4).abs() < f32::EPSILON);
        assert!((jump_scale_from_power(45) - 0.6).abs() < f32::EPSILON);
        assert!((jump_scale_from_power(89) - (0.4 + 0.4 * 89.0 / 90.0)).abs() < f32::EPSILON);
        assert!((jump_scale_from_power(90) - 1.0).abs() < f32::EPSILON);
        assert!((jump_scale_from_power(100) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn abstract_horse_animation_flags_use_vanilla_lifetimes() {
        let animation = EquineAnimationState::default();

        animation.open_mouth();
        assert_eq!(animation.flags(), 0x40);
        for _ in 0..29 {
            assert!(!animation.tick(true));
        }
        assert!(animation.tick(true));
        assert_eq!(animation.flags(), 0);

        animation.stand();
        assert_eq!(animation.flags(), 0x20);
        for _ in 0..10 {
            assert!(!animation.tick(false));
        }
        assert_eq!(animation.flags(), 0x20);
        for _ in 0..19 {
            assert!(!animation.tick(true));
        }
        assert!(animation.tick(true));
        assert_eq!(animation.flags(), 0);
    }
}
