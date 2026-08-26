use std::{any::Any, future::Future, pin::Pin};

use crate::entity::{EntityBase, area_effect_cloud::AreaEffectCloudEntity, player::Player};
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use pumpkin_data::data_component_impl::PotionContentsImpl;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::potion::Potion;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockDirection, BlockStateId};
use pumpkin_protocol::java::client::play::Metadata;
use pumpkin_util::Hand;
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};

pub struct GlassBottleItem;

impl ItemMetadata for GlassBottleItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::GLASS_BOTTLE.id])
    }
}

fn is_water_source(block: &Block, state_id: BlockStateId) -> bool {
    block.id == Block::WATER.id && state_id == Block::WATER.default_state.id
}

fn water_potion() -> ItemStack {
    let mut stack = ItemStack::new(1, &Item::POTION);
    stack
        .get_data_component_mut::<PotionContentsImpl>()
        .expect("potions always have potion contents")
        .potion_id = Some(i32::from(Potion::WATER.id));
    stack
}

async fn bottle_in_hand(player: &Player) -> Option<(ItemStack, Hand)> {
    let inventory = player.inventory();
    let main = inventory.held_item().await;
    if main.item.id == Item::GLASS_BOTTLE.id {
        Some((main, Hand::Right))
    } else {
        let off = inventory.off_hand_item().await;
        (off.item.id == Item::GLASS_BOTTLE.id).then_some((off, Hand::Left))
    }
}

async fn turn_bottle_into(player: &Player, mut bottles: ItemStack, hand: Hand, result: ItemStack) {
    if player.gamemode.load() == pumpkin_util::GameMode::Creative {
        player.inventory.offer_or_drop_stack(result, player).await;
        return;
    }
    bottles.decrement(1);
    if bottles.is_empty() {
        player.inventory.set_stack_in_hand(hand, result).await;
    } else {
        player.inventory.set_stack_in_hand(hand, bottles).await;
        player.inventory.offer_or_drop_stack(result, player).await;
    }
}

impl GlassBottleItem {
    async fn try_collect_dragon_breath(player: &Player) -> bool {
        let world = player.world();
        let search_box = player
            .get_entity()
            .bounding_box
            .load()
            .expand(2.0, 2.0, 2.0);
        for entity in world.get_entities_at_box(&search_box) {
            if *entity.get_entity().entity_type != EntityType::AREA_EFFECT_CLOUD
                || !entity.get_entity().is_alive()
            {
                continue;
            }
            let Some(cloud) = entity.cast_any().downcast_ref::<AreaEffectCloudEntity>() else {
                continue;
            };
            // Dragon clouds carry this marker stack; lingering-potion clouds carry a potion.
            if cloud.item_stack.lock().await.item.id != Item::DRAGON_BREATH.id {
                continue;
            }
            let Some((bottles, hand)) = bottle_in_hand(player).await else {
                return false;
            };
            let new_radius = {
                let mut radius = cloud.radius.lock().await;
                *radius -= 0.5;
                *radius
            };
            cloud.entity.send_meta_data(
                &[Metadata::new(
                    pumpkin_data::tracked_data::area_effect_cloud::RADIUS,
                    new_radius,
                )],
                None,
            );
            world.play_sound_fine(
                Sound::ItemBottleFillDragonbreath,
                SoundCategory::Players,
                &player.position(),
                1.0,
                1.0,
            );
            turn_bottle_into(
                player,
                bottles,
                hand,
                ItemStack::new(1, &Item::DRAGON_BREATH),
            )
            .await;
            return true;
        }
        false
    }

    async fn try_collect_water(player: &Player) -> bool {
        let world = player.world();
        let (start, end) = <Self as ItemBehaviour>::get_start_and_end_pos(&Self, player);
        let checker = async |pos: &BlockPos, world: &std::sync::Arc<crate::world::World>| {
            let state_id = world.get_block_state_id(pos);
            let block = Block::from_state_id(state_id);
            is_water_source(block, state_id)
                || (state_id != Block::AIR.default_state.id
                    && block.id != Block::WATER.id
                    && block.id != Block::LAVA.id)
        };
        let Some((pos, _)) = world.raycast(start, end, checker).await else {
            return false;
        };
        let (block, state_id) = world.get_block_and_state_id(&pos);
        if !is_water_source(block, state_id) {
            return false;
        }
        let Some((bottles, hand)) = bottle_in_hand(player).await else {
            return false;
        };
        world.play_sound_fine(
            Sound::ItemBottleFill,
            SoundCategory::Players,
            &player.position(),
            1.0,
            1.0,
        );
        turn_bottle_into(player, bottles, hand, water_potion()).await;
        true
    }
}

impl ItemBehaviour for GlassBottleItem {
    fn normal_use<'a>(
        &'a self,
        _item: &'a Item,
        player: &'a Player,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if !Self::try_collect_dragon_breath(player).await {
                Self::try_collect_water(player).await;
            }
        })
    }

    // Bedrock can route water clicks here. Java uses normal_use; cauldrons consume
    // their interaction before this hook.
    fn use_on_block<'a>(
        &'a self,
        item: &'a mut ItemStack,
        player: &'a Player,
        location: BlockPos,
        face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        block: &'a Block,
        _server: &'a Server,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let pos = if block.id == Block::WATER.id {
                location
            } else {
                location.offset(face.to_offset())
            };
            let world = player.world();
            let (target, state_id) = world.get_block_and_state_id(&pos);
            if !is_water_source(target, state_id) {
                return;
            }
            world.play_sound_fine(
                Sound::ItemBottleFill,
                SoundCategory::Players,
                &player.position(),
                1.0,
                1.0,
            );
            item.decrement_unless_creative(player.gamemode.load(), 1);
            player
                .inventory
                .offer_or_drop_stack(water_potion(), player)
                .await;
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{is_water_source, water_potion};
    use pumpkin_data::{
        Block, data_component_impl::PotionContentsImpl, item::Item, potion::Potion,
    };

    #[test]
    fn only_still_water_is_a_bottle_source() {
        assert!(is_water_source(
            &Block::WATER,
            Block::WATER.default_state.id
        ));
        let flowing = Block::WATER
            .from_properties(&[("level", "1")])
            .to_state_id(&Block::WATER);
        assert!(!is_water_source(&Block::WATER, flowing));
        assert!(!is_water_source(&Block::LAVA, Block::LAVA.default_state.id));
    }

    #[test]
    fn filled_bottle_is_explicitly_water() {
        let stack = water_potion();
        assert_eq!(stack.item.id, Item::POTION.id);
        assert_eq!(
            stack
                .get_data_component::<PotionContentsImpl>()
                .and_then(|c| c.potion_id),
            Some(i32::from(Potion::WATER.id))
        );
    }
}
