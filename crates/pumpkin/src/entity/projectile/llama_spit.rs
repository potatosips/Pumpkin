use std::sync::Arc;

use pumpkin_data::damage::DamageType;

use crate::{
    entity::{
        Entity, EntityBase, EntityBaseFuture, NBTStorage,
        projectile::{ProjectileHit, ThrownItemEntity},
    },
    server::Server,
};

const GRAVITY: f64 = 0.06;

pub struct LlamaSpitEntity {
    pub projectile: ThrownItemEntity,
}

impl LlamaSpitEntity {
    pub fn new(entity: Entity) -> Self {
        Self {
            projectile: ThrownItemEntity {
                entity,
                owner_id: None,
                collides_with_projectiles: false,
                has_hit: std::sync::atomic::AtomicBool::new(false),
                gravity: GRAVITY,
            },
        }
    }

    pub fn new_shot(entity: Entity, owner: &Entity) -> Self {
        Self {
            projectile: ThrownItemEntity::new(entity, owner, GRAVITY),
        }
    }
}

impl NBTStorage for LlamaSpitEntity {}

impl EntityBase for LlamaSpitEntity {
    fn tick<'a>(
        &'a self,
        caller: &'a Arc<dyn EntityBase>,
        server: &'a Server,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move { self.projectile.process_tick(caller, server).await })
    }

    fn get_entity(&self) -> &Entity {
        self.projectile.get_entity()
    }

    fn get_living_entity(&self) -> Option<&crate::entity::living::LivingEntity> {
        None
    }

    fn as_nbt_storage(&self) -> &dyn NBTStorage {
        self
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }

    fn on_hit(&self, hit: ProjectileHit) -> EntityBaseFuture<'_, ()> {
        Box::pin(async move {
            if let ProjectileHit::Entity { entity, .. } = hit
                && let Some(owner_id) = self.projectile.owner_id
                && let Some(owner) = self.get_entity().world.load().get_entity_by_id(owner_id)
            {
                entity
                    .damage_with_context(
                        entity.as_ref(),
                        1.0,
                        DamageType::SPIT,
                        None,
                        Some(owner.as_ref()),
                        Some(owner.as_ref()),
                    )
                    .await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llama_spit_uses_vanilla_gravity_and_entity_type() {
        assert_eq!(GRAVITY, 0.06);
        assert_eq!(
            pumpkin_data::entity::EntityType::LLAMA_SPIT.resource_name,
            "llama_spit"
        );
    }
}
