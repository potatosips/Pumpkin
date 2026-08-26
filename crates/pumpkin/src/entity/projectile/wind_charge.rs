use pumpkin_util::math::vector3::Vector3;
use std::{
    f64,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};

use crate::{
    entity::{
        Entity, EntityBase, EntityBaseFuture, NBTStorage,
        living::LivingEntity,
        projectile::{ProjectileHit, ThrownItemEntity},
        projectile_deflection::ProjectileDeflectionType,
    },
    server::Server,
};

const EXPLOSION_POWER: f32 = 1.2;
const PLAYER_KNOCKBACK_MULTIPLIER: f64 = 1.22;
const BREEZE_EXPLOSION_POWER: f32 = 3.0;
const DEFAULT_DEFLECT_COOLDOWN: u8 = 5;
pub const WIND_CHARGE_GRAVITY: f64 = 0.0;

/// A kind to differentiate both types of wind charges from each other.
enum WindChargeKind {
    /// Represents a wind charge spawned by a player or dispenser.
    /// This wind charge also has a deflect cooldown counter.
    Normal { deflect_cooldown: AtomicU8 },
    /// Represents a wind charge spawned by a breeze.
    Breeze,
}

pub struct WindChargeEntity {
    kind: WindChargeKind,
    thrown_item_entity: ThrownItemEntity,
}

impl WindChargeEntity {
    #[must_use]
    pub const fn new_normal(thrown_item_entity: ThrownItemEntity) -> Self {
        Self {
            kind: WindChargeKind::Normal {
                deflect_cooldown: AtomicU8::new(DEFAULT_DEFLECT_COOLDOWN),
            },
            thrown_item_entity,
        }
    }

    #[must_use]
    pub const fn new_breeze(thrown_item_entity: ThrownItemEntity) -> Self {
        Self {
            kind: WindChargeKind::Breeze,
            thrown_item_entity,
        }
    }

    pub const fn deflect_cooldown(&self) -> Option<&AtomicU8> {
        if let WindChargeKind::Normal {
            deflect_cooldown, ..
        } = &self.kind
        {
            Some(deflect_cooldown)
        } else {
            None
        }
    }

    pub async fn create_explosion(&self, position: Vector3<f64>) {
        let (power, knockback_multiplier) = match self.kind {
            WindChargeKind::Normal { .. } => (EXPLOSION_POWER, PLAYER_KNOCKBACK_MULTIPLIER),
            WindChargeKind::Breeze => (BREEZE_EXPLOSION_POWER, 1.0),
        };
        self.get_entity()
            .world
            .load()
            .explode_wind_charge(position, power, knockback_multiplier)
            .await;
    }

    pub fn deflect(
        &mut self,
        deflection: &ProjectileDeflectionType,
        deflector: Option<&dyn EntityBase>,
        _from_attack: bool,
    ) -> bool {
        if let Some(cooldown) = self.deflect_cooldown()
            && cooldown.load(Ordering::Relaxed) > 0
        {
            return false;
        }

        deflection.deflect(self, deflector);

        /* TODO: Does this need to be implemented?
        if self.get_entity().world().is_client() {
            self.set_owner();
            self.on_Deflected(from_attack);
        }
         */
        true
    }
}

impl NBTStorage for WindChargeEntity {}

impl EntityBase for WindChargeEntity {
    fn tick<'a>(
        &'a self,
        caller: &'a Arc<dyn EntityBase>,
        server: &'a Server,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            self.thrown_item_entity.process_tick(caller, server).await;

            if let Some(cooldown) = self.deflect_cooldown() {
                let cooldown_ticks = cooldown.load(Ordering::Relaxed);
                if cooldown_ticks > 0 {
                    cooldown.store(cooldown_ticks - 1, Ordering::Relaxed);
                }
            }
        })
    }

    fn get_entity(&self) -> &Entity {
        &self.thrown_item_entity.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
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
            let hit_pos = match &hit {
                ProjectileHit::Block {
                    hit_pos, normal, ..
                } => hit_pos.add(&normal.multiply(0.25, 0.25, 0.25)),
                ProjectileHit::Entity {
                    entity, hit_pos, ..
                } => {
                    let world = self.get_entity().world.load();
                    let owner = self
                        .thrown_item_entity
                        .owner_id
                        .and_then(|owner_id| world.get_entity_by_id(owner_id));
                    let source = owner.as_deref().unwrap_or(self);
                    entity
                        .damage_with_context(
                            entity.as_ref(),
                            1.0,
                            pumpkin_data::damage::DamageType::WIND_CHARGE,
                            Some(*hit_pos),
                            Some(self),
                            Some(source),
                        )
                        .await;
                    *hit_pos
                }
            };
            self.create_explosion(hit_pos).await;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wind_charge_constants_parity() {
        assert_eq!(EXPLOSION_POWER, 1.2);
        assert_eq!(PLAYER_KNOCKBACK_MULTIPLIER, 1.22);
        assert_eq!(BREEZE_EXPLOSION_POWER, 3.0);
        assert_eq!(DEFAULT_DEFLECT_COOLDOWN, 5);
        assert_eq!(WIND_CHARGE_GRAVITY, 0.0);
    }
}
