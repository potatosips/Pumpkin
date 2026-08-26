use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::entity::player::Player;
use crate::entity::projectile::experience_bottle::ExperienceBottleEntity;
use crate::entity::{Entity, EntityBase};
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::sound::{Sound, SoundCategory};

pub struct ExperienceBottleItem;

impl ItemMetadata for ExperienceBottleItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::EXPERIENCE_BOTTLE.id])
    }
}

impl ItemBehaviour for ExperienceBottleItem {
    fn normal_use<'a>(
        &'a self,
        _item: &'a Item,
        player: &'a Player,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let world = player.world();
            let pos = player.position();
            world.play_sound_fine(
                Sound::EntityExperienceBottleThrow,
                SoundCategory::Players,
                &pos,
                0.5,
                0.4 / (rand::random::<f32>() * 0.4 + 0.8),
            );

            let inventory = player.inventory();
            let main_hand = inventory.held_item().await;
            let (held, hand) = if main_hand.item.id == Item::EXPERIENCE_BOTTLE.id {
                (main_hand, pumpkin_util::Hand::Right)
            } else {
                (inventory.off_hand_item().await, pumpkin_util::Hand::Left)
            };
            let entity = Entity::new(world.clone(), pos, &EntityType::EXPERIENCE_BOTTLE);
            let bottle =
                ExperienceBottleEntity::new_shot(entity, player.get_entity(), held.clone());
            let (yaw, pitch) = player.rotation();
            bottle
                .thrown
                .set_velocity_from(player.get_entity(), pitch, yaw, -20.0, 0.7, 1.0);
            world.spawn_entity(Arc::new(bottle)).await;

            let mut held = held;
            held.decrement_unless_creative(player.gamemode.load(), 1);
            inventory.set_stack_in_hand(hand, held).await;
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
