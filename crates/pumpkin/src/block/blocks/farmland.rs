use std::sync::Arc;

use crate::block::BlockBehaviour;
use crate::block::BlockFuture;
use crate::block::CanPlaceAtArgs;
use crate::block::GetStateForNeighborUpdateArgs;
use crate::block::OnLandedUponArgs;
use crate::block::OnPlaceArgs;
use crate::block::OnScheduledTickArgs;
use crate::block::RandomTickArgs;
use crate::world::World;
use pumpkin_data::Block;
use pumpkin_data::BlockDirection;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::BlockProperties;
use pumpkin_data::block_properties::FarmlandLikeProperties;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::tag;
use pumpkin_data::tag::Taggable;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockAccessor;
use pumpkin_world::world::BlockFlags;

type FarmlandProperties = FarmlandLikeProperties;

#[pumpkin_block("minecraft:farmland")]
pub struct FarmlandBlock;

impl BlockBehaviour for FarmlandBlock {
    fn on_landed_upon<'a>(&'a self, args: OnLandedUponArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if let Some(living) = args.entity.get_living_entity() {
                let entity = args.entity.get_entity();
                let can_modify = args.entity.get_player().is_some()
                    || args.world.level_info.load().game_rules.mob_griefing;
                if should_trample(
                    args.fall_distance,
                    rand::random::<f32>(),
                    entity.width(),
                    entity.height(),
                    can_modify,
                ) && args.world.get_block(args.position) == &Block::FARMLAND
                {
                    let mut event =
                        crate::plugin::api::events::block::block_fade::BlockFadeEvent::new(
                            *args.position,
                            &Block::DIRT,
                        );
                    if let Some(server) = args.world.server.upgrade() {
                        server.plugin_manager.fire(&server, &mut event).await;
                    }
                    if !event.cancelled {
                        args.world
                            .set_block_state(
                                args.position,
                                Block::DIRT.default_state.id,
                                BlockFlags::NOTIFY_ALL,
                            )
                            .await;
                    }
                }

                living
                    .handle_fall_damage(args.entity, args.fall_distance, 1.0)
                    .await;
            }
        })
    }

    fn on_scheduled_tick<'a>(&'a self, args: OnScheduledTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            // TODO: push up entities
            args.world
                .set_block_state(
                    args.position,
                    Block::DIRT.default_state.id,
                    BlockFlags::NOTIFY_ALL,
                )
                .await;
        })
    }

    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if !can_place_at(args.world, args.position) {
                return Block::DIRT.default_state.id;
            }
            args.block.default_state.id
        })
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if args.direction == BlockDirection::Up && !can_place_at(args.world, args.position) {
                args.world
                    .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
            }
            args.state_id
        })
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_at(args.block_accessor, args.position)
    }

    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if is_water_nearby(args.world, args.position)
                || args.world.is_raining_at(&args.position.up()).await
            {
                let mut props = FarmlandProperties::default(args.block);
                props.moisture = 7;
                let mut event = crate::plugin::block::moisture_change::MoistureChangeEvent {
                    block_pos: *args.position,
                    world: args.world.clone(),
                    new_moisture: 7,
                    cancelled: false,
                };
                if let Some(server) = args.world.server.upgrade() {
                    server.plugin_manager.fire(&server, &mut event).await;
                }
                if !event.cancelled {
                    props.moisture = (event.new_moisture.clamp(0, 7)) as u8;
                    args.world
                        .set_block_state(
                            args.position,
                            props.to_state_id(args.block),
                            BlockFlags::NOTIFY_NEIGHBORS,
                        )
                        .await;
                }
            } else {
                let state_id = args.world.get_block_state_id(args.position);
                let mut props = FarmlandProperties::from_state_id(state_id, args.block);
                if props.moisture == 0 {
                    if !args
                        .world
                        .get_block(&args.position.up())
                        .has_tag(&tag::Block::MINECRAFT_MAINTAINS_FARMLAND)
                    {
                        let mut event =
                            crate::plugin::api::events::block::block_fade::BlockFadeEvent::new(
                                *args.position,
                                &Block::DIRT,
                            );
                        if let Some(server) = args.world.server.upgrade() {
                            server.plugin_manager.fire(&server, &mut event).await;
                        }
                        if event.cancelled {
                            return;
                        }

                        //TODO push entities up
                        args.world
                            .set_block_state(
                                args.position,
                                Block::DIRT.default_state.id,
                                BlockFlags::NOTIFY_NEIGHBORS,
                            )
                            .await;
                    }
                } else {
                    let mut event = crate::plugin::block::moisture_change::MoistureChangeEvent {
                        block_pos: *args.position,
                        world: args.world.clone(),
                        new_moisture: (props.moisture as i32) - 1,
                        cancelled: false,
                    };
                    if let Some(server) = args.world.server.upgrade() {
                        server.plugin_manager.fire(&server, &mut event).await;
                    }
                    if !event.cancelled {
                        props.moisture = (event.new_moisture.clamp(0, 7)) as u8;
                        args.world
                            .set_block_state(
                                args.position,
                                props.to_state_id(args.block),
                                BlockFlags::NOTIFY_NEIGHBORS,
                            )
                            .await;
                    }
                }
            }
        })
    }
}

fn can_place_at(world: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
    let above = block_pos.up();
    let (block, state) = world.get_block_and_state(&above);
    !state.is_solid() || block.has_tag(&tag::Block::MINECRAFT_FENCE_GATES)
}

fn is_water_nearby(world: &Arc<World>, block_pos: &BlockPos) -> bool {
    for dx in -4..=4 {
        for dy in 0..=1 {
            for dz in -4..=4 {
                let check_pos = block_pos.offset(Vector3 {
                    x: dx,
                    y: dy,
                    z: dz,
                });
                if is_hydrating_fluid(world.get_fluid(&check_pos)) {
                    return true;
                }
            }
        }
    }
    false
}

fn is_hydrating_fluid(fluid: &Fluid) -> bool {
    fluid.has_tag(&tag::Fluid::MINECRAFT_WATER)
}

fn should_trample(
    fall_distance: f32,
    random_value: f32,
    width: f32,
    height: f32,
    can_modify: bool,
) -> bool {
    can_modify && random_value < fall_distance - 0.5 && width * width * height > 0.512
}

#[cfg(test)]
mod tests {
    use super::{is_hydrating_fluid, should_trample};
    use pumpkin_data::fluid::Fluid;

    #[test]
    fn water_fluid_tag_controls_farmland_hydration() {
        assert!(is_hydrating_fluid(&Fluid::WATER));
        assert!(is_hydrating_fluid(&Fluid::FLOWING_WATER));
        assert!(!is_hydrating_fluid(&Fluid::LAVA));
        assert!(!is_hydrating_fluid(&Fluid::EMPTY));
    }

    #[test]
    fn farmland_trampling_matches_fall_chance_size_and_griefing_gates() {
        assert!(should_trample(1.0, 0.49, 0.9, 1.8, true));
        assert!(!should_trample(1.0, 0.5, 0.9, 1.8, true));
        assert!(!should_trample(1.0, 0.0, 0.5, 1.0, true));
        assert!(!should_trample(2.0, 0.0, 0.9, 1.8, false));
        assert!(should_trample(2.0, 0.999, 0.9, 1.8, true));
    }
}
