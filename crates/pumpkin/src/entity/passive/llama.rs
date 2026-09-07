use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::{
    data_component_impl::EquipmentSlot,
    entity::EntityType,
    item::Item,
    item_stack::ItemStack,
    sound::Sound,
    tag::{self, Taggable},
    tracked_data,
};
use pumpkin_protocol::java::client::play::Metadata;
use pumpkin_util::random::RandomImpl;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, NBTStorage, NbtFuture,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        active_target::ActiveTargetGoal, breed::BreedGoal, escape_danger::EscapeDangerGoal,
        follow_parent::FollowParentGoal, llama_follow_caravan::LlamaFollowCaravanGoal,
        llama_revenge::LlamaRevengeGoal, llama_spit_attack::LlamaSpitAttackGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal,
        run_around_like_crazy::RunAroundLikeCrazyGoal, swim::SwimGoal, tempt::TemptGoal,
        trader_llama_defend::TraderLlamaDefendGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    player::Player,
};

const TRADER_LLAMA_ZOMBIE_TARGETS: [&EntityType; 4] = [
    &EntityType::ZOMBIE,
    &EntityType::HUSK,
    &EntityType::DROWNED,
    &EntityType::ZOMBIE_VILLAGER,
];
const TRADER_LLAMA_ILLAGER_TARGETS: [&EntityType; 4] = [
    &EntityType::PILLAGER,
    &EntityType::VINDICATOR,
    &EntityType::EVOKER,
    &EntityType::ILLUSIONER,
];

/// Represents a Llama, a neutral mob that can be used for carrying items and spits at enemies.
///
/// Wiki: <https://minecraft.wiki/w/Llama>
pub struct LlamaEntity {
    pub mob_entity: MobEntity,
    ageable_data: AgeableData,
    variant: AtomicI32,
    strength: AtomicI32,
    tamed: AtomicBool,
    temper: AtomicI32,
    owner: AtomicCell<Option<Uuid>>,
    caravan_head: AtomicI32,
    caravan_tail: AtomicI32,
    did_spit: AtomicBool,
    trader_despawn_delay: AtomicI32,
    animation_state: super::horse_food::EquineAnimationState,
    pub chested_horse: super::chested_horse::ChestedHorseData,
}

