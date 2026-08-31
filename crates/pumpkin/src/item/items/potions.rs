use std::pin::Pin;
use std::sync::Arc;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::entity::projectile::{
    lingering_potion::LingeringPotionEntity, splash_potion::SplashPotionEntity,
};
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use pumpkin_data::Block;
use pumpkin_data::BlockDirection;
use pumpkin_data::data_component_impl::PotionContentsImpl;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::GameMode;
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};
use pumpkin_world::world::BlockFlags;

pub struct PotionItem;
pub struct SplashPotionItem;
pub struct LingeringPotionItem;

impl ItemMetadata for PotionItem {
    fn ids() -> Box<[u16]> {
        [Item::POTION.id].into()
    }
}

impl ItemMetadata for SplashPotionItem {
    fn ids() -> Box<[u16]> {
        [Item::SPLASH_POTION.id].into()
    }
}

impl ItemMetadata for LingeringPotionItem {
    fn ids() -> Box<[u16]> {
        [Item::LINGERING_POTION.id].into()
    }
}

const POWER: f32 = 0.5;

impl ItemBehaviour for PotionItem {
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
            if !is_water_potion(item) || !converts_to_mud(block) {
                return;
            }
            let world = player.world();
            world.play_sound_fine(
                Sound::ItemBottleEmpty,
                SoundCategory::Blocks,
                &location.to_f64(),
                1.0,
                1.0,
            );
            world
                .set_block_state(
                    &location,
                    Block::MUD.default_state.id,
                    BlockFlags::NOTIFY_ALL,
                )
                .await;
            if player.gamemode.load() == GameMode::Creative {
                return;
            }
            item.decrement(1);
            let bottle = ItemStack::new(1, &Item::GLASS_BOTTLE);
            if item.is_empty() {
                *item = bottle;
            } else {
                player.inventory.offer_or_drop_stack(bottle, player).await;
            }
        })
    }

    fn normal_use<'a>(
        &'a self,
        _item: &'a Item,
        _player: &'a Player,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        // Drinking is handled by the consumable flow in the server (active hand + consumption tick).
        Box::pin(async move {})
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn is_water_potion(stack: &ItemStack) -> bool {
    stack.item.id == Item::POTION.id
        && stack
            .get_data_component::<PotionContentsImpl>()
            .is_some_and(|contents| contents.potion_id == Some(0))
}

fn converts_to_mud(block: &Block) -> bool {
    block.id == Block::DIRT.id
        || block.id == Block::COARSE_DIRT.id
        || block.id == Block::ROOTED_DIRT.id
}

impl ItemBehaviour for SplashPotionItem {
    fn normal_use<'a>(
        &'a self,
        _item: &'a Item,
        player: &'a Player,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let position = player.position();
            let world = player.world();
            world.play_sound(
                Sound::EntityWitchThrow,
                pumpkin_data::sound::SoundCategory::Neutral,
                &position,
            );
            let entity = Entity::new(world.clone(), position, &EntityType::SPLASH_POTION);
            let splash = SplashPotionEntity::new_shot(entity, player.get_entity());

            // Copy the held item stack data into the projectile
            let main_s = player.inventory.held_item().await;
            let mut used_main = true;
            let mut stack = (!main_s.is_empty()
                && main_s.item.id == pumpkin_data::item::Item::SPLASH_POTION.id)
                .then_some(main_s);
            if stack.is_none() {
                let off_s = player.inventory.off_hand_item().await;
                if !off_s.is_empty() && off_s.item.id == pumpkin_data::item::Item::SPLASH_POTION.id
                {
                    stack = Some(off_s);
                    used_main = false;
                }
            }
            let stack = stack.unwrap_or_else(|| ItemStack::EMPTY.clone());
            splash.set_item_stack(stack).await;

            let (yaw, pitch) = player.rotation();
            splash
                .thrown
                .set_velocity_from(player.get_entity(), pitch, yaw, 0.0, POWER, 1.0);

            world.spawn_entity(Arc::new(splash)).await;

            // Decrement the used stack (clear)
            if used_main {
                let mut s = player.inventory.held_item().await;
                s.decrement_unless_creative(player.gamemode.load(), 1);
                player.inventory.set_held_item(s).await;
            } else {
                let mut s = player.inventory.off_hand_item().await;
                s.decrement_unless_creative(player.gamemode.load(), 1);
                player
                    .inventory
                    .set_stack_in_hand(pumpkin_util::Hand::Left, s)
                    .await;
            }
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ItemBehaviour for LingeringPotionItem {
    fn normal_use<'a>(
        &'a self,
        _item: &'a Item,
        player: &'a Player,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let position = player.position();
            let world = player.world();
            world.play_sound(
                Sound::EntityWitchThrow,
                pumpkin_data::sound::SoundCategory::Neutral,
                &position,
            );
            let entity = Entity::new(world.clone(), position, &EntityType::LINGERING_POTION);
            let ling = LingeringPotionEntity::new_shot(entity, player.get_entity());

            // Copy the held item stack data into the projectile
            let main_s = player.inventory.held_item().await;
            let mut used_main = true;
            let mut stack = (!main_s.is_empty()
                && main_s.item.id == pumpkin_data::item::Item::LINGERING_POTION.id)
                .then_some(main_s);
            if stack.is_none() {
                let off_s = player.inventory.off_hand_item().await;
                if !off_s.is_empty()
                    && off_s.item.id == pumpkin_data::item::Item::LINGERING_POTION.id
                {
                    stack = Some(off_s);
                    used_main = false;
                }
            }
            let stack = stack.unwrap_or_else(|| ItemStack::EMPTY.clone());
            ling.set_item_stack(stack).await;

            let (yaw, pitch) = player.rotation();
            ling.thrown
                .set_velocity_from(player.get_entity(), pitch, yaw, 0.0, POWER, 1.0);

            world.spawn_entity(Arc::new(ling)).await;

            // Decrement the used stack (clear)
            if used_main {
                let mut s = player.inventory.held_item().await;
                s.decrement_unless_creative(player.gamemode.load(), 1);
                player.inventory.set_held_item(s).await;
            } else {
                let mut s = player.inventory.off_hand_item().await;
                s.decrement_unless_creative(player.gamemode.load(), 1);
                player
                    .inventory
                    .set_stack_in_hand(pumpkin_util::Hand::Left, s)
                    .await;
            }
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_potion_mud_conversion_targets() {
        assert!(converts_to_mud(&Block::DIRT));
        assert!(converts_to_mud(&Block::COARSE_DIRT));
        assert!(converts_to_mud(&Block::ROOTED_DIRT));
        assert!(!converts_to_mud(&Block::GRASS_BLOCK));

        let mut water = ItemStack::new(1, &Item::POTION);
        water
            .get_data_component_mut::<PotionContentsImpl>()
            .expect("potions have potion contents")
            .potion_id = Some(0);
        assert!(is_water_potion(&water));
        let mut awkward = water.clone();
        awkward
            .get_data_component_mut::<PotionContentsImpl>()
            .expect("potions have potion contents")
            .potion_id = Some(1);
        assert!(!is_water_potion(&awkward));
    }
}
