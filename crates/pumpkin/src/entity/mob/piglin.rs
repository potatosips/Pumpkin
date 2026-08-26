use std::sync::{Arc, Weak};

use pumpkin_data::entity::EntityType;
use pumpkin_data::tag::Taggable;

use crate::entity::{
    Entity, NBTStorage,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
};

pub struct PiglinEntity {
    pub mob_entity: MobEntity,
}

impl PiglinEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let piglin = Self { mob_entity };
        let mob_arc = Arc::new(piglin);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            // Piglins use crossbows or swords, but for now we give them melee
            goal_selector.add_goal(2, Box::new(MeleeAttackGoal::new(1.0, true)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak.clone(), &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(
                1,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(
                    &mob_arc.mob_entity,
                    &EntityType::WITHER_SKELETON,
                    true,
                ),
            );
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::WITHER, true),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn is_item_piglin_safe(item: &pumpkin_data::item::Item) -> bool {
        item.has_tag(&pumpkin_data::tag::Item::MINECRAFT_PIGLIN_SAFE_ARMOR)
            || *item == pumpkin_data::item::Item::GOLDEN_HELMET
            || *item == pumpkin_data::item::Item::GOLDEN_CHESTPLATE
            || *item == pumpkin_data::item::Item::GOLDEN_LEGGINGS
            || *item == pumpkin_data::item::Item::GOLDEN_BOOTS
    }

    #[must_use]
    pub fn is_player_wearing_safe_armor(armor_items: &[Option<&pumpkin_data::item::Item>]) -> bool {
        armor_items.iter().any(|item_opt| {
            if let Some(item) = item_opt {
                Self::is_item_piglin_safe(item)
            } else {
                false
            }
        })
    }

    #[must_use]
    pub fn is_barter_currency(item: &pumpkin_data::item::Item) -> bool {
        *item == pumpkin_data::item::Item::GOLD_INGOT
    }

    #[must_use]
    pub fn is_loved_item(item: &pumpkin_data::item::Item) -> bool {
        Self::is_item_piglin_safe(item)
            || *item == pumpkin_data::item::Item::GOLD_INGOT
            || *item == pumpkin_data::item::Item::RAW_GOLD
            || *item == pumpkin_data::item::Item::GOLD_BLOCK
            || *item == pumpkin_data::item::Item::RAW_GOLD_BLOCK
            || *item == pumpkin_data::item::Item::GOLD_NUGGET
            || *item == pumpkin_data::item::Item::GOLDEN_SWORD
            || *item == pumpkin_data::item::Item::GOLDEN_AXE
            || *item == pumpkin_data::item::Item::GOLDEN_PICKAXE
            || *item == pumpkin_data::item::Item::GOLDEN_SHOVEL
            || *item == pumpkin_data::item::Item::GOLDEN_HOE
            || *item == pumpkin_data::item::Item::GOLDEN_HORSE_ARMOR
            || *item == pumpkin_data::item::Item::BELL
            || *item == pumpkin_data::item::Item::GLISTERING_MELON_SLICE
            || *item == pumpkin_data::item::Item::GOLDEN_CARROT
            || *item == pumpkin_data::item::Item::GOLDEN_APPLE
            || *item == pumpkin_data::item::Item::ENCHANTED_GOLDEN_APPLE
            || *item == pumpkin_data::item::Item::LIGHT_WEIGHTED_PRESSURE_PLATE
    }

    #[must_use]
    pub fn is_soul_fire_block(block: &pumpkin_data::Block) -> bool {
        *block == pumpkin_data::Block::SOUL_FIRE
            || *block == pumpkin_data::Block::SOUL_TORCH
            || *block == pumpkin_data::Block::SOUL_WALL_TORCH
            || *block == pumpkin_data::Block::SOUL_LANTERN
            || *block == pumpkin_data::Block::SOUL_CAMPFIRE
    }
}

impl NBTStorage for PiglinEntity {}

impl Mob for PiglinEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;
    use pumpkin_data::item::Item;

    #[test]
    fn vanilla_piglin_safe_armor_detection() {
        let gold_helmet = Item::GOLDEN_HELMET;
        let iron_chestplate = Item::IRON_CHESTPLATE;
        let diamond_boots = Item::DIAMOND_BOOTS;

        assert!(PiglinEntity::is_item_piglin_safe(&gold_helmet));
        assert!(!PiglinEntity::is_item_piglin_safe(&iron_chestplate));
        assert!(!PiglinEntity::is_item_piglin_safe(&diamond_boots));

        let equipped_with_gold = [
            Some(&gold_helmet),
            Some(&iron_chestplate),
            None,
            Some(&diamond_boots),
        ];
        assert!(PiglinEntity::is_player_wearing_safe_armor(
            &equipped_with_gold
        ));

        let equipped_without_gold = [None, Some(&iron_chestplate), None, Some(&diamond_boots)];
        assert!(!PiglinEntity::is_player_wearing_safe_armor(
            &equipped_without_gold
        ));
    }

    #[test]
    fn vanilla_piglin_barter_and_loved_items() {
        assert!(PiglinEntity::is_barter_currency(&Item::GOLD_INGOT));
        assert!(!PiglinEntity::is_barter_currency(&Item::IRON_INGOT));
        assert!(!PiglinEntity::is_barter_currency(&Item::GOLD_NUGGET));

        assert!(PiglinEntity::is_loved_item(&Item::GOLD_INGOT));
        assert!(PiglinEntity::is_loved_item(&Item::GOLDEN_SWORD));
        assert!(PiglinEntity::is_loved_item(&Item::GOLDEN_APPLE));
        assert!(!PiglinEntity::is_loved_item(&Item::DIAMOND));
    }

    #[test]
    fn vanilla_piglin_soul_fire_repellents() {
        assert!(PiglinEntity::is_soul_fire_block(&Block::SOUL_FIRE));
        assert!(PiglinEntity::is_soul_fire_block(&Block::SOUL_TORCH));
        assert!(PiglinEntity::is_soul_fire_block(&Block::SOUL_LANTERN));
        assert!(PiglinEntity::is_soul_fire_block(&Block::SOUL_CAMPFIRE));
        assert!(!PiglinEntity::is_soul_fire_block(&Block::FIRE));
        assert!(!PiglinEntity::is_soul_fire_block(&Block::TORCH));
    }
}
