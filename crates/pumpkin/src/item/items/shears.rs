use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::ageable::AgeableMob;
use crate::entity::item::ItemEntity;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use crate::world::World;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockDirection, BlockStateId};
use pumpkin_util::GameMode;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;

use crate::entity::NBTStorage;
use crate::entity::passive::cow::CowEntity;

pub struct ShearsItem;

impl ItemMetadata for ShearsItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::SHEARS.id])
    }
}

const fn get_wool_item_for_color(color: u8) -> &'static Item {
    match color {
        0 => &Item::WHITE_WOOL,
        1 => &Item::ORANGE_WOOL,
        2 => &Item::MAGENTA_WOOL,
        3 => &Item::LIGHT_BLUE_WOOL,
        4 => &Item::YELLOW_WOOL,
        5 => &Item::LIME_WOOL,
        6 => &Item::PINK_WOOL,
        7 => &Item::GRAY_WOOL,
        8 => &Item::LIGHT_GRAY_WOOL,
        9 => &Item::CYAN_WOOL,
        10 => &Item::PURPLE_WOOL,
        11 => &Item::BLUE_WOOL,
        12 => &Item::BROWN_WOOL,
        13 => &Item::GREEN_WOOL,
        14 => &Item::RED_WOOL,
        _ => &Item::BLACK_WOOL,
    }
}

impl ItemBehaviour for ShearsItem {
    fn can_mine(&self, player: &Player) -> bool {
        player.gamemode.load() != GameMode::Creative
    }

    fn use_on_block<'a>(
        &'a self,
        item: &'a mut ItemStack,
        player: &'a Player,
        location: BlockPos,
        _face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        block: &'a Block,
        _server: &'a Server,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let world = player.world();
            let state_id = world.get_block_state_id(&location);

            if handle_growing_plant(player, &location, block, state_id).await {
                let _ = item.damage_item(1);
                return;
            }

            if handle_beehive(player, &location, block, state_id).await {
                let _ = item.damage_item(1);
                return;
            }

            if handle_pumpkin(player, &location, block).await {
                let _ = item.damage_item(1);
            }
        })
    }

    fn use_on_entity<'a>(
        &'a self,
        item: &'a mut ItemStack,
        player: &'a Player,
        entity: Arc<dyn EntityBase>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let Some(sheep) = entity.get_mob().and_then(|m| m.get_sheep())
                && sheep.mob_entity.living_entity.entity.is_alive()
                && !sheep.is_sheared()
                && !sheep.is_baby()
            {
                sheep.set_sheared(true);
                let world = player.world();
                let pos = sheep.mob_entity.living_entity.entity.pos.load();
                world.play_sound(Sound::EntitySheepShear, SoundCategory::Players, &pos);

                let wool_count = (rand::random::<u8>() % 3 + 1) as u8;
                let wool_item = get_wool_item_for_color(sheep.get_color());
                for _ in 0..wool_count {
                    let drop_pos = Vector3::new(pos.x, pos.y + 1.0, pos.z);
                    let item_entity = Arc::new(ItemEntity::new(
                        Entity::new(world.clone(), drop_pos, &EntityType::ITEM),
                        ItemStack::new(1, wool_item),
                    ));
                    world.spawn_entity(item_entity).await;
                }
                let _ = item.damage_item(1);
                return;
            }

            if let Some(snow_golem) = entity.get_mob().and_then(|m| m.get_snow_golem())
                && snow_golem.mob_entity.living_entity.entity.is_alive()
                && snow_golem.has_pumpkin()
            {
                let world = player.world();
                let pos = snow_golem.mob_entity.living_entity.entity.pos.load();
                world.play_sound(Sound::EntitySnowGolemShear, SoundCategory::Players, &pos);
                snow_golem.set_pumpkin(false);
                spawn_sheared_item(&world, pos, &Item::CARVED_PUMPKIN).await;
                let _ = item.damage_item(1);
                return;
            }

            if let Some(bogged) = entity.get_mob().and_then(|m| m.get_bogged())
                && bogged.get_entity().is_alive()
                && !bogged.is_sheared()
            {
                let world = player.world();
                let pos = bogged.get_entity().pos.load();
                world.play_sound(Sound::EntityBoggedShear, SoundCategory::Players, &pos);
                bogged.set_sheared(true);
                for _ in 0..2 {
                    let mushroom = if rand::random::<bool>() {
                        &Item::RED_MUSHROOM
                    } else {
                        &Item::BROWN_MUSHROOM
                    };
                    spawn_sheared_item(&world, pos, mushroom).await;
                }
                let _ = item.damage_item(1);
                return;
            }

            if let Some(mooshroom) = entity.get_mob().and_then(|m| m.get_mooshroom())
                && mooshroom.mob_entity.living_entity.entity.is_alive()
                && !mooshroom.is_baby()
            {
                let world = player.world();
                let source = &mooshroom.mob_entity.living_entity.entity;
                let pos = source.pos.load();
                let cow = CowEntity::new(Entity::new(world.clone(), pos, &EntityType::COW));

                let mut nbt = pumpkin_nbt::compound::NbtCompound::new();
                mooshroom.write_nbt(&mut nbt).await;
                cow.read_nbt_non_mut(&nbt).await;

                let mut transform_event =
                    crate::plugin::api::events::entity::entity_transform::EntityTransformEvent::new(
                        source.entity_id,
                        cow.mob_entity.living_entity.entity.entity_id,
                        "SHEARED".to_string(),
                    );
                if let Some(server) = world.server.upgrade() {
                    server
                        .plugin_manager
                        .fire(&server, &mut transform_event)
                        .await;
                }
                if transform_event.cancelled {
                    return;
                }

                world.play_sound(Sound::EntityMooshroomShear, SoundCategory::Players, &pos);
                world.spawn_particle(
                    Vector3::new(pos.x, pos.y + 0.5, pos.z),
                    Vector3::new(0.0, 0.0, 0.0),
                    0.0,
                    1,
                    Particle::Explosion,
                );
                world.remove_entity(entity.as_ref()).await;
                world.spawn_entity(cow).await;
                let mushroom = if mooshroom.is_brown() {
                    &Item::BROWN_MUSHROOM
                } else {
                    &Item::RED_MUSHROOM
                };
                for _ in 0..5 {
                    spawn_sheared_item(&world, pos, mushroom).await;
                }
                let _ = item.damage_item(1);
            }
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