impl LlamaEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let is_trader = entity.entity_type == &EntityType::TRADER_LLAMA;
        let mob_entity = MobEntity::new(entity);
        let (variant, strength) = {
            let mut rng = mob_entity
                .random
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let strength_max = if rng.next_f32() < 0.04 { 5 } else { 3 };
            (
                rng.next_inbetween_i32(0, 3),
                rng.next_inbetween_i32(1, strength_max),
            )
        };
        let llama = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(variant),
            strength: AtomicI32::new(strength),
            tamed: AtomicBool::new(false),
            temper: AtomicI32::new(0),
            owner: AtomicCell::new(None),
            caravan_head: AtomicI32::new(-1),
            caravan_tail: AtomicI32::new(-1),
            did_spit: AtomicBool::new(false),
            trader_despawn_delay: AtomicI32::new(if is_trader { 47_999 } else { -1 }),
            animation_state: super::horse_food::EquineAnimationState::default(),
            chested_horse: super::chested_horse::ChestedHorseData::default(),
        };
        let mob_arc = Arc::new(llama);
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

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, Box::new(RunAroundLikeCrazyGoal::new()));
            if is_trader {
                goal_selector.add_goal(1, EscapeDangerGoal::new(2.0));
            }
            goal_selector.add_goal(2, Box::new(LlamaFollowCaravanGoal::new(2.1)));
            goal_selector.add_goal(3, Box::new(LlamaSpitAttackGoal::new(1.25)));
            goal_selector.add_goal(3, EscapeDangerGoal::new(1.2));
            goal_selector.add_goal(4, BreedGoal::new(1.0));
            goal_selector.add_goal(5, Box::new(TemptGoal::new(1.25, &[&Item::HAY_BLOCK])));
            goal_selector.add_goal(6, Box::new(FollowParentGoal::new(1.0)));
            goal_selector.add_goal(7, Box::new(WanderAroundGoal::new(0.7)));
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(9, Box::new(RandomLookAroundGoal::default()));
        };

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(1, Box::new(LlamaRevengeGoal::new()));
            if is_trader {
                target_selector.add_goal(1, Box::new(TraderLlamaDefendGoal::new()));
            }
            target_selector.add_goal(
                2,
                Box::new(
                    ActiveTargetGoal::new(
                        &mob_arc.mob_entity,
                        &EntityType::WOLF,
                        16,
                        false,
                        true,
                        Some(
                            |entity_id: i32, world: Arc<crate::world::World>| async move {
                                world.get_entity_by_id(entity_id).and_then(|entity| {
                                    entity.get_mob().and_then(Mob::get_wolf).map(Mob::is_tame)
                                }) == Some(false)
                            },
                        ),
                    )
                    .with_follow_distance_scale(0.25),
                ),
            );
            if is_trader {
                // Vanilla targets Zombie subclasses except Zombified Piglins (explicit
                // predicate), plus every AbstractIllager subclass, at priority 2.
                target_selector.add_goal(
                    2,
                    Box::new(ActiveTargetGoal::new_many(
                        &mob_arc.mob_entity,
                        &TRADER_LLAMA_ZOMBIE_TARGETS,
                        10,
                        true,
                        false,
                    )),
                );
                target_selector.add_goal(
                    2,
                    Box::new(ActiveTargetGoal::new_many(
                        &mob_arc.mob_entity,
                        &TRADER_LLAMA_ILLAGER_TARGETS,
                        10,
                        true,
                        false,
                    )),
                );
            }
        }

        mob_arc
    }

    pub fn variant(&self) -> i32 {
        self.variant.load(Ordering::Relaxed)
    }

    pub fn strength(&self) -> i32 {
        self.strength.load(Ordering::Relaxed)
    }

    fn is_trader_llama(&self) -> bool {
        self.get_entity().entity_type == &EntityType::TRADER_LLAMA
    }

    pub fn set_variant(&self, variant: i32) {
        let variant = variant.clamp(0, 3);
        self.variant.store(variant, Ordering::Relaxed);
        self.get_entity().send_meta_data(
            &[Metadata::new(tracked_data::llama::DATA_VARIANT_ID, variant)],
            None,
        );
    }

    pub(crate) fn in_caravan(&self) -> bool {
        self.caravan_head.load(Ordering::Relaxed) >= 0
    }

    pub(crate) fn caravan_head_id(&self) -> i32 {
        self.caravan_head.load(Ordering::Relaxed)
    }

    pub(crate) fn has_caravan_tail(&self) -> bool {
        self.caravan_tail.load(Ordering::Relaxed) >= 0
    }

    pub(crate) fn did_spit(&self) -> bool {
        self.did_spit.load(Ordering::Relaxed)
    }

    pub(crate) fn set_did_spit(&self, did_spit: bool) {
        self.did_spit.store(did_spit, Ordering::Relaxed);
    }

    pub(crate) fn join_caravan(&self, head_id: i32) {
        self.caravan_head.store(head_id, Ordering::Relaxed);
        if let Some(head) = self.get_entity().world.load().get_entity_by_id(head_id)
            && let Some(head) = head.get_mob().and_then(Mob::get_llama)
        {
            head.caravan_tail
                .store(self.get_entity().entity_id, Ordering::Relaxed);
        }
    }

    pub(crate) fn leave_caravan(&self) {
        let head_id = self.caravan_head.swap(-1, Ordering::Relaxed);
        if let Some(head) = self.get_entity().world.load().get_entity_by_id(head_id)
            && let Some(head) = head.get_mob().and_then(Mob::get_llama)
            && head.caravan_tail.load(Ordering::Relaxed) == self.get_entity().entity_id
        {
            head.caravan_tail.store(-1, Ordering::Relaxed);
        }
    }

    pub fn set_strength(&self, strength: i32) {
        let strength = strength.clamp(1, 5);
        self.strength.store(strength, Ordering::Relaxed);
        self.get_entity().send_meta_data(
            &[Metadata::new(
                tracked_data::llama::DATA_STRENGTH_ID,
                strength,
            )],
            None,
        );
    }

    fn set_tamed(&self, tamed: bool, owner: Option<Uuid>) {
        self.tamed.store(tamed, Ordering::Relaxed);
        self.owner.store(if tamed { owner } else { None });
        self.sync_flags();
    }

    fn sync_flags(&self) {
        let flags = (if self.tamed.load(Ordering::Relaxed) {
            0x02
        } else {
            0
        }) | self.animation_state.flags();
        self.get_entity().send_meta_data(
            &[Metadata::new(
                tracked_data::abstract_horse::DATA_ID_FLAGS,
                flags as i8,
            )],
            None,
        );
    }
}

fn inherited_strength(first: i32, second: i32, roll: i32, mutates: bool) -> i32 {
    let strongest = first.max(second).clamp(1, 5);
    (roll.clamp(0, strongest - 1) + 1 + i32::from(mutates)).clamp(1, 5)
}

