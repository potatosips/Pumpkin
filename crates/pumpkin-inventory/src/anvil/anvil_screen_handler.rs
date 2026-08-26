use std::any::Any;
use std::sync::Arc;

use pumpkin_data::{item_stack::ItemStack, screen::WindowType};
use pumpkin_world::inventory::Inventory;

use crate::{
    player::player_inventory::PlayerInventory,
    screen_handler::{
        InventoryPlayer, ItemStackFuture, ScreenHandler, ScreenHandlerBehaviour,
        ScreenHandlerFuture, offer_or_drop_stack,
    },
    slot::NormalSlot,
    window_property::{Anvil, WindowProperty},
};

pub struct AnvilScreenHandler {
    pub inventory: Arc<dyn Inventory>,
    behaviour: ScreenHandlerBehaviour,
    pub rename_text: String,
    pub repair_cost: i16,
}

impl AnvilScreenHandler {
    #[expect(clippy::needless_pass_by_value)]
    pub fn new(
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        inventory: Arc<dyn Inventory>,
    ) -> Self {
        let mut handler = Self {
            inventory: inventory.clone(),
            behaviour: ScreenHandlerBehaviour::new(sync_id, Some(WindowType::Anvil)),
            rename_text: String::new(),
            repair_cost: 0,
        };

        // Anvil specific slots: 2 input, 1 output
        for i in 0..3 {
            handler.add_slot(Arc::new(NormalSlot::new(inventory.clone(), i)));
        }

        let player_inventory: Arc<dyn Inventory> = player_inventory.clone();
        handler.add_player_slots(&player_inventory);

        handler
    }

    pub async fn update_item_name(&mut self, name: String) {
        self.rename_text = name;
        self.update_result_slot().await;
        self.send_content_updates().await;
    }

