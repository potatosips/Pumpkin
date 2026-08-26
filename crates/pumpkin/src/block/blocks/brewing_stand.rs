use std::sync::Arc;

use crate::block::{BlockFuture, GetComparatorOutputArgs, PlacedArgs};
use crate::block::{
    registry::BlockActionResult,
    {BlockBehaviour, NormalUseArgs},
};

use crate::block::entities::brewing_stand::BrewingStandBlockEntity;
use pumpkin_data::translation;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{BoxFuture, ScreenHandlerFactory, SharedScreenHandler};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::text::TextComponent;
use pumpkin_world::inventory::Inventory;

struct BrewingScreenFactory(
    Arc<dyn Inventory>,
    Arc<dyn crate::block::entities::PropertyDelegate>,
);

impl ScreenHandlerFactory for BrewingScreenFactory {
    fn create_screen_handler<'a>(
        &'a self,
        sync_id: u8,
        player_inventory: &'a Arc<PlayerInventory>,
        _player: &'a dyn pumpkin_inventory::screen_handler::InventoryPlayer,
    ) -> BoxFuture<'a, Option<SharedScreenHandler>> {
        let inventory = self.0.clone();
        let property_delegate = self.1.clone();
        let pi = player_inventory.clone();
        Box::pin(async move {
            // Delegate to pumpkin-inventory brewing creator
            pumpkin_inventory::brewing::create_brewing(sync_id, pi, inventory, property_delegate)
                .await
                .map(|handler| Arc::new(tokio::sync::Mutex::new(handler)) as SharedScreenHandler)
        })
    }

    fn get_display_name(&self) -> TextComponent {
        pumpkin_macros::translate_cross!(
            translation::java::CONTAINER_BREWING,
            translation::bedrock::CONTAINER_BREWING
        )
    }
}

#[pumpkin_block("minecraft:brewing_stand")]
pub struct BrewingStandBlock;

impl BlockBehaviour for BrewingStandBlock {
    fn normal_use<'a>(&'a self, args: NormalUseArgs<'a>) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async move {
            if let Some(block_entity) = args.world.get_block_entity(args.position)
                && let Some(inventory) = block_entity.clone().get_inventory()
                && let Some(pd) = block_entity.clone().to_property_delegate()
            {
                args.player
                    .increment_stat(
                        pumpkin_data::statistic::StatisticCategory::Custom,
                        pumpkin_data::statistic::CustomStatistic::InteractWithBrewingstand as i32,
                        1,
                    )
                    .await;
                args.player
                    .open_handled_screen(&BrewingScreenFactory(inventory, pd), Some(*args.position))
                    .await;
            }

            BlockActionResult::Success
        })
    }

    fn placed<'a>(&'a self, args: PlacedArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let be = BrewingStandBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(be));
        })
    }

    fn get_comparator_output<'a>(
        &'a self,
        args: GetComparatorOutputArgs<'a>,
    ) -> BlockFuture<'a, Option<u8>> {
        Box::pin(async move {
            if let Some(block_entity) = args.world.get_block_entity(args.position)
                && let Some(inventory) = block_entity.get_inventory()
            {
                let mut bottles = 0u8;
                // Bottle slots are 0, 1, 2 in brewing stands
                for slot in 0..3 {
                    let stack = inventory.get_stack(slot).await;
                    if !stack.is_empty() {
                        bottles += 1;
                    }
                }
                Some(bottles)
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;
    use pumpkin_data::block_properties::{BlockProperties, BrewingStandLikeProperties};

    #[test]
    fn brewing_stand_block_id_parity() {
        assert_eq!(Block::BREWING_STAND.name, "brewing_stand");
    }

    #[test]
    fn brewing_stand_default_state_parity() {
        assert_ne!(
            Block::BREWING_STAND.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn brewing_stand_properties_roundtrip_parity() {
        for has_bottle_0 in [true, false] {
            for has_bottle_1 in [true, false] {
                for has_bottle_2 in [true, false] {
                    let props = BrewingStandLikeProperties {
                        has_bottle_0,
                        has_bottle_1,
                        has_bottle_2,
                    };
                    let state_id = props.to_state_id(&Block::BREWING_STAND);
                    let rt =
                        BrewingStandLikeProperties::from_state_id(state_id, &Block::BREWING_STAND);
                    assert_eq!(rt.has_bottle_0, has_bottle_0);
                    assert_eq!(rt.has_bottle_1, has_bottle_1);
                    assert_eq!(rt.has_bottle_2, has_bottle_2);
                }
            }
        }
    }
}