fn inherited_variant(first: i32, second: i32, choose_first: bool) -> i32 {
    if choose_first { first } else { second }.clamp(0, 3)
}

fn trader_llama_can_despawn(
    tamed: bool,
    has_exactly_one_player_passenger: bool,
    leash_holder_type: Option<&'static EntityType>,
) -> bool {
    !tamed
        && !has_exactly_one_player_passenger
        && leash_holder_type.is_none_or(|kind| kind == &EntityType::WANDERING_TRADER)
}

fn trader_llama_mount_blocked(
    is_trader_llama: bool,
    leash_holder_type: Option<&'static EntityType>,
) -> bool {
    is_trader_llama && leash_holder_type == Some(&EntityType::WANDERING_TRADER)
}

async fn has_exactly_one_player_passenger(entity: &Entity) -> bool {
    let mut pending = entity.passengers.lock().await.clone();
    let mut player_count = 0;
    while let Some(passenger) = pending.pop() {
        if passenger.get_entity().entity_type == &EntityType::PLAYER {
            player_count += 1;
            if player_count > 1 {
                return false;
            }
        }
        pending.extend(
            passenger
                .get_entity()
                .passengers
                .lock()
                .await
                .iter()
                .cloned(),
        );
    }
    player_count == 1
}

impl AgeableMob for LlamaEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}
impl super::horse_food::Equine for LlamaEntity {
    fn animation_state(&self) -> Option<&super::horse_food::EquineAnimationState> {
        Some(&self.animation_state)
    }

    fn sync_equine_flags(&self) {
        self.sync_flags();
    }

    fn temper(&self) -> i32 {
        self.temper.load(Ordering::Relaxed)
    }

    fn add_temper(&self, amount: i32) {
        self.temper
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some((value + amount).clamp(0, 30))
            })
            .ok();
    }

    fn set_tamed(&self, tamed: bool, owner: Option<Uuid>) {
        LlamaEntity::set_tamed(self, tamed, owner);
    }

    fn max_temper(&self) -> i32 {
        30
    }

    fn food_effect(&self, item: &Item) -> Option<super::horse_food::FoodEffect> {
        match item.id {
            id if id == Item::WHEAT.id => Some(super::horse_food::FoodEffect {
                healing: 2.0,
                growth_seconds: 10,
                temper: 3,
                breeds: false,
            }),
            id if id == Item::HAY_BLOCK.id => Some(super::horse_food::FoodEffect {
                healing: 10.0,
                growth_seconds: 90,
                temper: 6,
                breeds: true,
            }),
            _ => None,
        }
    }
}

