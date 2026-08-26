use std::borrow::Cow;

use pumpkin_data::{
    Enchantment,
    data_component::DataComponent,
    data_component_impl::{EnchantableImpl, StoredEnchantmentsImpl},
    item::Item,
    item_stack::ItemStack,
    tag::Taggable,
};
use pumpkin_util::random::RandomImpl;

/// Generates the same weighted, compatible enchantment list used by
/// `EnchantmentHelper.selectEnchantment` in Vanilla.
pub fn generate_enchantments(
    random: &mut impl RandomImpl,
    item: &ItemStack,
    level: i32,
    options: &str,
) -> Vec<(&'static Enchantment, i32)> {
    let enchantability = item
        .get_data_component::<EnchantableImpl>()
        .map_or(0, |component| component.value);
    let mut enchant_level = level
        + 1
        + random.next_bounded_i32(enchantability / 4 + 1)
        + random.next_bounded_i32(enchantability / 4 + 1);
    let bonus = (random.next_f32() + random.next_f32() - 1.0) * 0.15;
    enchant_level = (enchant_level as f32 * (1.0 + bonus)).round() as i32;
    enchant_level = enchant_level.max(1);

    let mut available = Vec::new();
    for enchantment in Enchantment::all() {
        if enchantment.is_tagged_with(options).unwrap_or(false)
            && (item.item == &Item::BOOK || enchantment.can_enchant(item.item))
        {
            for enchantment_level in (1..=enchantment.max_level).rev() {
                if enchant_level >= enchantment.min_cost.calculate(enchantment_level)
                    && enchant_level <= enchantment.max_cost.calculate(enchantment_level)
                {
                    available.push((*enchantment, enchantment_level));
                    break;
                }
            }
        }
    }

    let mut result = Vec::new();
    if let Some(selected) = select_weighted(random, &available) {
        result.push(selected);
        let mut current_level = enchant_level;
        while random.next_bounded_i32(50) <= (current_level + 1) / 2 {
            available.retain(|(candidate, _)| {
                result
                    .iter()
                    .all(|(selected, _)| candidate.are_compatible(selected))
            });
            let Some(selected) = select_weighted(random, &available) else {
                break;
            };
            result.push(selected);
            current_level /= 2;
        }
    }
    result
}

fn select_weighted(
    random: &mut impl RandomImpl,
    choices: &[(&'static Enchantment, i32)],
) -> Option<(&'static Enchantment, i32)> {
    let total_weight = choices
        .iter()
        .map(|(enchantment, _)| enchantment.weight)
        .sum();
    if total_weight <= 0 {
        return None;
    }
    let mut target = random.next_bounded_i32(total_weight);
    choices.iter().copied().find(|(enchantment, _)| {
        target -= enchantment.weight;
        target < 0
    })
}

/// Applies a level-based loot enchantment. Vanilla converts an ordinary book
/// to an enchanted book and stores its enchantments in the dedicated component.
pub fn enchant_with_levels(
    stack: ItemStack,
    level: i32,
    options: &str,
    random: &mut impl RandomImpl,
) -> ItemStack {
    let enchantments = generate_enchantments(random, &stack, level, options);
    apply_enchantments(stack, enchantments)
}

pub fn apply_enchantments(
    mut stack: ItemStack,
    enchantments: Vec<(&'static Enchantment, i32)>,
) -> ItemStack {
    if stack.item == &Item::BOOK {
        return ItemStack::new_with_component(
            stack.item_count,
            &Item::ENCHANTED_BOOK,
            vec![(
                DataComponent::StoredEnchantments,
                Some(Box::new(StoredEnchantmentsImpl {
                    enchantment: Cow::Owned(enchantments),
                })),
            )],
        );
    }
    for (enchantment, level) in enchantments {
        stack.add_enchantment(enchantment, level as u16);
    }
    stack
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::data_component_impl::StoredEnchantmentsImpl;
    use pumpkin_util::random::legacy_rand::LegacyRand;

    #[test]
    fn vanilla_loot_enchanting_converts_books_and_adds_stored_enchantments() {
        let mut random = LegacyRand::from_seed(42);
        let stack = enchant_with_levels(
            ItemStack::new(1, &Item::BOOK),
            30,
            "#minecraft:on_random_loot",
            &mut random,
        );
        assert_eq!(stack.item.id, Item::ENCHANTED_BOOK.id);
        assert!(
            stack
                .get_data_component::<StoredEnchantmentsImpl>()
                .is_some_and(|component| !component.enchantment.is_empty())
        );
    }
}
