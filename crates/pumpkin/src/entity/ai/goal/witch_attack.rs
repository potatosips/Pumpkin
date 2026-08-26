use std::sync::Arc;

use pumpkin_data::data_component::DataComponent;
use pumpkin_data::data_component_impl::{DataComponentImpl, PotionContentsImpl};
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use rand::RngExt;

use super::{Controls, Goal, GoalFuture};
use crate::entity::ai::pathfinder::NavigatorGoal;
use crate::entity::mob::Mob;
use crate::entity::projectile::splash_potion::SplashPotionEntity;
use crate::entity::{Entity, EntityBase};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WitchPotionChoice {
    Slowness,
    Poison,
    Weakness,
    Harming,
}

pub struct WitchAttackGoal {
    cooldown: i32,
}

impl WitchAttackGoal {
    const RANGE_SQUARED: f64 = 100.0;

    #[must_use]
    pub const fn new() -> Self {
        Self { cooldown: 0 }
    }

    const fn choose_potion(
        distance_squared: f64,
        target_health: f32,
        has_slowness: bool,
        has_poison: bool,
        has_weakness: bool,
        weakness_roll: bool,
    ) -> WitchPotionChoice {
        if distance_squared >= 64.0 && !has_slowness {
            WitchPotionChoice::Slowness
        } else if target_health >= 8.0 && !has_poison {
            WitchPotionChoice::Poison
        } else if distance_squared <= 9.0 && !has_weakness && weakness_roll {
            WitchPotionChoice::Weakness
        } else {
            WitchPotionChoice::Harming
        }
    }

    const fn potion_id(choice: WitchPotionChoice) -> i32 {
        match choice {
            WitchPotionChoice::Slowness => pumpkin_data::potion::Potion::SLOWNESS.id as i32,
            WitchPotionChoice::Poison => pumpkin_data::potion::Potion::POISON.id as i32,
            WitchPotionChoice::Weakness => pumpkin_data::potion::Potion::WEAKNESS.id as i32,
            WitchPotionChoice::Harming => pumpkin_data::potion::Potion::HARMING.id as i32,
        }
    }

    fn potion_stack(choice: WitchPotionChoice) -> ItemStack {
        let mut stack = ItemStack::new(1, &Item::SPLASH_POTION);
        stack.patch.push((
            DataComponent::PotionContents,
            Some(
                PotionContentsImpl {
                    potion_id: Some(Self::potion_id(choice)),
                    custom_color: None,
                    custom_effects: Vec::new(),
                    custom_name: None,
                }
                .to_dyn(),
            ),
        ));
        stack
    }

    async fn shoot(mob: &dyn Mob, target: &Arc<dyn EntityBase>, distance_squared: f64) {
        let target_living = target.get_living_entity();
        let (health, has_slowness, has_poison, has_weakness) = if let Some(living) = target_living {
            (
                living.health.load(),
                living.has_effect(&StatusEffect::SLOWNESS).await,
                living.has_effect(&StatusEffect::POISON).await,
                living.has_effect(&StatusEffect::WEAKNESS).await,
            )
        } else {
            (0.0, false, false, false)
        };
        let choice = Self::choose_potion(
            distance_squared,
            health,
            has_slowness,
            has_poison,
            has_weakness,
            mob.get_random().random_range(0.0..1.0) < 0.25,
        );

        let shooter = mob.get_entity();
        let world = shooter.world.load();
        let potion_entity = Entity::new(
            world.clone(),
            shooter.pos.load(),
            &EntityType::SPLASH_POTION,
        );
        let potion = SplashPotionEntity::new_shot(potion_entity, shooter);
        potion.set_item_stack(Self::potion_stack(choice)).await;

        let potion_pos = potion.thrown.entity.pos.load();
        let target_pos = target.get_entity().pos.load();
        let dx = target_pos.x - potion_pos.x;
        let dz = target_pos.z - potion_pos.z;
        let horizontal = dx.hypot(dz);
        let dy = target.get_entity().get_eye_y() - 1.1 - potion_pos.y;
        potion
            .thrown
            .set_velocity(dx, horizontal.mul_add(0.2, dy), dz, 0.75, 8.0);
        world.spawn_entity(Arc::new(potion)).await;
    }
}

impl Default for WitchAttackGoal {
    fn default() -> Self {
        Self::new()
    }
}

impl Goal for WitchAttackGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            mob.get_mob_entity()
                .target
                .lock()
                .await
                .as_ref()
                .is_some_and(|target| target.get_entity().is_alive())
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            mob.get_mob_entity()
                .target
                .lock()
                .await
                .as_ref()
                .is_some_and(|target| target.get_entity().is_alive())
        })
    }

    fn tick<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.cooldown = (self.cooldown - 1).max(0);
            if mob
                .get_mob_entity()
                .living_entity
                .active_hand
                .lock()
                .await
                .is_some()
            {
                return;
            }
            let Some(target) = mob.get_mob_entity().target.lock().await.clone() else {
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
                Self::shoot(mob, &target, distance_squared).await;
                self.cooldown = 60;
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

#[cfg(test)]
mod tests {
    use super::{WitchAttackGoal, WitchPotionChoice};

    #[test]
    fn potion_selection_matches_vanilla_priority() {
        assert_eq!(
            WitchAttackGoal::choose_potion(64.0, 20.0, false, false, false, false),
            WitchPotionChoice::Slowness
        );
        assert_eq!(
            WitchAttackGoal::choose_potion(16.0, 20.0, false, false, false, false),
            WitchPotionChoice::Poison
        );
        assert_eq!(
            WitchAttackGoal::choose_potion(4.0, 4.0, false, true, false, true),
            WitchPotionChoice::Weakness
        );
        assert_eq!(
            WitchAttackGoal::choose_potion(4.0, 4.0, false, true, true, true),
            WitchPotionChoice::Harming
        );
    }
}