impl super::animal::Animal for LlamaEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item == &Item::HAY_BLOCK
    }
}
impl NBTStorage for LlamaEntity {
    fn write_nbt<'a>(
        &'a self,
        nbt: &'a mut pumpkin_nbt::compound::NbtCompound,
    ) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.write_nbt(nbt).await;
            self.write_ageable_nbt(nbt);
            super::animal::Animal::write_animal_nbt(self, nbt);
            super::horse_food::write_equine_state_nbt(self, nbt);
            nbt.put_int("Variant", self.variant.load(Ordering::Relaxed));
            nbt.put_int("Strength", self.strength.load(Ordering::Relaxed));
            nbt.put_bool("Tame", self.tamed.load(Ordering::Relaxed));
            nbt.put_int("Temper", self.temper.load(Ordering::Relaxed));
            if let Some(owner) = self.owner.load() {
                nbt.put_uuid("Owner", owner);
            }
            self.chested_horse.write_nbt(nbt).await;
            if self.is_trader_llama() {
                nbt.put_int(
                    "DespawnDelay",
                    self.trader_despawn_delay.load(Ordering::Relaxed),
                );
            }
        })
    }
    fn read_nbt_non_mut<'a>(
        &'a self,
        nbt: &'a pumpkin_nbt::compound::NbtCompound,
    ) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            super::chested_horse::sanitize_body_equipment(
                &self.mob_entity,
                super::chested_horse::MountBodySlotKind::LlamaDecor,
            )
            .await;
            self.read_ageable_nbt(nbt);
            super::animal::Animal::read_animal_nbt(self, nbt);
            super::horse_food::read_equine_state_nbt(self, nbt);
            if let Some(variant) = nbt.get_int("Variant") {
                self.set_variant(variant);
            }
            if let Some(strength) = nbt.get_int("Strength") {
                self.set_strength(strength);
            }
            self.temper.store(
                nbt.get_int("Temper").unwrap_or(0).clamp(0, 30),
                Ordering::Relaxed,
            );
            self.set_tamed(nbt.get_bool("Tame").unwrap_or(false), nbt.get_uuid("Owner"));
            self.chested_horse.read_nbt(self, nbt).await;
            if self.is_trader_llama()
                && let Some(delay) = nbt.get_int("DespawnDelay")
            {
                self.trader_despawn_delay.store(delay, Ordering::Relaxed);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TRADER_LLAMA_ILLAGER_TARGETS, TRADER_LLAMA_ZOMBIE_TARGETS, inherited_strength,
        inherited_variant, trader_llama_can_despawn, trader_llama_mount_blocked,
    };
    use pumpkin_data::entity::EntityType;

    #[test]
    fn llama_state_ranges() {
        assert_eq!(0.clamp(0, 3), 0);
        assert_eq!(9.clamp(0, 3), 3);
        assert_eq!((-2).clamp(1, 5), 1);
        assert_eq!(9.clamp(1, 5), 5);
    }

    #[test]
    fn llama_inheritance_uses_strongest_parent_and_rare_mutation() {
        assert_eq!(inherited_strength(2, 4, 0, false), 1);
        assert_eq!(inherited_strength(2, 4, 3, false), 4);
        assert_eq!(inherited_strength(2, 4, 3, true), 5);
        assert_eq!(inherited_strength(5, 5, 4, true), 5);
        assert_eq!(inherited_variant(1, 3, true), 1);
        assert_eq!(inherited_variant(1, 3, false), 3);
    }

    #[test]
    fn trader_llama_despawn_exemptions_match_vanilla() {
        assert!(trader_llama_can_despawn(false, false, None));
        assert!(trader_llama_can_despawn(
            false,
            false,
            Some(&EntityType::WANDERING_TRADER)
        ));
        assert!(!trader_llama_can_despawn(true, false, None));
        assert!(!trader_llama_can_despawn(false, true, None));
        assert!(!trader_llama_can_despawn(
            false,
            false,
            Some(&EntityType::PLAYER)
        ));
    }

    #[test]
    fn only_the_wandering_trader_leash_blocks_mounting() {
        assert!(trader_llama_mount_blocked(
            true,
            Some(&EntityType::WANDERING_TRADER)
        ));
        assert!(!trader_llama_mount_blocked(true, None));
        assert!(!trader_llama_mount_blocked(true, Some(&EntityType::PLAYER)));
        assert!(!trader_llama_mount_blocked(
            false,
            Some(&EntityType::WANDERING_TRADER)
        ));
    }

    #[test]
    fn trader_llama_hostile_families_match_1_21_4() {
        assert!(TRADER_LLAMA_ZOMBIE_TARGETS.contains(&&EntityType::DROWNED));
        assert!(!TRADER_LLAMA_ZOMBIE_TARGETS.contains(&&EntityType::ZOMBIFIED_PIGLIN));
        assert!(TRADER_LLAMA_ILLAGER_TARGETS.contains(&&EntityType::EVOKER));
        assert!(TRADER_LLAMA_ILLAGER_TARGETS.contains(&&EntityType::ILLUSIONER));
    }
}

