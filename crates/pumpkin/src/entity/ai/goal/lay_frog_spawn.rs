use std::sync::Arc;

use pumpkin_data::{
    Block,
    sound::{Sound, SoundCategory},
};
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};
use pumpkin_world::world::BlockFlags;

use super::{Controls, Goal, GoalFuture};
use crate::entity::{ai::pathfinder::NavigatorGoal, mob::Mob, passive::frog::FrogEntity};

pub struct LayFrogSpawnGoal {
    frog: Arc<FrogEntity>,
    target: Option<BlockPos>,
}

impl LayFrogSpawnGoal {
    pub const fn new(frog: Arc<FrogEntity>) -> Self {
        Self { frog, target: None }
    }

    fn find_target(&self, mob: &dyn Mob) -> Option<BlockPos> {
        let entity = mob.get_entity();
        let world = entity.world.load();
        let center = entity.block_pos.load();
        let mut best = None;
        for dy in -2..=2 {
            for dx in -8..=8 {
                for dz in -8..=8 {
                    let water = BlockPos::new(center.0.x + dx, center.0.y + dy, center.0.z + dz);
                    let spawn = water.up();
                    if world.get_block(&water) == &Block::WATER
                        && world.get_block_state(&spawn).is_air()
                    {
                        let distance = entity.pos.load().squared_distance_to_vec(&spawn.to_f64());
                        if best.is_none_or(|(d, _)| distance < d) {
                            best = Some((distance, spawn));
                        }
                    }
                }
            }
        }
        best.map(|(_, pos)| pos)
    }
}

impl Goal for LayFrogSpawnGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            if !self.frog.is_pregnant() {
                return false;
            }
            self.target = self.find_target(mob);
            self.target.is_some()
        })
    }

    fn should_continue<'a>(&'a self, _mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move { self.frog.is_pregnant() && self.target.is_some() })
    }

    fn tick<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            let Some(target) = self.target else { return };
            let entity = mob.get_entity();
            let center = Vector3::new(
                f64::from(target.0.x) + 0.5,
                f64::from(target.0.y),
                f64::from(target.0.z) + 0.5,
            );
            if entity.pos.load().squared_distance_to_vec(&center) > 2.0 {
                let mut navigator = mob
                    .get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                navigator.set_progress(NavigatorGoal::new(entity.pos.load(), center, 1.0));
                return;
            }
            let world = entity.world.load_full();
            if world.get_block(&target.down()) == &Block::WATER
                && world.get_block_state(&target).is_air()
            {
                world
                    .set_block_state(
                        &target,
                        Block::FROGSPAWN.default_state.id,
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;
                crate::block::blocks::frogspawn::schedule_hatch(&world, target);
                world.play_sound(Sound::EntityFrogLaySpawn, SoundCategory::Neutral, &center);
                self.frog.set_pregnant(false);
            }
            self.target = None;
        })
    }

    fn stop<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.target = None;
        })
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }
    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}
