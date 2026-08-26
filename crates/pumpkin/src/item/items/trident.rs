use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::entity::player::Player;
use crate::entity::projectile::arrow::ArrowPickup;
use crate::entity::projectile::trident::TridentEntity;
use crate::entity::{Entity, EntityBase};
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;
use pumpkin_util::GameMode;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::inventory::Inventory;

pub struct TridentItem;

const AUTO_SPIN_ATTACK_TICKS: u8 = 20;

fn riptide_launch_speed(level: u32) -> f64 {
    3.0 * f64::from(level + 1) / 4.0
}

fn riptide_sound(level: u32) -> Sound {
    match level {
        0 | 1 => Sound::ItemTridentRiptide1,
        2 => Sound::ItemTridentRiptide2,
        _ => Sound::ItemTridentRiptide3,
    }
}

fn next_damage_will_break(stack: &ItemStack) -> bool {
    !stack.is_unbreakable()
        && stack
            .get_max_damage()
            .is_some_and(|max| stack.get_damage().saturating_add(1) >= max)
}

impl ItemMetadata for TridentItem {
    fn ids() -> Box<[u16]> {
        [Item::TRIDENT.id].into()
    }
}

impl ItemBehaviour for TridentItem {
    fn normal_use<'a>(
        &'a self,
        _item: &'a Item,
        player: &'a Player,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let inventory = player.inventory();
            let stack = inventory.held_item().await;

            // TridentItem#use rejects a stack whose next durability point would
            // destroy it. This is checked before the client starts charging.
            if next_damage_will_break(&stack) {
                return;
            }

