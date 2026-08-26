use std::sync::Arc;

use crate::block::entities::ender_chest::EnderChestBlockEntity;
use crate::block::{
    BlockBehaviour, BlockFuture, NormalUseArgs, OnPlaceArgs, OnSyncedBlockEventArgs, PlacedArgs,
    registry::BlockActionResult,
};
use crate::world::World;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{BlockProperties, LadderLikeProperties};
use pumpkin_data::translation;
use pumpkin_inventory::{
    generic_container_screen_handler::create_generic_9x3,
    player::ender_chest_inventory::EnderChestInventory,
    player::player_inventory::PlayerInventory,
    screen_handler::{BoxFuture, InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::text::TextComponent;
use pumpkin_world::block::viewer::ViewerCountTracker;
use tokio::sync::Mutex;

pub struct EnderChestScreenFactory {
    pub inventory: Arc<EnderChestInventory>,
    pub tracker: Option<Arc<ViewerCountTracker>>,
}

impl ScreenHandlerFactory for EnderChestScreenFactory {
    fn create_screen_handler<'a>(
        &'a self,
        sync_id: u8,
        player_inventory: &'a Arc<PlayerInventory>,
        _player: &'a dyn InventoryPlayer,
    ) -> BoxFuture<'a, Option<SharedScreenHandler>> {
        Box::pin(async move {
            if let Some(tracker) = &self.tracker {
                self.inventory.set_tracker(tracker.clone()).await;
            }
            let handler =
                create_generic_9x3(sync_id, player_inventory, self.inventory.clone()).await;
            let concrete_arc = Arc::new(Mutex::new(handler));

            Some(concrete_arc as SharedScreenHandler)
        })
    }

    fn get_display_name(&self) -> TextComponent {
        pumpkin_macros::translate_cross!(
            translation::java::CONTAINER_ENDERCHEST,
            translation::bedrock::CONTAINER_ENDERCHEST
        )
    }
}

#[pumpkin_block("minecraft:ender_chest")]
pub struct EnderChestBlock;

impl BlockBehaviour for EnderChestBlock {
    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let mut props = LadderLikeProperties::default(args.block);
            props.facing = args
                .player
                .living_entity
                .entity
                .get_horizontal_facing()
                .opposite();
            props.waterlogged = args.replacing.water_source();
            props.to_state_id(args.block)
        })
    }

    fn on_synced_block_event<'a>(
        &'a self,
        args: OnSyncedBlockEventArgs<'a>,
    ) -> BlockFuture<'a, bool> {
        Box::pin(async move {
            // On the server, we don't need to do more because the client is responsible for that.
            args.r#type == Self::LID_ANIMATION_EVENT_TYPE
        })
    }

    fn normal_use<'a>(&'a self, args: NormalUseArgs<'a>) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async move {
            if is_chest_blocked(args.world, args.position) {
                return BlockActionResult::Success;
            }

            let block_entity = if let Some(be) = args.world.get_block_entity(args.position) {
                be
            } else {
                let be = Arc::new(EnderChestBlockEntity::new(*args.position));
                args.world.add_block_entity(be.clone());
                be
            };

            if let Some(block_entity) = block_entity
                .as_any()
                .downcast_ref::<EnderChestBlockEntity>()
            {
                let inventory = args.player.ender_chest_inventory();
                args.player
                    .increment_stat(
                        pumpkin_data::statistic::StatisticCategory::Custom,
                        pumpkin_data::statistic::CustomStatistic::OpenEnderchest as i32,
                        1,
                    )
                    .await;
                args.player
                    .open_handled_screen(
                        &EnderChestScreenFactory {
                            inventory: inventory.clone(),
                            tracker: Some(block_entity.get_tracker()),
                        },
                        Some(*args.position),
                    )
                    .await;
                // TODO: PiglinBrain.onGuardedBlockInteracted(serverWorld, player, true);
            }

            BlockActionResult::Success
        })
    }

    fn placed<'a>(&'a self, args: PlacedArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let block_entity = EnderChestBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(block_entity));
        })
    }
}

fn is_chest_blocked(world: &World, block_pos: &BlockPos) -> bool {
    has_block_on_top(world, block_pos) || has_cat_on_top(world, block_pos)
}

fn has_block_on_top(world: &World, block_pos: &BlockPos) -> bool {
    let above_pos = block_pos.up();
    let above_state = world.get_block_state(&above_pos);
    above_state.is_solid_block()
}

fn has_cat_on_top(world: &World, block_pos: &BlockPos) -> bool {
    let above_pos = block_pos.up();
    let aabb = pumpkin_util::math::boundingbox::BoundingBox::from_block(&above_pos);
    let entities = world.get_entities_at_box(&aabb);
    for entity in entities {
        let entity_type = entity.get_entity().entity_type;
        if entity_type == &pumpkin_data::entity::EntityType::CAT
            || entity_type == &pumpkin_data::entity::EntityType::OCELOT
        {
            if let Some(mob) = entity.get_mob() {
                if mob.is_sitting() {
                    return true;
                }
            }
        }
    }
    false
}
impl EnderChestBlock {
    pub const LID_ANIMATION_EVENT_TYPE: u8 = 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;
    use pumpkin_data::block_properties::{BlockProperties, HorizontalFacing, LadderLikeProperties};

    #[test]
    fn ender_chest_block_id_parity() {
        assert_eq!(Block::ENDER_CHEST.name, "ender_chest");
    }

    #[test]
    fn ender_chest_default_state_parity() {
        assert_ne!(
            Block::ENDER_CHEST.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn ender_chest_properties_roundtrip_parity() {
        for facing in [
            HorizontalFacing::North,
            HorizontalFacing::South,
            HorizontalFacing::East,
            HorizontalFacing::West,
        ] {
            for waterlogged in [true, false] {
                let props = LadderLikeProperties {
                    facing,
                    waterlogged,
                };
                let state_id = props.to_state_id(&Block::ENDER_CHEST);
                let rt = LadderLikeProperties::from_state_id(state_id, &Block::ENDER_CHEST);
                assert_eq!(rt.facing, facing);
                assert_eq!(rt.waterlogged, waterlogged);
            }
        }
    }
}