async fn spawn_sheared_item(world: &Arc<World>, pos: Vector3<f64>, item: &'static Item) {
    let drop_pos = Vector3::new(pos.x, pos.y + 1.0, pos.z);
    let item_entity = Arc::new(ItemEntity::new(
        Entity::new(world.clone(), drop_pos, &EntityType::ITEM),
        ItemStack::new(1, item),
    ));
    world.spawn_entity(item_entity).await;
}

async fn handle_growing_plant(
    player: &Player,
    location: &BlockPos,
    block: &Block,
    state_id: BlockStateId,
) -> bool {
    let is_growing_plant = block.id == Block::KELP.id
        || block.id == Block::CAVE_VINES.id
        || block.id == Block::CAVE_VINES_PLANT.id
        || block.id == Block::TWISTING_VINES.id
        || block.id == Block::WEEPING_VINES.id;

    if !is_growing_plant {
        return false;
    }

    let world = player.world();
    let action = block.properties(state_id).and_then(|props| {
        let prop_map = props.to_props();
        prop_map
            .iter()
            .find(|(k, _)| *k == "age")
            .and_then(|(_, age_str)| age_str.parse::<u8>().ok())
            .filter(|&age| age < 25)
            .map(|_| {
                let new_props: Vec<(&str, &str)> = prop_map
                    .iter()
                    .map(|(k, v)| if *k == "age" { (*k, "25") } else { (*k, *v) })
                    .collect();
                block.from_properties(&new_props).to_state_id(block)
            })
    });

    if let Some(new_state_id) = action {
        world
            .set_block_state(location, new_state_id, BlockFlags::NOTIFY_ALL)
            .await;
        world.play_sound(
            Sound::BlockGrowingPlantCrop,
            SoundCategory::Blocks,
            &location.to_f64(),
        );
        return true;
    }

    false
}

async fn handle_beehive(
    player: &Player,
    location: &BlockPos,
    block: &Block,
    state_id: BlockStateId,
) -> bool {
    if block.id != Block::BEEHIVE.id && block.id != Block::BEE_NEST.id {
        return false;
    }

    let world = player.world();
    let action = block.properties(state_id).and_then(|props| {
        let prop_map = props.to_props();
        prop_map
            .iter()
            .find(|(k, v)| *k == "honey_level" && *v == "5")
            .map(|_| {
                let new_props: Vec<(&str, &str)> = prop_map
                    .iter()
                    .map(|(k, v)| {
                        if *k == "honey_level" {
                            (*k, "0")
                        } else {
                            (*k, *v)
                        }
                    })
                    .collect();
                block.from_properties(&new_props).to_state_id(block)
            })
    });

    if let Some(new_state_id) = action {
        world
            .set_block_state(location, new_state_id, BlockFlags::NOTIFY_ALL)
            .await;
        world.play_sound(
            Sound::BlockBeehiveShear,
            SoundCategory::Blocks,
            &location.to_f64(),
        );

        let drop_pos = Vector3::new(
            f64::from(location.0.x) + 0.5,
            f64::from(location.0.y) + 0.5,
            f64::from(location.0.z) + 0.5,
        );
        let item_entity = Arc::new(ItemEntity::new(
            Entity::new(world.clone(), drop_pos, &EntityType::ITEM),
            ItemStack::new(3, &Item::HONEYCOMB),
        ));
        world.spawn_entity(item_entity).await;
        return true;
    }

    false
}

