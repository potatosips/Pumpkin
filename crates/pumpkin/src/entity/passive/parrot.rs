use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::damage::DamageType;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::{EntityStatus, EntityType};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::bedrock::server::actor_event::ActorEventType;
use pumpkin_protocol::java::client::play::Metadata;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, NBTStorage, NbtFuture,
    ai::goal::{
        follow_owner::FollowOwnerGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, swim::SwimGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    player::Player,
};

/// Duration in ticks of the poison a parrot gets from eating a cookie, matching
/// vanilla `Parrot.mobInteract`.
const COOKIE_POISON_DURATION: i32 = 900;

/// Represents a Parrot, a passive flying mob that can mimic nearby mob sounds.
///
/// Wiki: <https://minecraft.wiki/w/Parrot>
pub struct ParrotEntity {
    pub mob_entity: MobEntity,
    is_tame: AtomicBool,
    is_sitting: AtomicBool,
    owner: AtomicCell<Option<Uuid>>,
}

impl ParrotEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let parrot = Self {
            mob_entity,
            is_tame: AtomicBool::new(false),
            is_sitting: AtomicBool::new(false),
            owner: AtomicCell::new(None),
        };
        let mob_arc = Arc::new(parrot);
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
            goal_selector.add_goal(1, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(3, FollowOwnerGoal::new(1.0, 5.0, 1.0));
            goal_selector.add_goal(
                2,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(3, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    fn tame_flags(&self) -> u8 {
        u8::from(self.is_sitting.load(Ordering::Relaxed))
            | if self.is_tame.load(Ordering::Relaxed) {
                0x04
            } else {
                0
            }
    }

    fn sync_tame_flags(&self) {
        self.get_entity().send_meta_data(
            &[Metadata::new(
                pumpkin_data::tracked_data::parrot::TAMEABLE_FLAGS,
                self.tame_flags(),
            )],
            None,
        );
    }

    /// Feeds the parrot a cookie: it is poisoned and then killed, as in vanilla
    /// `Parrot.mobInteract`.
    async fn eat_cookie(&self, player: &Arc<Player>, item_stack: &mut ItemStack) {
        item_stack.decrement_unless_creative(player.gamemode.load(), 1);

        self.mob_entity
            .living_entity
            .add_effect(pumpkin_data::potion::Effect {
                effect_type: &StatusEffect::POISON,
                duration: COOKIE_POISON_DURATION,
                amplifier: 0,
                ambient: false,
                show_particles: true,
                show_icon: true,
                blend: true,
            })
            .await;

        // Vanilla guards this call with `player.isCreative() || !this.isInvulnerable()`,
        // but `hurt` re-checks invulnerability itself and `player_attack` doesn't bypass
        // it, so the guard only skips a call that would do nothing anyway.
        self.damage_with_context(
            self,
            f32::MAX,
            DamageType::PLAYER_ATTACK,
            None,
            Some(player.as_ref()),
            Some(player.as_ref()),
        )
        .await;
    }
}

impl NBTStorage for ParrotEntity {
    fn write_nbt<'a>(&'a self, nbt: &'a mut NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async {
            self.mob_entity.write_nbt(nbt).await;
            nbt.put_bool("IsTame", self.is_tame.load(Ordering::Relaxed));
            nbt.put_bool("Sitting", self.is_sitting.load(Ordering::Relaxed));
            if let Some(owner) = self.owner.load() {
                nbt.put_uuid("Owner", owner);
            }
        })
    }

    fn read_nbt_non_mut<'a>(&'a self, nbt: &'a NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            if let Some(owner) = nbt.get_uuid("Owner") {
                self.owner.store(Some(owner));
                self.is_tame.store(true, Ordering::Relaxed);
            } else if let Some(tame) = nbt.get_bool("IsTame") {
                self.is_tame.store(tame, Ordering::Relaxed);
            }
            if let Some(sitting) = nbt.get_bool("Sitting") {
                self.is_sitting.store(sitting, Ordering::Relaxed);
            }
        })
    }
}

impl Mob for ParrotEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_owner_uuid(&self) -> Option<Uuid> {
        self.owner.load()
    }

    fn is_sitting(&self) -> bool {
        self.is_sitting.load(Ordering::Relaxed)
    }

    fn is_tame(&self) -> bool {
        self.is_tame.load(Ordering::Relaxed)
    }

    fn mob_init_data_tracker(&self) -> EntityBaseFuture<'_, ()> {
        Box::pin(async move {
            let entity = self.get_entity();
            let tame_flags = self.tame_flags();
            entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::parrot::TAMEABLE_FLAGS,
                    tame_flags,
                )],
                None,
            );
            entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::parrot::OWNER_UUID,
                    self.owner.load(),
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
            if !self.is_tame.load(Ordering::Relaxed)
                && item_stack
                    .get_item()
                    .has_tag(&tag::Item::MINECRAFT_PARROT_FOOD)
            {
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                if rand::rng().random_range(0..10) == 0 {
                    let entity = self.get_entity();
                    let mut event =
                        crate::plugin::api::events::entity::entity_tame::EntityTameEvent::new(
                            entity.entity_id,
                            player.clone(),
                        );
                    if let Some(server) = entity.world.load().server.upgrade() {
                        server.plugin_manager.fire(&server, &mut event).await;
                    }
                    if event.cancelled {
                        return true;
                    }
                    self.is_tame.store(true, Ordering::Relaxed);
                    self.owner.store(Some(player.gameprofile.id));
                    self.sync_tame_flags();
                    entity.send_meta_data(
                        &[Metadata::new(
                            pumpkin_data::tracked_data::parrot::OWNER_UUID,
                            Some(player.gameprofile.id),
                        )],
                        None,
                    );
                    entity.world.load().send_entity_status(
                        entity,
                        EntityStatus::TamingSucceeded,
                        Some(ActorEventType::TamingSucceeded),
                    );
                } else {
                    let entity = self.get_entity();
                    entity.world.load().send_entity_status(
                        entity,
                        EntityStatus::TamingFailed,
                        Some(ActorEventType::TamingFailed),
                    );
                }
                return true;
            }

            if self.is_tame.load(Ordering::Relaxed)
                && self.owner.load() == Some(player.gameprofile.id)
                && !item_stack
                    .get_item()
                    .has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD)
            {
                self.is_sitting.fetch_xor(true, Ordering::Relaxed);
                self.sync_tame_flags();
                return true;
            }

            if !item_stack
                .get_item()
                .has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD)
            {
                return self.mob_entity.mob_interact(player, item_stack).await;
            }

            self.eat_cookie(player, item_stack).await;
            true
        })
    }
}

#[cfg(test)]
mod tests {
    use super::COOKIE_POISON_DURATION;
    use pumpkin_data::item::Item;
    use pumpkin_data::tag::{self, Taggable};

    /// The interaction is gated on the vanilla `parrot_poisonous_food` tag rather than
    /// on a hardcoded cookie id, so check the tag actually resolves the way the
    /// interaction assumes.
    #[test]
    fn cookie_is_poisonous_parrot_food() {
        assert!(Item::COOKIE.has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD));
    }

    /// Seeds tame a parrot in vanilla and must not reach the poison branch.
    #[test]
    fn parrot_food_is_not_poisonous() {
        assert!(!Item::WHEAT_SEEDS.has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD));
        assert!(!Item::COOKED_CHICKEN.has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD));
    }

    #[test]
    fn poison_lasts_45_seconds() {
        assert_eq!(COOKIE_POISON_DURATION, 900);
    }
}
