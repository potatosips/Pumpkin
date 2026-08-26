use std::sync::{Arc, Weak};

use pumpkin_data::entity::EntityType;

use crate::entity::{
    Entity, NBTStorage,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, swim::SwimGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
};

pub struct BreezeEntity {
    pub mob_entity: MobEntity,
}

impl BreezeEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let breeze = Self { mob_entity };
        let mob_arc = Arc::new(breeze);
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
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak.clone(), &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(
                1,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
        };

        mob_arc
    }

    pub const MAX_TARGET_RANGE: f64 = 24.0;
    pub const MIN_CHARGE_SHOOT_RANGE: f64 = 4.0;
    pub const JUMP_COOLDOWN_TICKS: u32 = 40;
    pub const SHOOT_COOLDOWN_TICKS: u32 = 30;
    pub const PROJECTILE_SPEED: f64 = 0.7;

    #[must_use]
    pub fn is_valid_shooting_distance(distance: f64) -> bool {
        (Self::MIN_CHARGE_SHOOT_RANGE..=Self::MAX_TARGET_RANGE).contains(&distance)
    }

    #[must_use]
    pub fn should_deflect_projectile(entity_type: &EntityType) -> bool {
        *entity_type == EntityType::ARROW
            || *entity_type == EntityType::SPECTRAL_ARROW
            || *entity_type == EntityType::TRIDENT
            || *entity_type == EntityType::SNOWBALL
            || *entity_type == EntityType::EGG
            || *entity_type == EntityType::FIREWORK_ROCKET
            || *entity_type == EntityType::WIND_CHARGE
    }
}

impl NBTStorage for BreezeEntity {}

impl Mob for BreezeEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_breeze_shooting_distance_validation() {
        assert!(BreezeEntity::is_valid_shooting_distance(4.0));
        assert!(BreezeEntity::is_valid_shooting_distance(15.0));
        assert!(BreezeEntity::is_valid_shooting_distance(24.0));

        assert!(!BreezeEntity::is_valid_shooting_distance(3.9));
        assert!(!BreezeEntity::is_valid_shooting_distance(24.1));
    }

    #[test]
    fn vanilla_breeze_projectile_deflection() {
        assert!(BreezeEntity::should_deflect_projectile(&EntityType::ARROW));
        assert!(BreezeEntity::should_deflect_projectile(
            &EntityType::SPECTRAL_ARROW
        ));
        assert!(BreezeEntity::should_deflect_projectile(
            &EntityType::TRIDENT
        ));
        assert!(BreezeEntity::should_deflect_projectile(
            &EntityType::SNOWBALL
        ));
        assert!(BreezeEntity::should_deflect_projectile(
            &EntityType::WIND_CHARGE
        ));
        assert!(!BreezeEntity::should_deflect_projectile(
            &EntityType::PLAYER
        ));
    }
}