async fn handle_pumpkin(player: &Player, location: &BlockPos, block: &Block) -> bool {
    if block.id == Block::PUMPKIN.id {
        let world = player.world();
        let carved_state = Block::CARVED_PUMPKIN.default_state.id;
        world
            .set_block_state(location, carved_state, BlockFlags::NOTIFY_ALL)
            .await;
        world.play_sound(
            Sound::BlockPumpkinCarve,
            SoundCategory::Blocks,
            &location.to_f64(),
        );

        let drop_pos = Vector3::new(
            f64::from(location.0.x) + 0.5,
            f64::from(location.0.y) + 0.5,
            f64::from(location.0.z) + 0.5,
        );
        let item_entity = Arc::new(ItemEntity::new(
            Entity::new(world.clone(), drop_pos, &EntityType::ITEM),
            ItemStack::new(4, &Item::PUMPKIN_SEEDS),
        ));
        world.spawn_entity(item_entity).await;
        return true;
    }
    false
}

pub async fn try_shear_block(world: &Arc<World>, location: &BlockPos, block: &Block) -> bool {
    let state_id = world.get_block_state_id(location);
    if block.id == Block::BEEHIVE.id || block.id == Block::BEE_NEST.id {
        let action = block.properties(state_id).and_then(|props| {
            let prop_map = props.to_props();
            prop_map
                .iter()
                .find(|(k, v)| *k == "honey_level" && *v == "5")
                .map(|_| {
                    let new_props: Vec<(&str, &str)> = prop_map
                        .iter()
                        .map(|(k, v)| {
                            if *k == "honey_level" {
                                (*k, "0")
                            } else {
                                (*k, *v)
                            }
                        })
                        .collect();
                    block.from_properties(&new_props).to_state_id(block)
                })
        });

        if let Some(new_state_id) = action {
            world
                .set_block_state(location, new_state_id, BlockFlags::NOTIFY_ALL)
                .await;
            world.play_sound(
                Sound::BlockBeehiveShear,
                SoundCategory::Blocks,
                &location.to_f64(),
            );

            let drop_pos = Vector3::new(
                f64::from(location.0.x) + 0.5,
                f64::from(location.0.y) + 0.5,
                f64::from(location.0.z) + 0.5,
            );
            let item_entity = Arc::new(ItemEntity::new(
                Entity::new(world.clone(), drop_pos, &EntityType::ITEM),
                ItemStack::new(3, &Item::HONEYCOMB),
            ));
            world.spawn_entity(item_entity).await;
            return true;
        }
    } else if block.id == Block::PUMPKIN.id {
        let carved_state = Block::CARVED_PUMPKIN.default_state.id;
        world
            .set_block_state(location, carved_state, BlockFlags::NOTIFY_ALL)
            .await;
        world.play_sound(
            Sound::BlockPumpkinCarve,
            SoundCategory::Blocks,
            &location.to_f64(),
        );

        let drop_pos = Vector3::new(
            f64::from(location.0.x) + 0.5,
            f64::from(location.0.y) + 0.5,
            f64::from(location.0.z) + 0.5,
        );
        let item_entity = Arc::new(ItemEntity::new(
            Entity::new(world.clone(), drop_pos, &EntityType::ITEM),
            ItemStack::new(4, &Item::PUMPKIN_SEEDS),
        ));
        world.spawn_entity(item_entity).await;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_shears_wool_color_mapping() {
        assert_eq!(get_wool_item_for_color(0).id, Item::WHITE_WOOL.id);
        assert_eq!(get_wool_item_for_color(1).id, Item::ORANGE_WOOL.id);
        assert_eq!(get_wool_item_for_color(4).id, Item::YELLOW_WOOL.id);
        assert_eq!(get_wool_item_for_color(14).id, Item::RED_WOOL.id);
        assert_eq!(get_wool_item_for_color(15).id, Item::BLACK_WOOL.id);
        assert_eq!(get_wool_item_for_color(99).id, Item::BLACK_WOOL.id);
    }
}