impl Mob for LlamaEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }
    fn is_tame(&self) -> bool {
        self.tamed.load(Ordering::Relaxed)
    }
    fn can_mate_with<'a>(&'a self, mate: &'a dyn EntityBase) -> EntityBaseFuture<'a, bool> {
        Box::pin(async move {
            super::horse_food::can_equine_mate(
                self,
                mate,
                mate.get_entity().entity_type == &EntityType::LLAMA,
            )
            .await
        })
    }
    fn tick_untamed_riding<'a>(&'a self) -> EntityBaseFuture<'a, ()> {
        Box::pin(super::horse_food::tick_untamed_riding(self))
    }
    fn get_llama(&self) -> Option<&LlamaEntity> {
        Some(self)
    }
    fn create_mount_inventory(
        &self,
        entity: Arc<dyn EntityBase>,
    ) -> Option<Arc<dyn pumpkin_world::inventory::Inventory>> {
        let has_chest = self.chested_horse.has_chest();
        Some(Arc::new(super::chested_horse::MountInventory::new(
            entity,
            has_chest.then(|| self.chested_horse.inventory.clone()),
            if has_chest {
                (self.strength().clamp(1, 5) * 3) as usize
            } else {
                0
            },
        )))
    }
    fn mob_on_death<'a>(&'a self, _cause: Option<&'a dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            super::chested_horse::drop_mount_inventory_on_death(
                &self.mob_entity,
                Some(&self.chested_horse),
            )
            .await;
        })
    }
    fn configure_bred_child<'a>(
        &'a self,
        mate: &'a dyn EntityBase,
        child: &'a Arc<dyn EntityBase>,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            let (Some(mate), Some(child)) = (
                mate.get_mob().and_then(Mob::get_llama),
                child.get_mob().and_then(Mob::get_llama),
            ) else {
                return;
            };
            let strongest = self.strength().max(mate.strength()).clamp(1, 5);
            let mut rng = self.get_entity_random();
            child.set_strength(inherited_strength(
                self.strength(),
                mate.strength(),
                rng.next_bounded_i32(strongest),
                rng.next_f32() < 0.03,
            ));
            child.set_variant(inherited_variant(
                self.variant(),
                mate.variant(),
                rng.next_bool(),
            ));
        })
    }
    fn mob_tick<'a>(&'a self, caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            self.ageable_ai_step();
            super::horse_food::tick_equine_animations(self);
            super::horse_food::tick_equine_natural_regeneration(self);
            if self.is_trader_llama() {
                let entity = self.get_entity();
                let leash_holder = entity.leashed_to.lock().await.clone();
                let has_exactly_one_player_passenger =
                    has_exactly_one_player_passenger(entity).await;
                if trader_llama_can_despawn(
                    self.is_tame(),
                    has_exactly_one_player_passenger,
                    leash_holder
                        .as_ref()
                        .map(|holder| holder.get_entity().entity_type),
                ) {
                    let delay = if let Some(trader) = leash_holder
                        .as_ref()
                        .and_then(|holder| holder.get_mob())
                        .and_then(Mob::get_wandering_trader)
                    {
                        let delay = trader.despawn_delay() - 1;
                        self.trader_despawn_delay.store(delay, Ordering::Relaxed);
                        delay
                    } else {
                        self.trader_despawn_delay.fetch_sub(1, Ordering::Relaxed) - 1
                    };
                    if delay <= 0 {
                        entity.unleash().await;
                        entity.world.load().remove_entity(caller.as_ref()).await;
                    }
                }
            }
        })
    }
    fn mob_interact<'a>(
        &'a self,
        player: &'a Arc<Player>,
        stack: &'a mut ItemStack,
    ) -> EntityBaseFuture<'a, bool> {
        Box::pin(async move {
            if super::horse_food::open_equine_inventory(self, player).await {
                return true;
            }
            if super::horse_food::feed_equine(self, player, stack).await {
                return true;
            }
            if super::horse_food::should_make_untamed_equine_mad(
                self.is_tame(),
                stack.is_empty(),
                super::horse_food::is_equine_food(self, stack.item),
            ) {
                super::horse_food::make_equine_mad(self);
                return true;
            }
            if self.is_tame()
                && !self.is_baby()
                && self
                    .chested_horse
                    .try_attach(self, player, stack, Sound::EntityLlamaChest)
                    .await
            {
                return true;
            }
            if self.is_tame()
                && !self.is_baby()
                && stack.item.has_tag(&tag::Item::MINECRAFT_WOOL_CARPETS)
            {
                let living = &self.mob_entity.living_entity;
                let mut equipment = living.entity_equipment.lock().await;
                if equipment.get(&EquipmentSlot::BODY).is_empty() {
                    let decor = stack.copy_with_count(1);
                    equipment.put(&EquipmentSlot::BODY, decor.clone());
                    drop(equipment);
                    stack.decrement_unless_creative(player.gamemode.load(), 1);
                    living.send_equipment_changes(&[(EquipmentSlot::BODY, decor)]);
                    let entity = self.get_entity();
                    entity.world.load().play_sound(
                        Sound::EntityLlamaSwag,
                        pumpkin_data::sound::SoundCategory::Neutral,
                        &entity.pos.load(),
                    );
                    return true;
                }
            }
            let leash_holder = self.get_entity().leashed_to.lock().await.clone();
            if trader_llama_mount_blocked(
                self.is_trader_llama(),
                leash_holder
                    .as_ref()
                    .map(|holder| holder.get_entity().entity_type),
            ) {
                // AbstractHorse.mobInteract still returns SUCCESS after the
                // subclass declines to attach the rider.
                return true;
            }
            if super::horse_food::mount_equine(self, player).await {
                return true;
            }
            self.mob_entity.mob_interact(player, stack).await
        })
    }
}
