use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::entity::player::Player;
use crate::entity::projectile::arrow::{ArrowEntity, ArrowPickup};
use crate::entity::projectile::firework_rocket::FireworkRocketEntity;
use crate::entity::{Entity, EntityBase};
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::data_component::DataComponent;
use pumpkin_data::data_component_impl::{ChargedProjectilesImpl, EnchantmentsImpl};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::GameMode;
use pumpkin_world::inventory::Inventory;

pub struct CrossbowItem;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrossbowProjectileKind {
    Arrow,
    Firework,
}

const fn projectile_kind(item: &'static Item) -> CrossbowProjectileKind {
    if item.id == Item::FIREWORK_ROCKET.id {
        CrossbowProjectileKind::Firework
    } else {
        CrossbowProjectileKind::Arrow
    }
}

const fn durability_cost(kind: CrossbowProjectileKind) -> i32 {
    match kind {
        CrossbowProjectileKind::Arrow => 1,
        CrossbowProjectileKind::Firework => 3,
    }
}

const fn projectile_pickup_rule(creative_shooter: bool, shot_index: usize) -> ArrowPickup {
    if creative_shooter || shot_index > 0 {
        ArrowPickup::CreativeOnly
    } else {
        ArrowPickup::Allowed
    }
}

fn shot_yaw(base_yaw: f32, projectile_count: usize, shot_index: usize) -> f32 {
    if projectile_count < 3 || shot_index == 0 {
        base_yaw
    } else if shot_index == 1 {
        base_yaw - 10.0
    } else {
        base_yaw + 10.0
    }
}

const fn projectile_load_count(has_multishot: bool) -> usize {
    if has_multishot { 3 } else { 1 }
}

impl ItemMetadata for CrossbowItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::CROSSBOW.id])
    }
}

impl ItemBehaviour for CrossbowItem {
    fn normal_use<'a>(
        &'a self,
        _item: &'a Item,
        player: &'a Player,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let inventory = player.inventory();
            let stack = inventory.held_item().await;

            // Every crossbow carries a ChargedProjectiles component by default, so its mere
            // presence does not mean the crossbow is loaded. Vanilla checks the list is also
            // non-empty (CrossbowItem.java:68).
            if stack
                .get_data_component::<ChargedProjectilesImpl>()
                .is_some_and(|charged| !charged.projectiles.is_empty())
            {
                Self::fire_projectiles(player).await;
                return;
            }

