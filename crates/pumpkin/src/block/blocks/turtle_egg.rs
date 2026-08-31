use pumpkin_data::block_properties::{BlockProperties, TurtleEggLikeProperties};
use pumpkin_data::entity::{EntityPose, EntityType};
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{Block, BlockDirection, BlockStateId, tag};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use rand::RngExt;

use crate::block::{
    BlockBehaviour, BlockFuture, BlockIsReplacing, BrokenArgs, CanPlaceAtArgs, CanUpdateAtArgs,
    GetStateForNeighborUpdateArgs, OnEntityStepArgs, OnLandedUponArgs, OnPlaceArgs,
    OnScheduledTickArgs, RandomTickArgs,
};
use crate::entity::EntityBase;
use crate::entity::passive::turtle::TurtleEntity;
use crate::entity::r#type::from_type;
use uuid::Uuid;

type TurtleEggProperties = TurtleEggLikeProperties;

#[pumpkin_block("minecraft:turtle_egg")]
pub struct TurtleEggBlock;

impl BlockBehaviour for TurtleEggBlock {
    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if args.player.get_entity().pose.load() != EntityPose::Crouching
                && let BlockIsReplacing::Itself(state_id) = args.replacing
            {
                let mut properties = TurtleEggProperties::from_state_id(state_id, args.block);
                if properties.eggs < 4 {
                    properties.eggs += 1;
                }
                return properties.to_state_id(args.block);
            }

            let properties = TurtleEggProperties::default(args.block);
            if args
                .world
                .get_block(&args.position.down())
                .has_tag(&tag::Block::MINECRAFT_SAND)
            {
                args.world.sync_world_event(
                    WorldEvent::ParticlesTurtleEggPlacement,
                    *args.position,
                    15,
                );
            }
            properties.to_state_id(args.block)
        })
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_at(args.block_accessor, args.position)
    }

    fn can_update_at(&self, args: CanUpdateAtArgs<'_>) -> bool {
        let b = BlockAccessor::get_block(args.world, args.position);
        args.player.get_entity().pose.load() != EntityPose::Crouching
            && TurtleEggProperties::from_state_id(args.state_id, args.block).eggs < 4
            && args.block.id == b.id
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if !can_place_at(args.world, args.position) {
                args.world
                    .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
            }
            args.state_id
        })
    }

    fn on_scheduled_tick<'a>(&'a self, args: OnScheduledTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if !can_place_at(args.world.as_ref(), args.position) {
                args.world
                    .break_block(args.position, None, BlockFlags::empty())
                    .await;
            }
        })
    }

    fn random_tick<'a>(&'a self, args: RandomTickArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            // Turtle eggs can only hatch when placed on sand
            if !args
                .world
                .get_block(&args.position.down())
                .has_tag(&tag::Block::MINECRAFT_SAND)
            {
                return;
            }

            let time_of_day = args.world.level_time.lock().await.time_of_day;
            if !is_hatching_window(time_of_day) && !rand::rng().random_ratio(1, 500) {
                return;
            }

            let state_id = args.world.get_block_state_id(args.position);
            let mut props = TurtleEggProperties::from_state_id(state_id, args.block);

            if props.hatch < 2 {
                props.hatch += 1;
                args.world
                    .set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;

                args.world.play_sound_fine(
                    Sound::EntityTurtleEggCrack,
                    SoundCategory::Blocks,
                    &args.position.to_f64(),
                    0.7,
                    rand::random_range(0.9..1.1),
                );
            } else {
                args.world
                    .set_block_state(
                        args.position,
                        pumpkin_data::Block::AIR.default_state.id,
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;

                args.world.play_sound_fine(
                    Sound::EntityTurtleEggHatch,
                    SoundCategory::Blocks,
                    &args.position.to_f64(),
                    0.7,
                    rand::random_range(0.9..1.1),
                );

                for index in 0..props.eggs {
                    args.world.sync_world_event(
                        WorldEvent::ParticlesDestroyBlock,
                        *args.position,
                        state_id.as_u16().into(),
                    );
                    let turtle = from_type(
                        &EntityType::TURTLE,
                        Vector3::new(
                            f64::from(args.position.0.x) + 0.3 + f64::from(index) * 0.2,
                            f64::from(args.position.0.y),
                            f64::from(args.position.0.z) + 0.3,
                        ),
                        args.world,
                        Uuid::new_v4(),
                    );
                    turtle.get_entity().set_age(-24000);
                    turtle
                        .cast_any()
                        .downcast_ref::<TurtleEntity>()
                        .expect("turtle entity factory returned a different entity type")
                        .set_home_pos(*args.position);
                    args.world.spawn_entity(turtle).await;
                }
            }
        })
    }

    fn on_landed_upon<'a>(&'a self, args: OnLandedUponArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if let Some(living) = args.entity.get_living_entity() {
                living
                    .handle_fall_damage(args.entity, args.fall_distance, 1.0)
                    .await;
            }

            if args.entity.get_entity().entity_type != &EntityType::ZOMBIE {
                try_break_egg(args.world, args.position, args.entity, 3).await;
            }
        })
    }

    fn on_entity_step<'a>(&'a self, args: OnEntityStepArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if !args.entity.get_entity().is_sneaking() {
                try_break_egg(args.world, args.position, args.entity, 100).await;
            }
        })
    }

    fn broken<'a>(&'a self, args: BrokenArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let mut props = TurtleEggProperties::from_state_id(args.state.id, args.block);
            args.world.play_sound_fine(
                Sound::EntityTurtleEggBreak,
                SoundCategory::Blocks,
                &args.position.to_f64(),
                0.7,
                rand::random_range(0.9..1.1),
            );
            if props.eggs > 1 {
                props.eggs -= 1;
                args.world
                    .set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_LISTENERS,
                    )
                    .await;
                args.world.sync_world_event(
                    WorldEvent::ParticlesDestroyBlock,
                    *args.position,
                    args.state.id.as_u16().into(),
                );
            }
        })
    }
}

