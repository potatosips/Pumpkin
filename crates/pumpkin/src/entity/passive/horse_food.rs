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
    tag::{self, Taggable},
};
use pumpkin_protocol::bedrock::server::actor_event::ActorEventType;
use pumpkin_protocol::java::server::play::SPlayerInput;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::random::{RandomGenerator, RandomImpl};
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
    grass_counter: AtomicI32,
}

impl Default for EquineAnimationState {
    fn default() -> Self {
        Self {
            flags: AtomicU8::new(0),
            mouth_counter: AtomicI32::new(0),
            stand_counter: AtomicI32::new(0),
            grass_counter: AtomicI32::new(0),
        }
    }
}

impl EquineAnimationState {
    pub fn flags(&self) -> u8 {
        self.flags.load(Ordering::Relaxed)
    }

    fn set_persisted_flags(&self, eating: bool, bred: bool) {
        self.grass_counter.store(0, Ordering::Relaxed);
        self.flags
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |flags| {
                Some((flags & !0x18) | u8::from(eating) * 0x10 | u8::from(bred) * 0x08)
            })
            .ok();
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

    fn eat_grass(&self) {
        self.flags.fetch_or(0x10, Ordering::Relaxed);
    }

    fn stop_mount_poses(&self) -> bool {
        let previous = self.flags.fetch_and(!0x30, Ordering::Relaxed);
        previous & 0x30 != 0
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
        if self.flags() & 0x10 != 0 {
            let next = self.grass_counter.load(Ordering::Relaxed) + 1;
            if next > 50 {
                self.grass_counter.store(0, Ordering::Relaxed);
                self.flags.fetch_and(!0x10, Ordering::Relaxed);
                changed = true;
            } else {
                self.grass_counter.store(next, Ordering::Relaxed);
            }
        }
        changed
    }
}

pub(super) fn write_equine_state_nbt<T: Equine>(
    equine: &T,
    nbt: &mut pumpkin_nbt::compound::NbtCompound,
) {
    let flags = equine
        .animation_state()
        .map_or(0, EquineAnimationState::flags);
    nbt.put_bool("EatingHaystack", flags & 0x10 != 0);
    nbt.put_bool("Bred", flags & 0x08 != 0);
}

pub(super) fn read_equine_state_nbt<T: Equine>(
    equine: &T,
    nbt: &pumpkin_nbt::compound::NbtCompound,
) {
    if let Some(animation) = equine.animation_state() {
        animation.set_persisted_flags(
            nbt.get_bool("EatingHaystack").unwrap_or(false),
            nbt.get_bool("Bred").unwrap_or(false),
        );
        equine.sync_equine_flags();
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
        living.entity.world.load().play_sound_fine(
            equine.jump_sound(),
            SoundCategory::Neutral,
            &living.entity.pos.load(),
            0.4,
            1.0,
        );
    }
}

use crate::entity::{
    EntityBase,
    ageable::AgeableMob,
    living::LivingEntity,
    mob::{Mob, MobEntity},
    player::Player,
};

/// Items used by the priority-three horse/chested-horse TemptGoal in Java 1.21.4.
/// SkeletonHorse and ZombieHorse override the behavior-goal hook with no-op.
pub(super) const HORSE_TEMPT_ITEMS: &[&Item] = &[
    &Item::SUGAR,
    &Item::WHEAT,
    &Item::APPLE,
    &Item::GOLDEN_CARROT,
    &Item::GOLDEN_APPLE,
    &Item::ENCHANTED_GOLDEN_APPLE,
    &Item::HAY_BLOCK,
];

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

pub(super) async fn can_equine_parent(mob: &dyn Mob) -> bool {
    let entity = mob.get_entity();
    let Some(living) = mob.get_living_entity() else {
        return false;
    };
    equine_parent_allowed(
        entity.has_passengers().await,
        entity.has_vehicle().await,
        mob.is_tame(),
        entity.age.load(std::sync::atomic::Ordering::Relaxed),
        living.health.load(),
        living.get_max_health(),
        mob.get_mob_entity().is_in_love(),
    )
}

const fn equine_parent_allowed(
    is_vehicle: bool,
    is_passenger: bool,
    tame: bool,
    age: i32,
    health: f32,
    max_health: f32,
    in_love: bool,
) -> bool {
    !is_vehicle && !is_passenger && tame && age >= 0 && health >= max_health && in_love
}

