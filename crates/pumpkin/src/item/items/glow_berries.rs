use std::pin::Pin;

use pumpkin_data::{
    Block, BlockDirection,
    item::Item,
    item_stack::ItemStack,
    sound::{Sound, SoundCategory},
};
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};
use pumpkin_world::world::BlockFlags;

use crate::{
    entity::player::Player,
    item::{ItemBehaviour, ItemMetadata},
    server::Server,
};

pub struct GlowBerriesItem;

impl ItemMetadata for GlowBerriesItem {
    fn ids() -> Box<[u16]> {
        [Item::GLOW_BERRIES.id].into()
    }
}

impl ItemBehaviour for GlowBerriesItem {
    fn use_on_block<'a>(
        &'a self,
        stack: &'a mut ItemStack,
        player: &'a Player,
        location: BlockPos,
        face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        _block: &'a Block,
        server: &'a Server,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if face != BlockDirection::Down {
                return;
            }
            let world = player.world();
            let destination = location.down();
            let old_state_id = world.get_block_state_id(&destination);
            let support = world.get_block(&location);
            if !world.is_in_height_limit(destination.0.y)
                || !world.is_loaded(&destination)
                || !world.get_block_state(&destination).replaceable()
                || (!(support == &Block::CAVE_VINES || support == &Block::CAVE_VINES_PLANT)
                    && !world
                        .get_block_state(&location)
                        .is_side_solid(BlockDirection::Down))
            {
                return;
            }
            world
                .set_block_state(
                    &destination,
                    Block::CAVE_VINES.default_state.id,
                    BlockFlags::NOTIFY_ALL,
                )
                .await;
            server
                .block_registry
                .on_placed(
                    &world,
                    &Block::CAVE_VINES,
                    Block::CAVE_VINES.default_state.id,
                    &destination,
                    old_state_id,
                    true,
                )
                .await;
            world.play_sound_fine(
                Sound::BlockCaveVinesPlace,
                SoundCategory::Blocks,
                &destination.to_f64(),
                1.0,
                1.0,
            );
            stack.decrement_unless_creative(player.gamemode.load(), 1);
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glow_berries_item_and_cave_vines_are_distinct_registry_entries() {
        assert_eq!(Item::GLOW_BERRIES.registry_key, "glow_berries");
        assert_eq!(Block::CAVE_VINES.name, "cave_vines");
        assert_ne!(
            Block::CAVE_VINES.default_state.id,
            Block::AIR.default_state.id
        );
    }
}