fn is_hatching_window(time_of_day: i64) -> bool {
    let day_fraction = (time_of_day.rem_euclid(24_000) as f64 / 24_000.0 - 0.25).rem_euclid(1.0);
    let smoothed_fraction =
        (day_fraction * 2.0 + (0.5 - (day_fraction * std::f64::consts::PI).cos() / 2.0)) / 3.0;
    is_hatching_angle(smoothed_fraction)
}

fn is_hatching_angle(angle: f64) -> bool {
    angle > 0.65 && angle < 0.69
}

fn can_trample(entity: &dyn EntityBase) -> bool {
    let entity_type = entity.get_entity().entity_type;
    entity.get_living_entity().is_some()
        && entity_type != &EntityType::TURTLE
        && entity_type != &EntityType::BAT
}

async fn try_break_egg(
    world: &std::sync::Arc<crate::world::World>,
    position: &BlockPos,
    entity: &dyn EntityBase,
    chance: u32,
) {
    if !can_trample(entity)
        || (entity.get_player().is_none() && !world.level_info.load().game_rules.mob_griefing)
        || !rand::rng().random_ratio(1, chance)
    {
        return;
    }

    let state_id = world.get_block_state_id(position);
    if state_id.to_block_id() != Block::TURTLE_EGG.id {
        return;
    }
    let mut props = TurtleEggProperties::from_state_id(state_id, &Block::TURTLE_EGG);
    world.play_sound_fine(
        Sound::EntityTurtleEggBreak,
        SoundCategory::Blocks,
        &position.to_f64(),
        0.7,
        rand::random_range(0.9..1.1),
    );
    let new_state = if props.eggs <= 1 {
        Block::AIR.default_state.id
    } else {
        props.eggs -= 1;
        props.to_state_id(&Block::TURTLE_EGG)
    };
    world
        .set_block_state(position, new_state, BlockFlags::NOTIFY_ALL)
        .await;
    world.sync_world_event(
        WorldEvent::ParticlesDestroyBlock,
        *position,
        state_id.as_u16().into(),
    );
}

fn can_place_at(block_accessor: &dyn BlockAccessor, position: &BlockPos) -> bool {
    let (support_block, state) = block_accessor.get_block_and_state(&position.down());
    support_block.has_tag(&tag::Block::MINECRAFT_SAND) || state.is_center_solid(BlockDirection::Up)
}

#[cfg(test)]
mod tests {
    use super::{is_hatching_angle, is_hatching_window};

    #[test]
    fn turtle_eggs_always_progress_during_hatching_window() {
        assert!(is_hatching_window(21_600));
    }

    #[test]
    fn daytime_is_outside_hatching_window() {
        assert!(!is_hatching_window(6_000));
    }

    #[test]
    fn hatch_window_repeats_each_day() {
        assert!(is_hatching_window(21_600 + 24_000));
        assert!(is_hatching_window(21_600 - 24_000));
    }

    #[test]
    fn hatch_window_boundaries_are_strict() {
        assert!(!is_hatching_angle(0.65));
        assert!(is_hatching_angle(0.650_001));
        assert!(is_hatching_angle(0.689_999));
        assert!(!is_hatching_angle(0.69));
    }
}