pub(super) async fn can_equine_mate(mob: &dyn Mob, mate: &dyn EntityBase, accepted: bool) -> bool {
    if !accepted || mate.get_entity().entity_uuid == mob.get_entity().entity_uuid {
        return false;
    }
    let Some(mate_mob) = mate.get_mob() else {
        return false;
    };
    can_equine_parent(mob).await && can_equine_parent(mate_mob).await
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

fn chested_horse_max_health(first_roll: i32, second_roll: i32) -> f64 {
    15.0 + f64::from(first_roll.clamp(0, 7)) + f64::from(second_roll.clamp(0, 8))
}

pub(super) fn randomize_chested_horse_health(mob: &MobEntity) {
    let max_health = {
        let mut rng = mob
            .random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        chested_horse_max_health(rng.next_bounded_i32(8), rng.next_bounded_i32(9))
    };
    mob.living_entity
        .set_attribute_base(&Attributes::MAX_HEALTH, max_health);
    mob.living_entity.health.store(max_health as f32);
}

pub(super) fn configure_bred_equine_attributes(
    first: &LivingEntity,
    mate: &dyn EntityBase,
    child: &Arc<dyn EntityBase>,
    rng: &mut RandomGenerator,
) {
    let (Some(second), Some(child)) = (mate.get_living_entity(), child.get_living_entity()) else {
        return;
    };
    for (attribute, min, max) in [
        (&Attributes::MAX_HEALTH, 15.0, 30.0),
        (&Attributes::JUMP_STRENGTH, 0.4, 1.0),
        (&Attributes::MOVEMENT_SPEED, 0.1125, 0.3375),
    ] {
        let offset = (rng.next_f64() + rng.next_f64() + rng.next_f64()) / 3.0 - 0.5;
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
    fn jump_sound(&self) -> Sound {
        Sound::EntityHorseJump
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

pub(super) fn make_equine_mad<T: Equine>(equine: &T) {
    if let Some(animation) = equine.animation_state() {
        if animation.flags() & 0x20 == 0 {
            animation.stand();
            equine.sync_equine_flags();
            let entity = equine.get_entity();
            entity.world.load().play_sound(
                equine_angry_sound(entity.entity_type),
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
        }
    }
}

pub(super) fn react_to_equine_damage<T: Equine>(equine: &T) {
    let on_ground = equine
        .get_entity()
        .on_ground
        .load(std::sync::atomic::Ordering::Relaxed);
    if damage_rearing_triggers(equine.get_entity_random().next_bounded_i32(3), on_ground) {
        if let Some(animation) = equine.animation_state() {
            animation.stand();
            equine.sync_equine_flags();
        }
    }
}

const fn damage_rearing_triggers(roll: i32, on_ground: bool) -> bool {
    roll == 0 && on_ground
}

pub(super) async fn can_equine_ambient_stand<T: Equine>(equine: &T) -> bool {
    let Some(animation) = equine.animation_state() else {
        return false;
    };
    let Some(living) = equine.get_living_entity() else {
        return false;
    };
    let dead_or_dying =
        living.health.load() <= 0.0 || living.dead.load(std::sync::atomic::Ordering::Relaxed);
    ambient_stand_allowed(
        animation.flags(),
        dead_or_dying,
        equine.is_saddled(),
        equine.get_entity().has_passengers().await,
    )
}

const fn ambient_stand_allowed(
    animation_flags: u8,
    dead_or_dying: bool,
    saddled: bool,
    has_passengers: bool,
) -> bool {
    // AbstractHorse.isImmobile is (dead/dying && vehicle && saddled), eating,
    // or standing. RandomStandGoal starts only when that whole predicate is false.
    animation_flags & 0x30 == 0 && !(dead_or_dying && has_passengers && saddled)
}

pub(super) fn start_equine_ambient_stand<T: Equine>(equine: &T, sound: Sound) {
    if let Some(animation) = equine.animation_state() {
        animation.stand();
        equine.sync_equine_flags();
    }
    let entity = equine.get_entity();
    entity
        .world
        .load()
        .play_sound(sound, SoundCategory::Neutral, &entity.pos.load());
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

/// AbstractHorse.aiStep starts the grazing pose one tick in 300 while an
/// unridden horse stands on a block in #animals_spawnable_on. It is visual
/// behavior only; unlike EatGrassGoal it does not alter the block.
pub(super) async fn try_start_equine_grazing<T: Equine>(equine: &T) {
    let Some(animation) = equine.animation_state() else {
        return;
    };
    if animation.flags() & 0x10 != 0
        || equine.get_entity().has_passengers().await
        || !grazing_roll_triggers(equine.get_entity_random().next_bounded_i32(300))
    {
        return;
    }
    let entity = equine.get_entity();
    let block = entity
        .world
        .load()
        .get_block(&entity.block_pos.load().down());
    if block.has_tag(&tag::Block::MINECRAFT_ANIMALS_SPAWNABLE_ON) {
        animation.eat_grass();
        equine.sync_equine_flags();
    }
}

const fn grazing_roll_triggers(roll: i32) -> bool {
    roll == 0
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
    if equine
        .get_mob_entity()
        .navigator
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_idle()
    {
        return;
    }
    if equine.get_entity_random().next_bounded_i32(50) != 0 {
        return;
    }
    if let Some(player_entity) = passenger.get_player() {
        let max_temper = equine.max_temper();
        let success = max_temper > 0
            && taming_succeeds(
                equine.temper(),
                max_temper,
                equine.get_entity_random().next_bounded_i32(max_temper),
            );
        if success {
            let player = entity
                .world
                .load()
                .get_player_by_id(player_entity.entity_id());
            if let Some(player) = player {
                let mut event =
                    crate::plugin::api::events::entity::entity_tame::EntityTameEvent::new(
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
        }
        equine.add_temper(5);
    }

    let passenger_ids: Vec<i32> = entity
        .passengers
        .lock()
        .await
        .iter()
        .map(|passenger| passenger.get_entity().entity_id)
        .collect();
    for passenger_id in passenger_ids {
        entity.remove_passenger(passenger_id).await;
    }
    make_equine_mad(equine);
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
        && natural_regeneration_triggers(equine.get_entity_random().next_bounded_i32(900))
    {
        living.heal(1.0);
    }
}

const fn natural_regeneration_triggers(roll: i32) -> bool {
    roll == 0
}

pub(super) async fn mount_equine<T: Equine>(equine: &T, player: &Arc<Player>) -> bool {
    if !can_mount_equine(equine.is_baby(), equine.get_entity().has_passengers().await) {
        return false;
    }
    let world = player.world();
    let entity = equine.get_entity();
    if let Some(vehicle) = world.get_entity_by_id(entity.entity_id)
        && let Some(passenger) = world.get_player_by_id(player.entity_id())
    {
        if let Some(animation) = equine.animation_state()
            && animation.stop_mount_poses()
        {
            equine.sync_equine_flags();
        }
        passenger
            .get_entity()
            .set_rotation(entity.yaw.load(), entity.pitch.load());
        entity
            .add_passenger(vehicle, passenger as Arc<dyn EntityBase>)
            .await;
        return true;
    }
    false
}

const fn can_mount_equine(is_baby: bool, has_passengers: bool) -> bool {
    !is_baby && !has_passengers
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

pub(super) fn is_equine_food<T: Equine>(equine: &T, item: &Item) -> bool {
    equine.food_effect(item).is_some()
}

pub(super) const fn should_make_untamed_equine_mad(
    is_tamed: bool,
    stack_is_empty: bool,
    is_food: bool,
) -> bool {
    !is_tamed && !stack_is_empty && !is_food
}

pub async fn feed_equine<T: Equine>(
    equine: &T,
    player: &Arc<Player>,
    stack: &mut ItemStack,
) -> bool {
    let Some(effect) = equine.food_effect(stack.item) else {
        return false;
    };
    let living = &equine.get_mob_entity().living_entity;
    let can_heal = living.health.load() < living.get_max_health();
    let can_grow = equine.is_baby();
    let can_breed = effect.breeds
        && equine.can_breed()
        && equine.is_tame()
        && equine.get_age() == 0
        && !equine.get_mob_entity().is_in_love();
    let can_gain_temper = effect.temper > 0
        && equine.temper() < equine.max_temper()
        && food_temper_applies(equine.is_tame(), can_heal || can_grow || can_breed);
    if !can_heal && !can_grow && !can_gain_temper && !can_breed {
        return false;
    }
    let entity = equine.get_entity();
    let pos = entity.pos.load();
    let world = entity.world.load();
    stack.decrement_unless_creative(player.gamemode.load(), 1);
    if can_heal {
        living.heal(effect.healing);
    }
    if can_grow {
        equine.age_up(effect.growth_seconds, false);
        let mut random = equine.get_entity_random();
        let particle_pos = horse_growth_particle_position(
            pos,
            f64::from(entity.width()),
            f64::from(entity.height()),
            random.next_f64(),
            random.next_f64(),
            random.next_f64(),
        );
        world.spawn_particle(
            particle_pos,
            Vector3::new(0.0, 0.0, 0.0),
            0.0,
            1,
            Particle::HappyVillager,
        );
    }
    if can_gain_temper {
        equine.add_temper(effect.temper);
    }
    open_equine_mouth(equine);
    let mut random = equine.get_entity_random();
    world.play_sound_fine(
        equine_eating_sound(entity.entity_type),
        SoundCategory::Neutral,
        &pos,
        1.0,
        equine_eating_pitch(random.next_f32(), random.next_f32()),
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
    }
    true
}

const fn food_temper_applies(is_tame: bool, another_effect_applied: bool) -> bool {
    another_effect_applied || !is_tame
}

fn horse_growth_particle_position(
    position: Vector3<f64>,
    width: f64,
    height: f64,
    x_roll: f64,
    y_roll: f64,
    z_roll: f64,
) -> Vector3<f64> {
    Vector3::new(
        position.x + (x_roll - 0.5) * width,
        position.y + y_roll * height + 0.5,
        position.z + (z_roll - 0.5) * width,
    )
}

fn equine_eating_sound(entity_type: &EntityType) -> Sound {
    match entity_type.id {
        id if id == EntityType::DONKEY.id => Sound::EntityDonkeyEat,
        id if id == EntityType::MULE.id => Sound::EntityMuleEat,
        id if id == EntityType::LLAMA.id || id == EntityType::TRADER_LLAMA.id => {
            Sound::EntityLlamaEat
        }
        _ => Sound::EntityHorseEat,
    }
}

fn equine_eating_pitch(first_roll: f32, second_roll: f32) -> f32 {
    1.0 + (first_roll - second_roll) * 0.2
}

fn equine_angry_sound(entity_type: &EntityType) -> Sound {
    match entity_type.id {
        id if id == EntityType::DONKEY.id => Sound::EntityDonkeyAngry,
        id if id == EntityType::MULE.id => Sound::EntityMuleAngry,
        id if id == EntityType::LLAMA.id || id == EntityType::TRADER_LLAMA.id => {
            Sound::EntityLlamaAngry
        }
        _ => Sound::EntityHorseAngry,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abstract_horse_tempt_items_match_vanilla_food_predicate() {
        assert_eq!(
            HORSE_TEMPT_ITEMS
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            [
                Item::SUGAR.id,
                Item::WHEAT.id,
                Item::APPLE.id,
                Item::GOLDEN_CARROT.id,
                Item::GOLDEN_APPLE.id,
                Item::ENCHANTED_GOLDEN_APPLE.id,
                Item::HAY_BLOCK.id,
            ]
        );
    }

    #[test]
    fn vanilla_chested_horse_health_uses_two_bounded_integer_rolls() {
        assert_eq!(chested_horse_max_health(0, 0), 15.0);
        assert_eq!(chested_horse_max_health(7, 8), 30.0);
        assert_eq!(chested_horse_max_health(3, 4), 22.0);
    }

    #[test]
    fn equine_natural_regeneration_uses_vanilla_one_in_900_roll() {
        assert!(natural_regeneration_triggers(0));
        assert!(!natural_regeneration_triggers(1));
        assert!(!natural_regeneration_triggers(899));
    }

    #[test]
    fn equine_grazing_uses_vanilla_one_in_300_roll() {
        assert!(grazing_roll_triggers(0));
        assert!(!grazing_roll_triggers(1));
        assert!(!grazing_roll_triggers(299));
    }

    #[test]
    fn mounting_clears_grazing_and_rearing_flags() {
        let animation = EquineAnimationState::default();
        animation.stand();
        animation.eat_grass();
        assert_eq!(animation.flags() & 0x30, 0x30);
        assert!(animation.stop_mount_poses());
        assert_eq!(animation.flags() & 0x30, 0);
        assert!(!animation.stop_mount_poses());
        assert!(can_mount_equine(false, false));
        assert!(!can_mount_equine(true, false));
        assert!(!can_mount_equine(false, true));
    }

    #[test]
    fn opening_mouth_sets_only_the_vanilla_mouth_flag() {
        let animation = EquineAnimationState::default();
        animation.open_mouth();
        assert_eq!(animation.flags(), 0x40);
    }

    #[test]
    fn persisted_equine_flags_use_vanilla_bits() {
        let animation = EquineAnimationState::default();
        animation.set_persisted_flags(true, true);
        assert_eq!(animation.flags() & 0x18, 0x18);
        animation.set_persisted_flags(false, false);
        assert_eq!(animation.flags() & 0x18, 0);
    }

    #[test]
    fn equine_interaction_sounds_follow_each_vanilla_subclass() {
        assert_eq!(
            equine_eating_sound(&EntityType::HORSE),
            Sound::EntityHorseEat
        );
        assert_eq!(
            equine_eating_sound(&EntityType::DONKEY),
            Sound::EntityDonkeyEat
        );
        assert_eq!(equine_eating_sound(&EntityType::MULE), Sound::EntityMuleEat);
        assert_eq!(
            equine_eating_sound(&EntityType::LLAMA),
            Sound::EntityLlamaEat
        );
        assert_eq!(
            equine_angry_sound(&EntityType::MULE),
            Sound::EntityMuleAngry
        );
        assert_eq!(equine_eating_pitch(0.0, 1.0), 0.8);
        assert_eq!(equine_eating_pitch(1.0, 0.0), 1.2);
    }

    #[test]
    fn horse_growth_particle_uses_vanilla_random_entity_bounds() {
        let base = Vector3::new(10.0, 20.0, 30.0);
        assert_eq!(
            horse_growth_particle_position(base, 2.0, 1.5, 0.0, 0.0, 1.0),
            Vector3::new(9.0, 20.5, 31.0)
        );
    }

    #[test]
    fn food_temper_matches_vanilla_changed_or_untamed_gate() {
        assert!(food_temper_applies(false, false));
        assert!(food_temper_applies(true, true));
        assert!(!food_temper_applies(true, false));
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

    #[test]
    fn ineffective_horse_food_does_not_trigger_angry_rearing() {
        assert!(!should_make_untamed_equine_mad(false, false, true));
        assert!(should_make_untamed_equine_mad(false, false, false));
        assert!(!should_make_untamed_equine_mad(false, true, false));
        assert!(!should_make_untamed_equine_mad(true, false, false));
    }

    #[test]
    fn accepted_damage_rears_one_in_three_only_on_ground() {
        assert!(damage_rearing_triggers(0, true));
        assert!(!damage_rearing_triggers(1, true));
        assert!(!damage_rearing_triggers(2, true));
        assert!(!damage_rearing_triggers(0, false));
    }

    #[test]
    fn ambient_stand_uses_the_complete_abstract_horse_immobility_rule() {
        assert!(ambient_stand_allowed(0, false, false, false));
        assert!(!ambient_stand_allowed(0x10, false, false, false));
        assert!(!ambient_stand_allowed(0x20, false, false, false));
        assert!(!ambient_stand_allowed(0, true, true, true));
        assert!(ambient_stand_allowed(0, true, false, true));
        assert!(ambient_stand_allowed(0, true, true, false));
    }

    #[test]
    fn equine_parent_requires_vanillas_complete_can_parent_state() {
        assert!(equine_parent_allowed(
            false, false, true, 0, 20.0, 20.0, true
        ));
        assert!(!equine_parent_allowed(
            true, false, true, 0, 20.0, 20.0, true
        ));
        assert!(!equine_parent_allowed(
            false, true, true, 0, 20.0, 20.0, true
        ));
        assert!(!equine_parent_allowed(
            false, false, false, 0, 20.0, 20.0, true
        ));
        assert!(!equine_parent_allowed(
            false, false, true, -1, 20.0, 20.0, true
        ));
        assert!(!equine_parent_allowed(
            false, false, true, 0, 19.0, 20.0, true
        ));
        assert!(!equine_parent_allowed(
            false, false, true, 0, 20.0, 20.0, false
        ));
    }
}
