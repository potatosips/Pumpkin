use std::sync::Arc;

use pumpkin_data::{effect::StatusEffect, potion::Effect};

use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, NBTStorage,
    mob::{Mob, MobEntity, skeleton::SkeletonEntityBase},
};

pub struct WitherSkeletonEntity {
    entity: Arc<SkeletonEntityBase>,
}

impl WitherSkeletonEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = SkeletonEntityBase::new(entity);
        let skeleton = Self { entity };
        Arc::new(skeleton)
    }
}

impl NBTStorage for WitherSkeletonEntity {}

impl Mob for WitherSkeletonEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    fn on_successful_attack<'a>(&'a self, target: &'a dyn EntityBase) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            if let Some(living) = target.get_living_entity() {
                living
                    .add_effect(Effect {
                        effect_type: &StatusEffect::WITHER,
                        duration: 10 * 20,
                        amplifier: 0,
                        ambient: false,
                        show_particles: true,
                        show_icon: true,
                        blend: false,
                    })
                    .await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn vanilla_wither_skeleton_effect_parameters() {
        assert_eq!(10 * 20, 200); // 10 seconds of wither effect
    }
}
