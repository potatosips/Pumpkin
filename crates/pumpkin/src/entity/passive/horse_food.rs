use std::sync::Arc;

use pumpkin_data::{
    item::Item,
    item_stack::ItemStack,
    particle::Particle,
    sound::{Sound, SoundCategory},
};
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{ageable::AgeableMob, player::Player};

fn food_effect(item: &Item) -> Option<(f32, i32)> {
    match item.id {
        id if id == Item::SUGAR.id => Some((1.0, 30)),
        id if id == Item::WHEAT.id => Some((2.0, 20)),
        id if id == Item::APPLE.id => Some((3.0, 60)),
        id if id == Item::GOLDEN_CARROT.id => Some((4.0, 60)),
        id if id == Item::GOLDEN_APPLE.id || id == Item::ENCHANTED_GOLDEN_APPLE.id => {
            Some((10.0, 240))
        }
        id if id == Item::HAY_BLOCK.id => Some((20.0, 180)),
        _ => None,
    }
}

pub async fn feed_equine<T: AgeableMob>(
    equine: &T,
    player: &Arc<Player>,
    stack: &mut ItemStack,
    sound: Sound,
) -> bool {
    let Some((healing, seconds)) = food_effect(stack.item) else {
        return false;
    };
    let living = &equine.get_mob_entity().living_entity;
    let can_heal = living.health.load() < living.get_max_health();
    if !equine.is_baby() && !can_heal {
        return false;
    }
    stack.decrement_unless_creative(player.gamemode.load(), 1);
    if can_heal {
        living.heal(healing);
    }
    if equine.is_baby() {
        equine.age_up(seconds, true);
    }
    let entity = equine.get_entity();
    let pos = entity.pos.load();
    let world = entity.world.load();
    world.play_sound(sound, SoundCategory::Neutral, &pos);
    world.spawn_particle(
        pos + Vector3::new(0.0, f64::from(entity.height()), 0.0),
        Vector3::new(0.5, 0.5, 0.5),
        1.0,
        7,
        Particle::HappyVillager,
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_foal_growth_values() {
        assert_eq!(food_effect(&Item::WHEAT), Some((2.0, 20)));
        assert_eq!(food_effect(&Item::SUGAR), Some((1.0, 30)));
        assert_eq!(food_effect(&Item::APPLE), Some((3.0, 60)));
        assert_eq!(food_effect(&Item::GOLDEN_CARROT), Some((4.0, 60)));
        assert_eq!(food_effect(&Item::HAY_BLOCK), Some((20.0, 180)));
        assert_eq!(food_effect(&Item::GOLDEN_APPLE), Some((10.0, 240)));
    }
}