    pub async fn update_result_slot(&mut self) {
        let input_a = self.inventory.get_stack(0).await;

        if input_a.is_empty() {
            self.inventory.set_stack(2, ItemStack::EMPTY.clone()).await;
            self.set_repair_cost(0).await;
            return;
        }

        let input_b = self.inventory.get_stack(1).await;
        let mut result_item = input_a.clone();

        let mut repair_cost = 0;
        let mut renamed = false;

        use pumpkin_data::data_component::DataComponent;
        use pumpkin_data::data_component_impl::EnchantmentsImpl;
        use pumpkin_data::data_component_impl::StoredEnchantmentsImpl;

        let a_penalty = input_a
            .get_custom_data("pumpkin", "repair_cost")
            .and_then(|t| t.extract_int())
            .unwrap_or(0);
        let b_penalty = if input_b.is_empty() {
            0
        } else {
            input_b
                .get_custom_data("pumpkin", "repair_cost")
                .and_then(|t| t.extract_int())
                .unwrap_or(0)
        };
        let base_cost = Self::calculate_prior_work_penalty(a_penalty, b_penalty);

        // Rename logic
        if !self.rename_text.is_empty() {
            result_item.set_custom_name(self.rename_text.clone());
            renamed = true;
        }

        if !input_b.is_empty() {
            let is_book = input_b.item == &pumpkin_data::item::Item::ENCHANTED_BOOK;

            if input_a.item == input_b.item && input_a.is_damageable() {
                // Item combination repair
                let a_max = input_a.get_max_damage().unwrap_or(0);
                let a_damage = input_a.get_damage();
                let a_remaining = a_max - a_damage;
                let b_damage = input_b.get_damage();
                let b_remaining = a_max - b_damage;

                let bonus = (a_max as f32 * 0.12).floor() as i32;
                let new_remaining = a_remaining + b_remaining + bonus;
                let new_damage = a_max - new_remaining.min(a_max);
                result_item.set_damage(new_damage.max(0));

                repair_cost += 2;
            }

            // Transfer enchantments
            if is_book || input_a.item == input_b.item {
                let mut result_enchs = result_item
                    .get_data_component::<EnchantmentsImpl>()
                    .map(|e| e.enchantment.to_vec())
                    .unwrap_or_default();

                let b_enchs = if is_book {
                    input_b
                        .get_data_component::<StoredEnchantmentsImpl>()
                        .map(|e| e.enchantment.to_vec())
                        .unwrap_or_default()
                } else {
                    input_b
                        .get_data_component::<EnchantmentsImpl>()
                        .map(|e| e.enchantment.to_vec())
                        .unwrap_or_default()
                };

                let mut any_enchantment_added = false;

                for (enc, b_level) in b_enchs {
                    let mut found = false;
                    let mut cost_per_level = match enc.weight {
                        w if w <= 1 => 8,
                        w if w <= 2 => 4,
                        w if w <= 5 => 2,
                        _ => 1,
                    };
                    if is_book {
                        cost_per_level = (cost_per_level / 2).max(1);
                    }

                    for (r_enc, r_level) in &mut result_enchs {
                        if r_enc.name == enc.name {
                            // Same enchantment
                            let prev_level = *r_level;
                            if prev_level == b_level {
                                *r_level = (prev_level + 1).min(enc.max_level as i32);
                            } else {
                                *r_level = prev_level.max(b_level);
                            }
                            repair_cost += cost_per_level * (*r_level);
                            found = true;
                            any_enchantment_added = true;
                            break;
                        }
                    }

                    if !found {
                        // Check compatibility
                        let mut compatible = true;
                        for (r_enc, _) in &result_enchs {
                            if !r_enc.are_compatible(enc) {
                                compatible = false;
                                break;
                            }
                        }
                        if compatible {
                            result_enchs.push((enc, b_level));
                            repair_cost += cost_per_level * b_level;
                            any_enchantment_added = true;
                        }
                    }
                }

                // Apply enchantments back
                if any_enchantment_added {
                    if let Some(pos) = result_item
                        .patch
                        .iter()
                        .position(|(id, _)| *id == DataComponent::Enchantments)
                    {
                        result_item.patch[pos].1 = Some(Box::new(EnchantmentsImpl {
                            enchantment: std::borrow::Cow::Owned(result_enchs),
                        }));
                    } else {
                        result_item.patch.push((
                            DataComponent::Enchantments,
                            Some(Box::new(EnchantmentsImpl {
                                enchantment: std::borrow::Cow::Owned(result_enchs),
                            })),
                        ));
                    }
                }
            }
        }

        let cost = Self::calculate_anvil_cost(base_cost, repair_cost, renamed);

        if cost > 0 {
            // Apply new prior work penalty
            let new_penalty = Self::calculate_next_repair_cost(a_penalty.max(b_penalty));
            result_item.set_custom_data(
                "pumpkin",
                "repair_cost",
                pumpkin_nbt::tag::NbtTag::Int(new_penalty),
            );

            self.inventory.set_stack(2, result_item).await;
            self.set_repair_cost(cost as i16).await;
        } else {
            self.inventory.set_stack(2, ItemStack::EMPTY.clone()).await;
            self.set_repair_cost(0).await;
        }
    }

    #[must_use]
    pub const fn calculate_next_repair_cost(k: i32) -> i32 {
        (k * 2) + 1
    }

    #[must_use]
    pub const fn calculate_prior_work_penalty(left_cost: i32, right_cost: i32) -> i32 {
        left_cost + right_cost
    }

    #[must_use]
    pub const fn calculate_anvil_cost(base_cost: i32, repair_cost: i32, renamed: bool) -> i32 {
        base_cost + repair_cost + if renamed { 1 } else { 0 }
    }

    pub async fn set_repair_cost(&mut self, cost: i16) {
        self.repair_cost = cost;
        if let Some(sync_handler) = self.behaviour.sync_handler.as_ref() {
            let (property_id, property_value) =
                WindowProperty::new(Anvil::RepairCost, cost).into_tuple();
            sync_handler
                .update_property(&self.behaviour, property_id as i32, property_value as i32)
                .await;
        }
    }
}

impl ScreenHandler for AnvilScreenHandler {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn get_behaviour(&self) -> &ScreenHandlerBehaviour {
        &self.behaviour
    }

    fn get_behaviour_mut(&mut self) -> &mut ScreenHandlerBehaviour {
        &mut self.behaviour
    }

    fn on_closed<'a>(&'a mut self, player: &'a dyn InventoryPlayer) -> ScreenHandlerFuture<'a, ()> {
        Box::pin(async move {
            self.default_on_closed(player).await;
            self.inventory.on_close().await;
            // Drop inputs from anvil
            for i in 0..2 {
                let stack = self.inventory.remove_stack(i).await;
                if !stack.is_empty() {
                    offer_or_drop_stack(player, stack).await;
                }
            }
            self.inventory.set_stack(2, ItemStack::EMPTY.clone()).await;
        })
    }

