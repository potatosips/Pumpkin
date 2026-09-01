use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};

pub struct SaddleItem;

impl ItemMetadata for SaddleItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::SADDLE.id])
    }
}

impl ItemBehaviour for SaddleItem {
    fn use_on_entity<'a>(
        &'a self,
        item: &'a mut ItemStack,
        player: &'a Player,
        entity: Arc<dyn EntityBase>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let Some(mob) = entity.get_mob()
                && mob.can_be_saddled()
                && !mob.is_saddled()
            {
                mob.set_saddle_stack(item.copy_with_count(1)).await;
                let ent = entity.get_entity();
                let sound = saddle_sound_for_entity_type(ent.entity_type);
                player
                    .world()
                    .play_sound(sound, SoundCategory::Neutral, &ent.pos.load());
                item.decrement_unless_creative(player.gamemode.load(), 1);
            }
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn saddle_sound_for_entity_type(entity_type: &EntityType) -> Sound {
    match entity_type {
        ty if ty == &EntityType::STRIDER => Sound::EntityStriderSaddle,
        ty if ty == &EntityType::HORSE
            || ty == &EntityType::DONKEY
            || ty == &EntityType::MULE
            || ty == &EntityType::SKELETON_HORSE
            || ty == &EntityType::ZOMBIE_HORSE =>
        {
            Sound::EntityHorseSaddle
        }
        _ => Sound::EntityPigSaddle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undead_horses_use_the_abstract_horse_saddle_sound() {
        assert_eq!(
            saddle_sound_for_entity_type(&EntityType::SKELETON_HORSE),
            Sound::EntityHorseSaddle
        );
        assert_eq!(
            saddle_sound_for_entity_type(&EntityType::ZOMBIE_HORSE),
            Sound::EntityHorseSaddle
        );
        assert_eq!(
            saddle_sound_for_entity_type(&EntityType::STRIDER),
            Sound::EntityStriderSaddle
        );
    }
}