            player
                .living_entity
                .set_active_hand(pumpkin_util::Hand::Right, stack, 72000)
                .await;
        })
    }

    fn on_stopped_using<'a>(
        &'a self,
        _stack: &'a ItemStack,
        player: &'a Player,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let use_ticks = player
                .living_entity
                .item_use_time
                .load(std::sync::atomic::Ordering::Relaxed);
            let use_ticks = 72000 - use_ticks;

            if use_ticks < 10 {
                return;
            }

            let world = player.world();
            let stack_guard = player.inventory().held_item().await;

            // Check Riptide level
            let mut riptide_level = 0u32;
            if let Some(enchantments) = stack_guard
                .get_data_component::<pumpkin_data::data_component_impl::EnchantmentsImpl>(
            ) {
                for (enchantment, level) in enchantments.enchantment.iter() {
                    if **enchantment == pumpkin_data::Enchantment::RIPTIDE {
                        riptide_level = *level as u32;
                    }
                }
            }

            if riptide_level > 0 {
                let entity = player.get_entity();
                let position = player.position();
                let wet = entity
                    .touching_water
                    .load(std::sync::atomic::Ordering::Relaxed)
                    || world.is_raining_at(&position.to_block_pos()).await;
                if !wet {
                    player.living_entity.clear_active_hand().await;
                    return;
                }

                let (yaw, pitch) = player.rotation();
                let f_yaw = f32::to_radians(yaw);
                let f_pitch = f32::to_radians(pitch);

                let vx = f64::from(-f32::sin(f_yaw) * f32::cos(f_pitch));
                let vy = f64::from(-f32::sin(f_pitch));
                let vz = f64::from(f32::cos(f_yaw) * f32::cos(f_pitch));

                let sq = (vx * vx + vy * vy + vz * vz).sqrt();
                if sq > 0.0 {
                    let mult = riptide_launch_speed(riptide_level) / sq;
                    entity
                        .velocity
                        .store(Vector3::new(vx * mult, vy * mult, vz * mult));
                }

                player.damage_held_item(1).await;
                player
                    .auto_spin_attack_ticks
                    .store(AUTO_SPIN_ATTACK_TICKS, std::sync::atomic::Ordering::Relaxed);
                entity.set_pose(pumpkin_data::entity::EntityPose::SpinAttack);
                world.play_sound(
                    riptide_sound(riptide_level),
                    pumpkin_data::sound::SoundCategory::Players,
                    &position,
                );
                player
                    .increment_stat(
                        pumpkin_data::statistic::StatisticCategory::Used,
                        Item::TRIDENT.id.into(),
                        1,
                    )
                    .await;
                player.living_entity.clear_active_hand().await;
                return;
            }

            // Normal throw - spawn thrown trident
            let (yaw, pitch) = player.rotation();
            let entity = Entity::new(world.clone(), player.position(), &EntityType::TRIDENT);
            let mut thrown_stack = stack_guard.clone();
            let _ = thrown_stack.damage_item(1);
            let trident = TridentEntity::new_shot(
                entity,
                player.get_entity(),
                thrown_stack,
                ArrowPickup::Allowed,
            );
            trident.set_velocity_from_rotation(pitch, yaw, 0.0, 2.5, 1.0);
            world.spawn_entity(Arc::new(trident)).await;

            world.play_sound(
                Sound::ItemTridentThrow,
                pumpkin_data::sound::SoundCategory::Players,
                &player.position(),
            );

            player
                .increment_stat(
                    pumpkin_data::statistic::StatisticCategory::Used,
                    Item::TRIDENT.id.into(),
                    1,
                )
                .await;

            if player.gamemode.load() != GameMode::Creative {
                let inventory = player.inventory();
                let selected_slot = inventory.get_selected_slot() as usize;

                let main_hand_item = inventory.get_stack(selected_slot).await;
                if main_hand_item.item.id == Item::TRIDENT.id {
                    inventory
                        .set_stack(selected_slot, ItemStack::EMPTY.clone())
                        .await;
                    player
                        .sync_hand_slot(selected_slot, ItemStack::EMPTY.clone())
                        .await;
                } else {
                    let off_hand_slot =
                        pumpkin_inventory::player::player_inventory::PlayerInventory::OFF_HAND_SLOT;
                    let off_hand_item = inventory.get_stack(off_hand_slot).await;
                    if off_hand_item.item.id == Item::TRIDENT.id {
                        inventory
                            .set_stack(off_hand_slot, ItemStack::EMPTY.clone())
                            .await;
                        player
                            .sync_hand_slot(off_hand_slot, ItemStack::EMPTY.clone())
                            .await;
                    }
                }
            }

            player.living_entity.clear_active_hand().await;
        })
    }

    fn can_mine(&self, player: &Player) -> bool {
        player.gamemode.load() != GameMode::Creative
    }

    fn get_use_duration(&self) -> i32 {
        72000
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{next_damage_will_break, riptide_launch_speed, riptide_sound};
    use pumpkin_data::{item::Item, item_stack::ItemStack, sound::Sound};

    #[test]
    fn vanilla_riptide_launch_speeds() {
        assert_eq!(riptide_launch_speed(1), 1.5);
        assert_eq!(riptide_launch_speed(2), 2.25);
        assert_eq!(riptide_launch_speed(3), 3.0);
    }

    #[test]
    fn vanilla_riptide_sounds_follow_enchantment_level() {
        assert_eq!(riptide_sound(1), Sound::ItemTridentRiptide1);
        assert_eq!(riptide_sound(2), Sound::ItemTridentRiptide2);
        assert_eq!(riptide_sound(3), Sound::ItemTridentRiptide3);
    }

    #[test]
    fn vanilla_rejects_trident_use_when_next_damage_would_break_it() {
        let mut trident = ItemStack::new(1, &Item::TRIDENT);
        let max_damage = trident.get_max_damage().expect("trident is damageable");

        trident.set_damage(max_damage - 2);
        assert!(!next_damage_will_break(&trident));

        trident.set_damage(max_damage - 1);
        assert!(next_damage_will_break(&trident));
    }

    #[test]
    fn thrown_trident_carries_one_point_of_durability_damage() {
        let trident = ItemStack::new(1, &Item::TRIDENT);
        let mut projectile_stack = trident.clone();
        let _ = projectile_stack.damage_item(1);

        assert_eq!(trident.get_damage(), 0);
        assert_eq!(projectile_stack.get_damage(), 1);
    }
}