    fn quick_move<'a>(
        &'a mut self,
        player: &'a dyn InventoryPlayer,
        slot_index: i32,
    ) -> ItemStackFuture<'a> {
        Box::pin(async move {
            let mut stack_left = ItemStack::EMPTY.clone();
            let slot = self.get_behaviour().slots[slot_index as usize].clone();

            if slot.has_stack().await {
                let mut slot_stack = slot.get_stack().await;
                stack_left = slot_stack.clone();

                if slot_index < 3 {
                    // From anvil to player
                    if !self.insert_item(&mut slot_stack, 3, 39, true).await {
                        return ItemStack::EMPTY.clone();
                    }
                    slot.on_quick_move_crafted(slot_stack.clone(), stack_left.clone())
                        .await;
                } else {
                    // From player to anvil
                    if !self.insert_item(&mut slot_stack, 0, 2, false).await {
                        return ItemStack::EMPTY.clone();
                    }
                }

                if slot_stack.item_count == stack_left.item_count {
                    return ItemStack::EMPTY.clone();
                }

                slot.set_stack_prev(slot_stack.clone(), stack_left.clone())
                    .await;
                slot.on_take_item(player, &slot_stack).await;
                slot.mark_dirty().await;
            }

            stack_left
        })
    }

    fn on_slot_click<'a>(
        &'a mut self,
        slot_index: i32,
        button: i32,
        action_type: pumpkin_protocol::java::server::play::SlotActionType,
        player: &'a dyn InventoryPlayer,
    ) -> ScreenHandlerFuture<'a, ()> {
        Box::pin(async move {
            if slot_index == 2 {
                // Taking from output slot
                let result_slot = self.get_behaviour().slots[2].clone();
                if result_slot.has_stack().await {
                    let result_stack = result_slot.get_cloned_stack().await;
                    if !result_stack.is_empty() {
                        let is_creative = player.is_creative();
                        if (self.repair_cost < 40 || is_creative)
                            && (player.experience_level() >= self.repair_cost as i32 || is_creative)
                        {
                            // Consume experience
                            if !player.is_creative() {
                                player
                                    .add_experience_levels(-(self.repair_cost as i32))
                                    .await;
                            }

                            // Consume inputs
                            self.inventory.set_stack(0, ItemStack::EMPTY.clone()).await;
                            self.get_behaviour().slots[0].mark_dirty().await;
                            self.inventory.set_stack(1, ItemStack::EMPTY.clone()).await;
                            self.get_behaviour().slots[1].mark_dirty().await;
                        } else {
                            // Cancel click
                            self.send_content_updates().await;
                            return;
                        }
                    }
                }
            }

            self.internal_on_slot_click(slot_index, button, action_type, player)
                .await;
            if slot_index == 0 || slot_index == 1 || slot_index == 2 {
                self.update_result_slot().await;
                self.send_content_updates().await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::AnvilScreenHandler;

    #[test]
    fn vanilla_anvil_prior_work_penalty_exponential_scaling() {
        assert_eq!(AnvilScreenHandler::calculate_next_repair_cost(0), 1);
        assert_eq!(AnvilScreenHandler::calculate_next_repair_cost(1), 3);
        assert_eq!(AnvilScreenHandler::calculate_next_repair_cost(3), 7);
        assert_eq!(AnvilScreenHandler::calculate_next_repair_cost(7), 15);
        assert_eq!(AnvilScreenHandler::calculate_next_repair_cost(15), 31);
        assert_eq!(AnvilScreenHandler::calculate_next_repair_cost(31), 63);
    }

    #[test]
    fn vanilla_anvil_cost_composition_and_penalties() {
        let left_penalty = 3;
        let right_penalty = 1;
        let base_penalty =
            AnvilScreenHandler::calculate_prior_work_penalty(left_penalty, right_penalty);
        assert_eq!(base_penalty, 4);

        let repair_cost = 2;
        let renamed = true;
        let total_cost =
            AnvilScreenHandler::calculate_anvil_cost(base_penalty, repair_cost, renamed);
        assert_eq!(total_cost, 7); // 4 (penalties) + 2 (repair) + 1 (rename)
    }
}
