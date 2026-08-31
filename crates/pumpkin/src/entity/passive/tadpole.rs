use std::sync::{Arc, Weak, atomic::Ordering};

use pumpkin_data::{
    entity::EntityType,
    item::Item,
    item_stack::ItemStack,
    particle::Particle,
    sound::{Sound, SoundCategory},
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, NBTStorage, NbtFuture,
    ai::goal::{
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::frog::{FrogEntity, FrogVariant},
    player::Player,
    r#type::from_type,
};
use pumpkin_util::math::vector3::Vector3;

const TICKS_TO_GROW: i32 = 24000;

pub struct TadpoleEntity {
    pub mob_entity: MobEntity,
}

impl TadpoleEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let tadpole = Self { mob_entity };
        let mob_arc = Arc::new(tadpole);
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
            goal_selector.add_goal(
                2,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(3, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }
}

impl NBTStorage for TadpoleEntity {
    fn write_nbt<'a>(&'a self, nbt: &'a mut NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.write_nbt(nbt).await;
            nbt.put_int(
                "Age",
                self.get_entity()
                    .age
                    .load(Ordering::Relaxed)
                    .clamp(0, TICKS_TO_GROW),
            );
        })
    }

    fn read_nbt_non_mut<'a>(&'a self, nbt: &'a NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            self.get_entity().age.store(
                nbt.get_int("Age").unwrap_or(0).clamp(0, TICKS_TO_GROW),
                Ordering::Relaxed,
            );
        })
    }
}

impl Mob for TadpoleEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick<'a>(&'a self, _caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            let entity = self.get_entity();
            let age = entity.age.fetch_add(1, Ordering::Relaxed) + 1;
            if age < TICKS_TO_GROW {
                return;
            }

            let world = entity.world.load_full();
            let position = entity.pos.load();
            let frog = from_type(&EntityType::FROG, position, &world, Uuid::new_v4());
            let temperature = world
                .get_biome(&entity.block_pos.load())
                .weather
                .base_temperature();
            frog.cast_any()
                .downcast_ref::<FrogEntity>()
                .expect("frog entity factory returned a different entity type")
                .set_variant(FrogVariant::for_temperature(temperature));
            world.spawn_entity(frog).await;
            world.play_sound(
                Sound::EntityTadpoleGrowUp,
                SoundCategory::Neutral,
                &position,
            );
            entity.remove().await;
        })
    }

    fn mob_interact<'a>(
        &'a self,
        player: &'a Arc<Player>,
        item_stack: &'a mut ItemStack,
    ) -> EntityBaseFuture<'a, bool> {
        Box::pin(async move {
            if item_stack.item != &Item::SLIME_BALL {
                if item_stack.item == &Item::WATER_BUCKET {
                    let age = self
                        .get_entity()
                        .age
                        .load(Ordering::Relaxed)
                        .clamp(0, TICKS_TO_GROW);
                    let mut bucket = ItemStack::new(1, &Item::TADPOLE_BUCKET);
                    bucket.set_custom_data("pumpkin", "TadpoleAge", NbtTag::Int(age));
                    super::cow::exchange_empty_container_stack(player, item_stack, bucket).await;
                    let entity = self.get_entity();
                    entity.world.load().play_sound(
                        Sound::ItemBucketFillTadpole,
                        SoundCategory::Neutral,
                        &entity.pos.load(),
                    );
                    entity.remove().await;
                    return true;
                }
                return self.mob_entity.mob_interact(player, item_stack).await;
            }
            let entity = self.get_entity();
            let age = entity.age.load(Ordering::Relaxed).clamp(0, TICKS_TO_GROW);
            if age >= TICKS_TO_GROW {
                return false;
            }
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let speedup = ((TICKS_TO_GROW - age) / 10).max(1);
            entity
                .age
                .store((age + speedup).min(TICKS_TO_GROW), Ordering::Relaxed);
            let pos = entity.pos.load();
            entity.world.load().spawn_particle(
                pos + Vector3::new(0.0, f64::from(entity.height()), 0.0),
                Vector3::new(0.5, 0.5, 0.5),
                1.0,
                7,
                Particle::HappyVillager,
            );
            true
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_growth_duration() {
        assert_eq!(TICKS_TO_GROW, 24000);
    }
}