            let has_projectile = player.find_crossbow_projectile().await.is_some();
            if !has_projectile && player.gamemode.load() != GameMode::Creative {
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
            let use_ticks = player.living_entity.item_use_time.load(Ordering::Relaxed);
            let use_ticks = 72000 - use_ticks;

            let mut charge_time = 25;
            let mut has_multishot = false;
            let mut stack = player.inventory().held_item().await;

            if let Some(enchantments) = stack.get_data_component::<EnchantmentsImpl>() {
                for (enchantment, level) in enchantments.enchantment.iter() {
                    if **enchantment == pumpkin_data::Enchantment::QUICK_CHARGE {
                        charge_time -= 5 * level;
                    } else if **enchantment == pumpkin_data::Enchantment::MULTISHOT {
                        has_multishot = true;
                    }
                }
            }
            charge_time = charge_time.max(0);

            if use_ticks >= charge_time {
                let projectile_slot = player.find_crossbow_projectile().await;
                let (projectile_nbt_wrapper, slot) = {
                    if let Some(slot) = projectile_slot {
                        let inventory = player.inventory();

                        let arrow_stack = inventory.get_stack(slot).await;
                        let mut arrow_nbt = pumpkin_nbt::compound::NbtCompound::new();
                        arrow_stack
                            .copy_with_count(1)
                            .write_item_stack(&mut arrow_nbt);
                        (Some(arrow_nbt), slot)
                    } else if player.gamemode.load() == GameMode::Creative {
                        let mut arrow_nbt = pumpkin_nbt::compound::NbtCompound::new();
                        let arrow_stack = ItemStack::new(1, &Item::ARROW);
                        arrow_stack.write_item_stack(&mut arrow_nbt);

                        (Some(arrow_nbt), 0)
                    } else {
                        (None, 0)
                    }
                };
                if let Some(projectile_nbt) = projectile_nbt_wrapper {
                    stack.patch.push((
                        DataComponent::ChargedProjectiles,
                        Some(Box::new(ChargedProjectilesImpl {
                            projectiles: vec![projectile_nbt; projectile_load_count(has_multishot)],
                        })),
                    ));
                    player.inventory().set_held_item(stack).await;

                    if player.gamemode.load() != GameMode::Creative {
                        player.consume_arrow(slot).await;
                    }

                    player.world().play_sound(
                        Sound::ItemCrossbowLoadingEnd,
                        SoundCategory::Players,
                        &player.position(),
                    );
                }
            }
            player.living_entity.clear_active_hand().await;
        })
    }

    fn get_use_duration(&self) -> i32 {
        72000
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl CrossbowItem {
    async fn fire_projectiles(player: &Player) {
        let mut held = player.inventory().held_item().await;
        let projectiles = held.get_data_component::<ChargedProjectilesImpl>().cloned();
        let piercing_level =
            held.get_data_component::<EnchantmentsImpl>()
                .map_or(0, |enchantments| {
                    let mut piercing = 0;
                    for (enchantment, level) in enchantments.enchantment.iter() {
                        if **enchantment == pumpkin_data::Enchantment::PIERCING {
                            piercing = (*level).clamp(0, u8::MAX as i32) as u8;
                        }
                    }
                    piercing
                });

        if let Some(charged) = projectiles {
            let world = player.world();
            world.play_sound(
                Sound::ItemCrossbowShoot,
                SoundCategory::Players,
                &player.position(),
            );

            let (yaw, pitch) = player.rotation();
            let mut fired_kind = CrossbowProjectileKind::Arrow;

            let projectile_count = charged.projectiles.len();
            for (index, projectile_nbt) in charged.projectiles.into_iter().enumerate() {
                let Some(projectile) = ItemStack::read_item_stack(&projectile_nbt) else {
                    continue;
                };
                let kind = projectile_kind(projectile.item);
                if kind == CrossbowProjectileKind::Firework {
                    fired_kind = CrossbowProjectileKind::Firework;
                }
                let projectile_yaw = shot_yaw(yaw, projectile_count, index);
                if kind == CrossbowProjectileKind::Firework {
                    let rocket_entity = Entity::new(
                        world.clone(),
                        player.position(),
                        &pumpkin_data::entity::EntityType::FIREWORK_ROCKET,
                    );
                    let rocket = FireworkRocketEntity::new_crossbow_shot(
                        rocket_entity,
                        player.get_entity(),
                        &projectile,
                    );
                    rocket.set_velocity_from_rotation(pitch, projectile_yaw, 1.6, 1.0);
                    let rocket: Arc<dyn EntityBase> = Arc::new(rocket);
                    world.spawn_entity(rocket).await;
                    continue;
                }
                let arrow_entity = Entity::new(
                    world.clone(),
                    player.position(),
                    ArrowEntity::entity_type_for_item(projectile.item),
                );
                let pickup =
                    projectile_pickup_rule(player.gamemode.load() == GameMode::Creative, index);

                let arrow =
                    ArrowEntity::new_shot(arrow_entity, player.get_entity(), &projectile, pickup);
                arrow.set_pierce_level(piercing_level);
                arrow.set_velocity_from_rotation(pitch, projectile_yaw, 0.0, 3.15, 1.0);
                let arrow_arc: Arc<dyn EntityBase> = Arc::new(arrow);
                world.spawn_entity(arrow_arc).await;
            }

            held.patch
                .retain(|(id, _)| *id != DataComponent::ChargedProjectiles);
            player.inventory().set_held_item(held).await;
            player.damage_held_item(durability_cost(fired_kind)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CrossbowProjectileKind, durability_cost, projectile_kind, projectile_load_count,
        projectile_pickup_rule, shot_yaw,
    };
    use crate::entity::projectile::arrow::ArrowPickup;
    use pumpkin_data::item::Item;

    #[test]
    fn survival_multishot_only_allows_center_arrow_pickup() {
        assert_eq!(projectile_pickup_rule(false, 0), ArrowPickup::Allowed);
        assert_eq!(projectile_pickup_rule(false, 1), ArrowPickup::CreativeOnly);
        assert_eq!(projectile_pickup_rule(false, 2), ArrowPickup::CreativeOnly);
        assert_eq!(projectile_pickup_rule(false, 0), ArrowPickup::Allowed);
        assert_eq!(shot_yaw(30.0, 3, 0), 30.0);
        assert_eq!(shot_yaw(30.0, 3, 1), 20.0);
        assert_eq!(shot_yaw(30.0, 3, 2), 40.0);
        assert_eq!(shot_yaw(30.0, 1, 0), 30.0);
        assert_eq!(projectile_load_count(false), 1);
        assert_eq!(projectile_load_count(true), 3);
    }

    #[test]
    fn fireworks_keep_their_projectile_type_and_cost_three_durability() {
        assert_eq!(
            projectile_kind(&Item::FIREWORK_ROCKET),
            CrossbowProjectileKind::Firework
        );
        assert_eq!(durability_cost(CrossbowProjectileKind::Firework), 3);
        assert_eq!(projectile_kind(&Item::ARROW), CrossbowProjectileKind::Arrow);
        assert_eq!(durability_cost(CrossbowProjectileKind::Arrow), 1);
    }
}
