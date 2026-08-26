use std::sync::{Arc, Weak};

use pumpkin_data::entity::EntityType;

use crate::entity::{
    Entity, NBTStorage,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
};

pub struct WardenEntity {
    pub mob_entity: MobEntity,
}

impl WardenEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let warden = Self { mob_entity };
        let mob_arc = Arc::new(warden);
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
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.0, true)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(0.5)));
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

    pub const SONIC_BOOM_HORIZONTAL_RANGE: f64 = 15.0;
    pub const SONIC_BOOM_VERTICAL_RANGE: f64 = 20.0;
    pub const SONIC_BOOM_DAMAGE: f32 = 10.0;
    pub const SONIC_BOOM_COOLDOWN_TICKS: u32 = 100;
    pub const DARKNESS_EFFECT_RANGE: f64 = 20.0;
    pub const DARKNESS_INTERVAL_TICKS: u32 = 120;
    pub const ANGER_THRESHOLD_AGITATED: i32 = 40;
    pub const ANGER_THRESHOLD_ANGRY: i32 = 80;

    #[must_use]
    pub fn is_in_sonic_boom_range(
        warden_pos: &pumpkin_util::math::vector3::Vector3<f64>,
        target_pos: &pumpkin_util::math::vector3::Vector3<f64>,
    ) -> bool {
        let dx = (warden_pos.x - target_pos.x).abs();
        let dy = (warden_pos.y - target_pos.y).abs();
        let dz = (warden_pos.z - target_pos.z).abs();
        let horizontal_dist = (dx * dx + dz * dz).sqrt();

        horizontal_dist <= Self::SONIC_BOOM_HORIZONTAL_RANGE
            && dy <= Self::SONIC_BOOM_VERTICAL_RANGE
    }
}

impl NBTStorage for WardenEntity {}

impl Mob for WardenEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_util::math::vector3::Vector3;

    #[test]
    fn vanilla_warden_sonic_boom_range_checks() {
        let warden_pos = Vector3::new(0.0, 64.0, 0.0);

        // Within 15 blocks horizontally and 20 blocks vertically
        let close_target = Vector3::new(10.0, 70.0, 10.0); // dist = ~14.14, dy = 6.0
        assert!(WardenEntity::is_in_sonic_boom_range(
            &warden_pos,
            &close_target
        ));

        // Beyond 15 blocks horizontally
        let far_horizontal = Vector3::new(12.0, 64.0, 12.0); // dist = ~16.97
        assert!(!WardenEntity::is_in_sonic_boom_range(
            &warden_pos,
            &far_horizontal
        ));

        // Beyond 20 blocks vertically
        let far_vertical = Vector3::new(0.0, 85.0, 0.0); // dy = 21.0
        assert!(!WardenEntity::is_in_sonic_boom_range(
            &warden_pos,
            &far_vertical
        ));
    }

    #[test]
    fn vanilla_warden_anger_thresholds() {
        assert_eq!(WardenEntity::ANGER_THRESHOLD_AGITATED, 40);
        assert_eq!(WardenEntity::ANGER_THRESHOLD_ANGRY, 80);
        assert_eq!(WardenEntity::SONIC_BOOM_DAMAGE, 10.0);
    }
}
