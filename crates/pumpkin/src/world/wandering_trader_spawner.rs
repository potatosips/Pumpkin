use std::sync::Arc;

use pumpkin_data::{
    dimension::Dimension,
    entity::EntityType,
    game_rules::{GameRule, GameRuleValue},
    tag::{Taggable, WorldgenBiome::MINECRAFT_WITHOUT_WANDERING_TRADER_SPAWNS},
};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::chunk::ChunkHeightmapType;
use rand::{RngExt, rng};
use uuid::Uuid;

use crate::{
    entity::{EntityBase, r#type::from_type},
    server::Server,
    world::{World, natural_spawner::is_spawn_position_ok},
};

const CHECK_INTERVAL: i32 = 1_200;
const SPAWN_DELAY: i32 = 24_000;
const MIN_CHANCE: i32 = 25;
const MAX_CHANCE: i32 = 75;
const CHANCE_STEP: i32 = 25;

pub struct WanderingTraderSpawner {
    tick_delay: i32,
    data: WanderingTraderState,
    trader_id: Option<Uuid>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WanderingTraderState {
    pub spawn_delay: i32,
    pub spawn_chance: i32,
}

impl WanderingTraderSpawner {
    pub fn new(mut data: WanderingTraderState, trader_id: Option<Uuid>) -> Self {
        // Vanilla initializes both values when an older/new world has neither value yet.
        if data.spawn_delay == 0 && data.spawn_chance == 0 {
            data.spawn_delay = SPAWN_DELAY;
            data.spawn_chance = MIN_CHANCE;
        }
        Self {
            tick_delay: CHECK_INTERVAL,
            data,
            trader_id,
        }
    }

    fn sync_spawn_delay(server: &Server, delay: i32) {
        server.level_info.rcu(|current| {
            let mut updated = (**current).clone();
            updated.wandering_trader_spawn_delay = delay;
            updated
        });
    }

    fn sync_spawn_chance(server: &Server, chance: i32) {
        server.level_info.rcu(|current| {
            let mut updated = (**current).clone();
            updated.wandering_trader_spawn_chance = chance;
            updated
        });
    }

    fn sync_trader_id(server: &Server, trader_id: Option<Uuid>) {
        server.level_info.rcu(|current| {
            let mut updated = (**current).clone();
            updated.wandering_trader_id = trader_id;
            updated
        });
    }

    /// Returns the delay value Java persists and whether an attempt is due.
    fn advance_timer(&mut self) -> (Option<i32>, bool) {
        self.tick_delay -= 1;
        if self.tick_delay > 0 {
            return (None, false);
        }
        self.tick_delay = CHECK_INTERVAL;
        self.data.spawn_delay -= CHECK_INTERVAL;
        let persisted_delay = self.data.spawn_delay;
        if self.data.spawn_delay > 0 {
            return (Some(persisted_delay), false);
        }
        self.data.spawn_delay = SPAWN_DELAY;
        (Some(persisted_delay), true)
    }

    fn advance_spawn_chance(&mut self) -> i32 {
        let chance = self.data.spawn_chance;
        self.data.spawn_chance = (chance + CHANCE_STEP).clamp(MIN_CHANCE, MAX_CHANCE);
        chance
    }

    fn record_success(&mut self) {
        self.data.spawn_chance = MIN_CHANCE;
    }

    pub async fn tick(&mut self, server: &Arc<Server>) {
        let world = server.get_world_from_dimension(&Dimension::OVERWORLD);
        // Vanilla checks doMobSpawning before advancing either countdown.
        if !matches!(
            world.get_game_rule(&GameRule::SpawnMobs),
            GameRuleValue::Bool(true)
        ) {
            return;
        }
        let (persisted_delay, attempt_due) = self.advance_timer();
        if let Some(delay) = persisted_delay {
            Self::sync_spawn_delay(server, delay);
        }
        if !attempt_due {
            return;
        }
        if !matches!(
            world.get_game_rule(&GameRule::SpawnWanderingTraders),
            GameRuleValue::Bool(true)
        ) {
            return;
        }
        let chance = self.advance_spawn_chance();
        Self::sync_spawn_chance(server, self.data.spawn_chance);
        if rng().random_range(0..100) > chance {
            return;
        }
        let players = world.players.load();
        // Vanilla's spawn method returns true when there is no random player,
        // so the outer state machine treats this as success and resets chance.
        if players.is_empty() {
            self.record_success();
            return;
        }
        if rng().random_range(0..10) != 0 {
            return;
        }
        let player = &players[rng().random_range(0..players.len())];
        let player_pos = player.get_entity().block_pos.load();
        drop(players);
        let center = world
            .villager_poi
            .lock()
            .await
            .nearest_meeting_site(player_pos, 48)
            .unwrap_or(player_pos);

        let Some(trader_pos) = find_spawn_position(&world, center, 48).await else {
            return;
        };
        if !has_enough_trader_space(&world, trader_pos)
            || world
                .get_biome(&trader_pos)
                .has_tag(&MINECRAFT_WITHOUT_WANDERING_TRADER_SPAWNS)
        {
            return;
        }
        let trader = from_type(
            &EntityType::WANDERING_TRADER,
            trader_pos.to_centered_f64(),
            &world,
            Uuid::new_v4(),
        );
        if let Some(mob) = trader.get_mob() {
            mob.get_mob_entity().position_target.store(center);
            mob.get_mob_entity()
                .position_target_range
                .store(16, std::sync::atomic::Ordering::Relaxed);
            if let Some(wandering_trader) = mob.get_wandering_trader() {
                wandering_trader.set_despawn_delay(48_000);
                wandering_trader.set_wander_target(Some(center));
            }
        }
        world.spawn_entity(trader.clone()).await;
        self.trader_id = Some(trader.get_entity().entity_uuid);
        Self::sync_trader_id(server, self.trader_id);

        for _ in 0..2 {
            let Some(llama_pos) = find_spawn_position(&world, trader_pos, 4).await else {
                continue;
            };
            let llama = from_type(
                &EntityType::TRADER_LLAMA,
                llama_pos.to_centered_f64(),
                &world,
                Uuid::new_v4(),
            );
            world.spawn_entity(llama.clone()).await;
            llama.get_entity().leash_to(trader.clone()).await;
        }
        self.record_success();
    }
}

async fn find_spawn_position(
    world: &Arc<World>,
    center: BlockPos,
    radius: i32,
) -> Option<BlockPos> {
    for _ in 0..10 {
        let x = spawn_axis(center.0.x, radius, rng().random_range(0..radius * 2));
        let z = spawn_axis(center.0.z, radius, rng().random_range(0..radius * 2));
        let y = world
            .get_heightmap_height_async(ChunkHeightmapType::MotionBlockingNoLeaves, x, z)
            .await;
        let pos = BlockPos::new(x, y, z);
        // Vanilla constructs the placement for WANDERING_TRADER once and reuses
        // it for both the trader search and both nearby llama searches.
        if is_spawn_position_ok(world, &pos, &EntityType::WANDERING_TRADER) {
            return Some(pos);
        }
    }
    None
}

const fn spawn_axis(center: i32, radius: i32, roll: i32) -> i32 {
    center + roll - radius
}

fn trader_space_positions(center: BlockPos) -> impl Iterator<Item = BlockPos> {
    BlockPos::iterate(
        center,
        BlockPos::new(center.0.x + 1, center.0.y + 2, center.0.z + 1),
    )
}

fn has_enough_trader_space(world: &World, center: BlockPos) -> bool {
    trader_space_positions(center).all(|pos| {
        world
            .get_block_state(&pos)
            .get_block_collision_shapes_at(&pos)
            .next()
            .is_none()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_timer_and_chance_progression() {
        let mut spawner = WanderingTraderSpawner::new(
            WanderingTraderState {
                spawn_delay: 24_000,
                spawn_chance: 25,
            },
            None,
        );
        for _ in 0..23_999 {
            assert!(!spawner.advance_timer().1);
        }
        assert_eq!(spawner.advance_timer(), (Some(0), true));
        assert_eq!(spawner.data.spawn_delay, 24_000);
        assert_eq!(spawner.advance_spawn_chance(), 25);
        assert_eq!(spawner.data.spawn_chance, 50);
        for _ in 0..23_999 {
            assert_eq!(spawner.advance_timer().1, false);
        }
        assert_eq!(spawner.advance_timer(), (Some(0), true));
        assert_eq!(spawner.advance_spawn_chance(), 50);
        assert_eq!(spawner.data.spawn_chance, 75);
    }

    #[test]
    fn spawn_delay_requests_persistence_every_sixty_seconds() {
        let mut spawner = WanderingTraderSpawner::new(
            WanderingTraderState {
                spawn_delay: 24_000,
                spawn_chance: 25,
            },
            None,
        );
        for _ in 0..1_199 {
            assert_eq!(spawner.advance_timer(), (None, false));
        }
        assert_eq!(spawner.advance_timer(), (Some(22_800), false));
        assert_eq!(spawner.data.spawn_delay, 22_800);
    }

    #[test]
    fn successful_or_playerless_attempt_resets_chance_in_memory() {
        let mut spawner = WanderingTraderSpawner::new(
            WanderingTraderState {
                spawn_delay: 24_000,
                spawn_chance: 75,
            },
            None,
        );
        spawner.record_success();
        assert_eq!(spawner.data.spawn_chance, 25);
    }

    #[test]
    fn absent_vanilla_values_are_initialized_together() {
        let spawner = WanderingTraderSpawner::new(
            WanderingTraderState {
                spawn_delay: 0,
                spawn_chance: 0,
            },
            None,
        );
        assert_eq!(spawner.data.spawn_delay, 24_000);
        assert_eq!(spawner.data.spawn_chance, 25);
    }

    #[test]
    fn trader_clearance_checks_the_exact_two_by_three_by_two_volume() {
        let positions = trader_space_positions(BlockPos::new(10, 64, -4)).collect::<Vec<_>>();
        assert_eq!(positions.len(), 12);
        assert!(positions.contains(&BlockPos::new(10, 64, -4)));
        assert!(positions.contains(&BlockPos::new(11, 66, -3)));
        assert!(!positions.contains(&BlockPos::new(9, 64, -5)));
    }

    #[test]
    fn spawn_axis_matches_vanilla_half_open_random_range() {
        assert_eq!(spawn_axis(100, 48, 0), 52);
        assert_eq!(spawn_axis(100, 48, 95), 147);
    }
}
