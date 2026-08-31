use std::sync::Arc;

use pumpkin_data::{
    Block,
    block_properties::{BlockProperties, TurtleEggLikeProperties},
    sound::{Sound, SoundCategory},
    tag::{self, Taggable},
};
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};
use pumpkin_world::world::BlockFlags;
use rand::RngExt;

use super::{Controls, Goal, GoalFuture};
use crate::entity::{ai::pathfinder::NavigatorGoal, mob::Mob, passive::turtle::TurtleEntity};

pub struct LayTurtleEggGoal {
    turtle: Arc<TurtleEntity>,
    target: Option<BlockPos>,
    digging_ticks: i32,
}

impl LayTurtleEggGoal {
    pub const fn new(turtle: Arc<TurtleEntity>) -> Self {
        Self {
            turtle,
            target: None,
            digging_ticks: 0,
        }
    }

    fn find_nest(&self, mob: &dyn Mob) -> Option<BlockPos> {
        let entity = mob.get_entity();
        let world = entity.world.load();
        let center = entity.block_pos.load();
        let home = self.turtle.home_pos();
        let mut best = None;
        for dy in -1..=1 {
            for dx in -8..=8 {
                for dz in -8..=8 {
                    let sand = BlockPos::new(center.0.x + dx, center.0.y + dy - 1, center.0.z + dz);
                    if sand.0.x.abs_diff(home.0.x) > 16 || sand.0.z.abs_diff(home.0.z) > 16 {
                        continue;
                    }
                    if world.get_block(&sand).has_tag(&tag::Block::MINECRAFT_SAND)
                        && world.get_block_state(&sand.up()).is_air()
                    {
                        let distance = entity
                            .pos
                            .load()
                            .squared_distance_to_vec(&sand.up().to_f64());
                        if best.is_none_or(|(best_distance, _)| distance < best_distance) {
                            best = Some((distance, sand));
                        }
                    }
                }
            }
        }
        best.map(|(_, pos)| pos)
    }
}

impl Goal for LayTurtleEggGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            if !self.turtle.has_egg() {
                return false;
            }
            self.target = self.find_nest(mob);
            self.target.is_some()
        })
    }

    fn should_continue<'a>(&'a self, _mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move { self.turtle.has_egg() && self.target.is_some() })
    }

    fn start<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.digging_ticks = 0;
        })
    }

    fn tick<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            let Some(sand) = self.target else { return };
            let entity = mob.get_entity();
            let nest = sand.up();
            let nest_center = Vector3::new(
                f64::from(nest.0.x) + 0.5,
                f64::from(nest.0.y),
                f64::from(nest.0.z) + 0.5,
            );
            if entity.pos.load().squared_distance_to_vec(&nest_center) > 2.0 {
                self.turtle.set_laying_egg(false);
                let mut navigator = mob
                    .get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                navigator.set_progress(NavigatorGoal::new(entity.pos.load(), nest_center, 1.0));
                return;
            }

            self.turtle.set_laying_egg(true);
            self.digging_ticks += 1;
            if self.digging_ticks <= 200 {
                return;
            }

            let world = entity.world.load_full();
            if world.get_block(&sand).has_tag(&tag::Block::MINECRAFT_SAND)
                && world.get_block_state(&nest).is_air()
            {
                let mut properties = TurtleEggLikeProperties::default(&Block::TURTLE_EGG);
                properties.eggs = mob.get_random().random_range(1..=4);
                world
                    .set_block_state(
                        &nest,
                        properties.to_state_id(&Block::TURTLE_EGG),
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;
                world.play_sound(
                    Sound::EntityTurtleLayEgg,
                    SoundCategory::Blocks,
                    &entity.pos.load(),
                );
                self.turtle.set_has_egg(false);
            }
            self.turtle.set_laying_egg(false);
            self.target = None;
        })
    }

    fn stop<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.turtle.set_laying_egg(false);
            self.target = None;
            self.digging_ticks = 0;
        })
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}
