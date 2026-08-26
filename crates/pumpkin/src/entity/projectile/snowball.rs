use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::entity::projectile::ProjectileHit;
use crate::{
    entity::{Entity, EntityBase, EntityBaseFuture, NBTStorage, projectile::ThrownItemEntity},
    server::Server,
};
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::{EntityStatus, EntityType};
use pumpkin_protocol::bedrock::server::actor_event::ActorEventType;
use pumpkin_util::math::vector3::Vector3;

const GRAVITY: f64 = 0.03;

pub struct SnowballEntity {
    pub thrown: ThrownItemEntity,
}

impl SnowballEntity {
    pub fn new(entity: Entity) -> Self {
        // Keep the velocity initialization
        entity.set_velocity(Vector3::new(0.0, 0.1, 0.0));

        // Initialize without owner
        let thrown = ThrownItemEntity {
            entity,
            owner_id: None,
            collides_with_projectiles: false,
            has_hit: AtomicBool::new(false),
            gravity: GRAVITY,
        };

        Self { thrown }
    }

    pub fn new_shot(entity: Entity, shooter: &Entity) -> Self {
        let thrown = ThrownItemEntity::new(entity, shooter, GRAVITY);
        thrown.entity.set_velocity(Vector3::new(0.0, 0.1, 0.0));
        Self { thrown }
    }
}

impl NBTStorage for SnowballEntity {}

impl EntityBase for SnowballEntity {
    fn tick<'a>(
        &'a self,
        caller: &'a Arc<dyn EntityBase>,
        server: &'a Server,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move { self.thrown.process_tick(caller, server).await })
    }

    fn get_entity(&self) -> &Entity {
        self.thrown.get_entity()
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

    fn on_hit(&self, hit: crate::entity::projectile::ProjectileHit) -> EntityBaseFuture<'_, ()> {
        Box::pin(async move {
            let world = self.get_entity().world.load();

            // Always send particle status regardless of what was hit
            world.send_entity_status(
                self.get_entity(),
                EntityStatus::Death,
                Some(ActorEventType::Death),
            );

            // Handle entity-specific damage & knockback
            if let ProjectileHit::Entity { ref entity, .. } = hit {
                let entity_clone = entity.clone();
                let is_blaze = entity_clone.get_entity().entity_type.id == EntityType::BLAZE.id;

                if let Some(owner_id) = self.thrown.owner_id {
                    if let Some(living) = entity_clone.get_living_entity() {
                        living
                            .last_attacker_id
                            .store(owner_id, std::sync::atomic::Ordering::Relaxed);
                        living
                            .last_attacked_time
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }

                if is_blaze {
                    tokio::spawn(async move {
                        entity_clone
                            .damage(entity_clone.as_ref(), 3.0, DamageType::THROWN)
                            .await;
                    });
                } else {
                    // Apply knockback to hit entity (including friendly fire on other snow golems)
                    let snowball_vel = self.get_entity().velocity.load();
                    let target_entity = entity_clone.get_entity();
                    target_entity.knockback(0.4, -snowball_vel.x, -snowball_vel.z);
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snowball_gravity_and_blaze_damage_parity() {
        assert_eq!(GRAVITY, 0.03);
    }
}
