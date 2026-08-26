use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::entity::experience_orb::ExperienceOrbEntity;
use crate::entity::projectile::ThrownItemEntity;
use crate::entity::{Entity, EntityBase, EntityBaseFuture, NBTStorage};
use crate::server::Server;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::world::WorldEvent;
use pumpkin_protocol::codec::item_stack_seralizer::ItemStackSerializer;
use pumpkin_protocol::java::client::play::Metadata;
use tokio::sync::RwLock;

const GRAVITY: f64 = 0.07;
const EXPERIENCE_BOTTLE_PARTICLE_COLOR: i32 = -13_083_194;

pub struct ExperienceBottleEntity {
    pub thrown: ThrownItemEntity,
    item_stack: RwLock<ItemStack>,
}

impl ExperienceBottleEntity {
    pub fn new(entity: Entity) -> Self {
        Self {
            thrown: ThrownItemEntity {
                entity,
                owner_id: None,
                collides_with_projectiles: false,
                has_hit: AtomicBool::new(false),
                gravity: GRAVITY,
            },
            item_stack: RwLock::new(ItemStack::new(1, &Item::EXPERIENCE_BOTTLE)),
        }
    }

    pub fn new_shot(entity: Entity, owner: &Entity, item_stack: ItemStack) -> Self {
        Self {
            thrown: ThrownItemEntity::new(entity, owner, GRAVITY),
            item_stack: RwLock::new(item_stack),
        }
    }
}

impl NBTStorage for ExperienceBottleEntity {}

impl EntityBase for ExperienceBottleEntity {
    fn init_data_tracker(&self) -> EntityBaseFuture<'_, ()> {
        Box::pin(async move {
            let stack = self.item_stack.read().await;
            self.get_entity().send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::experience_bottle::ITEM_STACK,
                    &ItemStackSerializer::from(stack.clone()),
                )],
                None,
            );
        })
    }

    fn tick<'a>(
        &'a self,
        caller: &'a Arc<dyn EntityBase>,
        server: &'a Server,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move { self.thrown.process_tick(caller, server).await })
    }

    fn on_hit(&self, hit: crate::entity::projectile::ProjectileHit) -> EntityBaseFuture<'_, ()> {
        Box::pin(async move {
            let world = self.get_entity().world.load();
            let hit_pos = hit.hit_pos();
            world.sync_world_event(
                WorldEvent::ParticlesSpellPotionSplash,
                hit_pos.to_block_pos(),
                EXPERIENCE_BOTTLE_PARTICLE_COLOR,
            );

            let amount = experience_amount(rand::random::<u32>(), rand::random::<u32>());
            ExperienceOrbEntity::spawn(&world, hit_pos, amount).await;
        })
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
}

const fn experience_amount(first_random: u32, second_random: u32) -> u32 {
    3 + first_random % 5 + second_random % 5
}

#[cfg(test)]
mod tests {
    use super::experience_amount;

    #[test]
    fn vanilla_experience_bottle_uses_two_independent_zero_to_four_rolls() {
        assert_eq!(experience_amount(0, 0), 3);
        assert_eq!(experience_amount(4, 4), 11);
        assert_eq!(experience_amount(1, 3), 7);
        assert_eq!(experience_amount(6, 8), 7);
    }
}
