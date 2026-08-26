use std::sync::Arc;

use pumpkin_data::{effect::StatusEffect, potion::Effect};
use pumpkin_util::difficulty::Difficulty;

use crate::entity::mob::zombie::ZombieEntityBase;
use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, NBTStorage, NbtFuture,
    mob::{Mob, MobEntity},
};

pub struct HuskEntity {
    entity: Arc<ZombieEntityBase>,
}

impl HuskEntity {
    const fn hunger_duration(difficulty: Difficulty) -> Option<i32> {
        match difficulty {
            Difficulty::Peaceful => None,
            Difficulty::Easy => Some(7 * 20),
            Difficulty::Normal => Some(14 * 20),
            Difficulty::Hard => Some(21 * 20),
        }
    }

    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = ZombieEntityBase::new(entity);
        let zombie = Self { entity };
        Arc::new(zombie)
    }
}

impl NBTStorage for HuskEntity {
    fn write_nbt<'a>(
        &'a self,
        nbt: &'a mut pumpkin_nbt::compound::NbtCompound,
    ) -> NbtFuture<'a, ()> {
        self.entity.write_nbt(nbt)
    }

    fn read_nbt_non_mut<'a>(
        &'a self,
        nbt: &'a pumpkin_nbt::compound::NbtCompound,
    ) -> NbtFuture<'a, ()> {
        self.entity.read_nbt_non_mut(nbt)
    }
}

impl Mob for HuskEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    fn mob_tick<'a>(&'a self, caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            self.entity
                .tick_underwater_conversion(caller, &pumpkin_data::entity::EntityType::ZOMBIE)
                .await;
        })
    }

    fn on_successful_attack<'a>(&'a self, target: &'a dyn EntityBase) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            let difficulty = self.get_entity().world.load().level_info.load().difficulty;
            let Some(duration) = Self::hunger_duration(difficulty) else {
                return;
            };
            if let Some(living) = target.get_living_entity() {
                living
                    .add_effect(Effect {
                        effect_type: &StatusEffect::HUNGER,
                        duration,
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
    use super::HuskEntity;
    use pumpkin_util::difficulty::Difficulty;

    #[test]
    fn hunger_duration_scales_with_vanilla_difficulty() {
        assert_eq!(HuskEntity::hunger_duration(Difficulty::Peaceful), None);
        assert_eq!(HuskEntity::hunger_duration(Difficulty::Easy), Some(140));
        assert_eq!(HuskEntity::hunger_duration(Difficulty::Normal), Some(280));
        assert_eq!(HuskEntity::hunger_duration(Difficulty::Hard), Some(420));
    }
}
