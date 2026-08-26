use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::item::Item;
use pumpkin_util::GameMode;

pub struct MaceItem;

impl ItemMetadata for MaceItem {
    fn ids() -> Box<[u16]> {
        [Item::MACE.id].into()
    }
}

impl ItemBehaviour for MaceItem {
    fn can_mine(&self, player: &Player) -> bool {
        player.gamemode.load() != GameMode::Creative
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl MaceItem {
    /// Calculates the smash attack bonus damage in Vanilla 1.21.4:
    /// - Fall distance 0..=3 blocks: +3.0 damage per block
    /// - Fall distance 3..=8 blocks: +1.5 damage per block
    /// - Fall distance >8 blocks: +1.0 damage per block
    /// - Density enchantment: adds +0.5 damage per enchantment level per block fallen.
    #[must_use]
    pub fn calculate_smash_damage_bonus(fall_distance: f32, density_level: u32) -> f32 {
        if fall_distance <= 0.0 {
            return 0.0;
        }

        let base_bonus = if fall_distance <= 3.0 {
            3.0 * fall_distance
        } else if fall_distance <= 8.0 {
            9.0 + 1.5 * (fall_distance - 3.0)
        } else {
            16.5 + 1.0 * (fall_distance - 8.0)
        };

        let density_bonus = (density_level as f32) * 0.5 * fall_distance;
        base_bonus + density_bonus
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_mace_smash_damage_tiers_and_density() {
        // Zero fall distance
        assert_eq!(MaceItem::calculate_smash_damage_bonus(0.0, 0), 0.0);

        // 3 blocks fallen (3.0 * 3 = 9.0)
        assert_eq!(MaceItem::calculate_smash_damage_bonus(3.0, 0), 9.0);

        // 5 blocks fallen (9.0 + 1.5 * 2 = 12.0)
        assert_eq!(MaceItem::calculate_smash_damage_bonus(5.0, 0), 12.0);

        // 8 blocks fallen (9.0 + 1.5 * 5 = 16.5)
        assert_eq!(MaceItem::calculate_smash_damage_bonus(8.0, 0), 16.5);

        // 10 blocks fallen (16.5 + 1.0 * 2 = 18.5)
        assert_eq!(MaceItem::calculate_smash_damage_bonus(10.0, 0), 18.5);

        // With Density V (5 * 0.5 * 10 = +25.0) -> 18.5 + 25.0 = 43.5
        assert_eq!(MaceItem::calculate_smash_damage_bonus(10.0, 5), 43.5);
    }
}
