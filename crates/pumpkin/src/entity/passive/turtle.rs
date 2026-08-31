use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::{
    entity::EntityType, item::Item, item_stack::ItemStack, sound::Sound, tracked_data,
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::java::client::play::Metadata;
use pumpkin_util::math::position::BlockPos;

use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, NBTStorage, NbtFuture,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        breed::BreedGoal, lay_turtle_egg::LayTurtleEggGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, swim::SwimGoal, tempt::TemptGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

const TEMPT_ITEMS: &[&Item] = &[&Item::SEAGRASS];

pub struct TurtleEntity {
    pub mob_entity: MobEntity,
    ageable_data: AgeableData,
    home_pos: AtomicCell<BlockPos>,
    has_egg: AtomicBool,
    laying_egg: AtomicBool,
}

impl TurtleEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let home_pos = BlockPos::floored_v(mob_entity.living_entity.entity.pos.load());
        let turtle = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            home_pos: AtomicCell::new(home_pos),
            has_egg: AtomicBool::new(false),
            laying_egg: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(turtle);
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

            goal_selector.add_goal(1, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, Box::new(LayTurtleEggGoal::new(mob_arc.clone())));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.1, TEMPT_ITEMS)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    pub fn set_home_pos(&self, position: BlockPos) {
        self.home_pos.store(position);
    }

    pub fn home_pos(&self) -> BlockPos {
        self.home_pos.load()
    }

    pub fn has_egg(&self) -> bool {
        self.has_egg.load(Ordering::Relaxed)
    }

    pub fn set_has_egg(&self, has_egg: bool) {
        self.has_egg.store(has_egg, Ordering::Relaxed);
        self.get_entity().send_meta_data(
            &[Metadata::new(tracked_data::turtle::HAS_EGG, has_egg)],
            None,
        );
    }

    pub fn set_laying_egg(&self, laying: bool) {
        if self.laying_egg.swap(laying, Ordering::Relaxed) != laying {
            self.get_entity().send_meta_data(
                &[Metadata::new(tracked_data::turtle::LAYING_EGG, laying)],
                None,
            );
        }
    }
}

impl AgeableMob for TurtleEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for TurtleEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.id == Item::SEAGRASS.id
    }
}

impl NBTStorage for TurtleEntity {
    fn write_nbt<'a>(&'a self, nbt: &'a mut NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.write_nbt(nbt).await;
            self.write_ageable_nbt(nbt);
            self.write_animal_nbt(nbt);
            let home = self.home_pos.load();
            nbt.put_int("HomePosX", home.0.x);
            nbt.put_int("HomePosY", home.0.y);
            nbt.put_int("HomePosZ", home.0.z);
            nbt.put_bool("HasEgg", self.has_egg.load(Ordering::Relaxed));
        })
    }

    fn read_nbt_non_mut<'a>(&'a self, nbt: &'a NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            self.read_ageable_nbt(nbt);
            self.read_animal_nbt(nbt);
            if let (Some(x), Some(y), Some(z)) = (
                nbt.get_int("HomePosX"),
                nbt.get_int("HomePosY"),
                nbt.get_int("HomePosZ"),
            ) {
                self.set_home_pos(BlockPos::new(x, y, z));
            }
            if let Some(has_egg) = nbt.get_bool("HasEgg") {
                self.set_has_egg(has_egg);
            }
        })
    }
}

impl Mob for TurtleEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_home(&self) -> Option<BlockPos> {
        Some(self.home_pos.load())
    }

    fn can_breed_now(&self) -> bool {
        !self.has_egg.load(Ordering::Relaxed)
    }

    fn spawn_breeding_offspring<'a>(
        &'a self,
        _mate: &'a dyn EntityBase,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            self.set_has_egg(true);
        })
    }

    fn mob_tick<'a>(&'a self, _caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            let was_baby = self.is_baby();
            self.ageable_ai_step();
            if was_baby && !self.is_baby() {
                let entity = self.get_entity();
                let world = entity.world.load_full();
                if world.level_info.load().game_rules.mob_drops {
                    world
                        .drop_stack(
                            &entity.block_pos.load(),
                            ItemStack::new(1, &Item::TURTLE_SCUTE),
                        )
                        .await;
                }
            }
        })
    }

    fn mob_interact<'a>(
        &'a self,
        player: &'a Arc<Player>,
        item_stack: &'a mut ItemStack,
    ) -> EntityBaseFuture<'a, bool> {
        self.animal_interact(player, item_stack, Sound::EntityTurtleAmbientLand)
    }
}
