use std::pin::Pin;

use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{Block, tag};
use pumpkin_data::{BlockDirection, BlockId};
use pumpkin_util::GameMode;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;

pub struct AxeItem;

impl ItemMetadata for AxeItem {
    fn ids() -> Box<[u16]> {
        tag::Item::MINECRAFT_AXES.1.into()
    }
}

impl ItemBehaviour for AxeItem {
    fn use_on_block<'a>(
        &'a self,
        item: &'a mut ItemStack,
        player: &'a Player,
        location: BlockPos,
        _face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        block: &'a Block,
        _server: &'a Server,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            // I tried to follow mojang order of doing things.
            let world = player.world();
            let replacement_block = try_use_axe(block.id);
            // First we try to strip the block. by getting his equivalent and applying it the axis.

            // If there is a strip equivalent.
            let changed = if let Some((replacement, effect)) = replacement_block {
                world.play_sound(effect.sound(), SoundCategory::Blocks, &location.to_f64());
                if let Some(event) = effect.world_event() {
                    world.sync_world_event(event, location, 0);
                }
                let new_block = replacement.to_block();
                let old_state_id = world.get_block_state_id(&location);
                let new_state_id = state_with_properties_of(block, old_state_id, new_block);
                world
                    .set_block_state(&location, new_state_id, BlockFlags::NOTIFY_ALL)
                    .await;
                true
            } else {
                false
            };

            if changed && player.gamemode.load() != GameMode::Creative {
                let _ = item.damage_item(1);
            }
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AxeEffect {
    Strip,
    Scrape,
    WaxOff,
}

/// Vanilla's `BlockState#setValue`-style replacement copies every property
/// shared by the old and replacement blocks. Unsupported properties are
/// ignored by the generated replacement property parser.
pub(crate) fn state_with_properties_of(
    old_block: &Block,
    old_state_id: pumpkin_data::BlockStateId,
    new_block: &Block,
) -> pumpkin_data::BlockStateId {
    old_block
        .properties(old_state_id)
        .map_or(new_block.default_state.id, |properties| {
            new_block
                .from_properties(&properties.to_props())
                .to_state_id(new_block)
        })
}

impl AxeEffect {
    const fn sound(self) -> Sound {
        match self {
            Self::Strip => Sound::ItemAxeStrip,
            Self::Scrape => Sound::ItemAxeScrape,
            Self::WaxOff => Sound::ItemAxeWaxOff,
        }
    }

    const fn world_event(self) -> Option<WorldEvent> {
        match self {
            Self::Strip => None,
            Self::Scrape => Some(WorldEvent::ParticlesScrape),
            Self::WaxOff => Some(WorldEvent::ParticlesWaxOff),
        }
    }
}

const fn try_use_axe(id: BlockId) -> Option<(BlockId, AxeEffect)> {
    // Trying to get the strip equivalent
    if let Some(block) = get_stripped_equivalent(id) {
        return Some((block, AxeEffect::Strip));
    }
    // Else decrease the level of oxidation
    if let Some(block) = get_deoxidized_equivalent(id) {
        return Some((block, AxeEffect::Scrape));
    }
    // Else unwax the block
    match get_unwaxed_equivalent(id) {
        Some(block) => Some((block, AxeEffect::WaxOff)),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_axe_effect_priority_and_feedback() {
        assert_eq!(
            try_use_axe(BlockId::OAK_LOG),
            Some((BlockId::STRIPPED_OAK_LOG, AxeEffect::Strip))
        );
        assert_eq!(
            try_use_axe(BlockId::WEATHERED_COPPER),
            Some((BlockId::EXPOSED_COPPER, AxeEffect::Scrape))
        );
        assert_eq!(
            try_use_axe(BlockId::WAXED_WEATHERED_COPPER),
            Some((BlockId::WEATHERED_COPPER, AxeEffect::WaxOff))
        );
        assert_eq!(try_use_axe(BlockId::STONE), None);

        assert!(AxeEffect::Strip.world_event().is_none());
        assert_eq!(
            AxeEffect::Scrape.world_event().map(|event| event as u16),
            Some(WorldEvent::ParticlesScrape as u16)
        );
        assert_eq!(
            AxeEffect::WaxOff.world_event().map(|event| event as u16),
            Some(WorldEvent::ParticlesWaxOff as u16)
        );
    }

    #[test]
    fn copper_replacements_preserve_all_shared_block_properties() {
        let pairs = [
            (&Block::COPPER_TRAPDOOR, &Block::WAXED_COPPER_TRAPDOOR),
            (&Block::CUT_COPPER_STAIRS, &Block::WAXED_CUT_COPPER_STAIRS),
            (&Block::CUT_COPPER_SLAB, &Block::WAXED_CUT_COPPER_SLAB),
            (&Block::COPPER_BULB, &Block::WAXED_COPPER_BULB),
        ];

        for (old_block, new_block) in pairs {
            let old_state = old_block.states.last().expect("block has states").id;
            let new_state = state_with_properties_of(old_block, old_state, new_block);
            assert_eq!(
                old_block
                    .properties(old_state)
                    .expect("old properties")
                    .to_props(),
                new_block
                    .properties(new_state)
                    .expect("new properties")
                    .to_props(),
                "{} -> {} lost block state",
                old_block.name,
                new_block.name,
            );
        }
    }
}

const fn get_stripped_equivalent(id: BlockId) -> Option<BlockId> {
    match id {
        BlockId::OAK_LOG => Some(BlockId::STRIPPED_OAK_LOG),
        BlockId::SPRUCE_LOG => Some(BlockId::STRIPPED_SPRUCE_LOG),
        BlockId::BIRCH_LOG => Some(BlockId::STRIPPED_BIRCH_LOG),
        BlockId::JUNGLE_LOG => Some(BlockId::STRIPPED_JUNGLE_LOG),
        BlockId::ACACIA_LOG => Some(BlockId::STRIPPED_ACACIA_LOG),
        BlockId::DARK_OAK_LOG => Some(BlockId::STRIPPED_DARK_OAK_LOG),
        BlockId::MANGROVE_LOG => Some(BlockId::STRIPPED_MANGROVE_LOG),
        BlockId::CHERRY_LOG => Some(BlockId::STRIPPED_CHERRY_LOG),
        BlockId::PALE_OAK_LOG => Some(BlockId::STRIPPED_PALE_OAK_LOG),
        BlockId::OAK_WOOD => Some(BlockId::STRIPPED_OAK_WOOD),
        BlockId::SPRUCE_WOOD => Some(BlockId::STRIPPED_SPRUCE_WOOD),
        BlockId::BIRCH_WOOD => Some(BlockId::STRIPPED_BIRCH_WOOD),
        BlockId::JUNGLE_WOOD => Some(BlockId::STRIPPED_JUNGLE_WOOD),
        BlockId::ACACIA_WOOD => Some(BlockId::STRIPPED_ACACIA_WOOD),
        BlockId::DARK_OAK_WOOD => Some(BlockId::STRIPPED_DARK_OAK_WOOD),
        BlockId::MANGROVE_WOOD => Some(BlockId::STRIPPED_MANGROVE_WOOD),
        BlockId::CHERRY_WOOD => Some(BlockId::STRIPPED_CHERRY_WOOD),
        BlockId::PALE_OAK_WOOD => Some(BlockId::STRIPPED_PALE_OAK_WOOD),
        BlockId::CRIMSON_STEM => Some(BlockId::STRIPPED_CRIMSON_STEM),
        BlockId::WARPED_STEM => Some(BlockId::STRIPPED_WARPED_STEM),
        BlockId::CRIMSON_HYPHAE => Some(BlockId::STRIPPED_CRIMSON_HYPHAE),
        BlockId::WARPED_HYPHAE => Some(BlockId::STRIPPED_WARPED_HYPHAE),
        BlockId::BAMBOO_BLOCK => Some(BlockId::STRIPPED_BAMBOO_BLOCK),
        _ => None,
    }
}

const fn get_deoxidized_equivalent(id: BlockId) -> Option<BlockId> {
    match id {
        BlockId::OXIDIZED_COPPER => Some(BlockId::WEATHERED_COPPER),
        BlockId::WEATHERED_COPPER => Some(BlockId::EXPOSED_COPPER),
        BlockId::EXPOSED_COPPER => Some(BlockId::COPPER_BLOCK),
        BlockId::OXIDIZED_CHISELED_COPPER => Some(BlockId::WEATHERED_CHISELED_COPPER),
        BlockId::WEATHERED_CHISELED_COPPER => Some(BlockId::EXPOSED_CHISELED_COPPER),
        BlockId::EXPOSED_CHISELED_COPPER => Some(BlockId::CHISELED_COPPER),
        BlockId::OXIDIZED_COPPER_GRATE => Some(BlockId::WEATHERED_COPPER_GRATE),
        BlockId::WEATHERED_COPPER_GRATE => Some(BlockId::EXPOSED_COPPER_GRATE),
        BlockId::EXPOSED_COPPER_GRATE => Some(BlockId::COPPER_GRATE),
        BlockId::OXIDIZED_CUT_COPPER => Some(BlockId::WEATHERED_CUT_COPPER),
        BlockId::WEATHERED_CUT_COPPER => Some(BlockId::EXPOSED_CUT_COPPER),
        BlockId::EXPOSED_CUT_COPPER => Some(BlockId::CUT_COPPER),
        BlockId::OXIDIZED_CUT_COPPER_STAIRS => Some(BlockId::WEATHERED_CUT_COPPER_STAIRS),
        BlockId::WEATHERED_CUT_COPPER_STAIRS => Some(BlockId::EXPOSED_CUT_COPPER_STAIRS),
        BlockId::EXPOSED_CUT_COPPER_STAIRS => Some(BlockId::CUT_COPPER_STAIRS),
        BlockId::OXIDIZED_CUT_COPPER_SLAB => Some(BlockId::WEATHERED_CUT_COPPER_SLAB),
        BlockId::WEATHERED_CUT_COPPER_SLAB => Some(BlockId::EXPOSED_CUT_COPPER_SLAB),
        BlockId::EXPOSED_CUT_COPPER_SLAB => Some(BlockId::CUT_COPPER_SLAB),
        BlockId::OXIDIZED_COPPER_BULB => Some(BlockId::WEATHERED_COPPER_BULB),
        BlockId::WEATHERED_COPPER_BULB => Some(BlockId::EXPOSED_COPPER_BULB),
        BlockId::EXPOSED_COPPER_BULB => Some(BlockId::COPPER_BULB),
        BlockId::OXIDIZED_COPPER_DOOR => Some(BlockId::WEATHERED_COPPER_DOOR),
        BlockId::WEATHERED_COPPER_DOOR => Some(BlockId::EXPOSED_COPPER_DOOR),
        BlockId::EXPOSED_COPPER_DOOR => Some(BlockId::COPPER_DOOR),
        BlockId::OXIDIZED_COPPER_TRAPDOOR => Some(BlockId::WEATHERED_COPPER_TRAPDOOR),
        BlockId::WEATHERED_COPPER_TRAPDOOR => Some(BlockId::EXPOSED_COPPER_TRAPDOOR),
        BlockId::EXPOSED_COPPER_TRAPDOOR => Some(BlockId::COPPER_TRAPDOOR),
        _ => None,
    }
}

const fn get_unwaxed_equivalent(id: BlockId) -> Option<BlockId> {
    match id {
        BlockId::WAXED_OXIDIZED_COPPER => Some(BlockId::OXIDIZED_COPPER),
        BlockId::WAXED_WEATHERED_COPPER => Some(BlockId::WEATHERED_COPPER),
        BlockId::WAXED_EXPOSED_COPPER => Some(BlockId::EXPOSED_COPPER),
        BlockId::WAXED_COPPER_BLOCK => Some(BlockId::COPPER_BLOCK),
        BlockId::WAXED_OXIDIZED_CHISELED_COPPER => Some(BlockId::OXIDIZED_CHISELED_COPPER),
        BlockId::WAXED_WEATHERED_CHISELED_COPPER => Some(BlockId::WEATHERED_CHISELED_COPPER),
        BlockId::WAXED_EXPOSED_CHISELED_COPPER => Some(BlockId::EXPOSED_CHISELED_COPPER),
        BlockId::WAXED_CHISELED_COPPER => Some(BlockId::CHISELED_COPPER),
        BlockId::WAXED_COPPER_GRATE => Some(BlockId::COPPER_GRATE),
        BlockId::WAXED_OXIDIZED_COPPER_GRATE => Some(BlockId::OXIDIZED_COPPER_GRATE),
        BlockId::WAXED_WEATHERED_COPPER_GRATE => Some(BlockId::WEATHERED_COPPER_GRATE),
        BlockId::WAXED_EXPOSED_COPPER_GRATE => Some(BlockId::EXPOSED_COPPER_GRATE),
        BlockId::WAXED_OXIDIZED_CUT_COPPER => Some(BlockId::OXIDIZED_CUT_COPPER),
        BlockId::WAXED_WEATHERED_CUT_COPPER => Some(BlockId::WEATHERED_CUT_COPPER),
        BlockId::WAXED_EXPOSED_CUT_COPPER => Some(BlockId::EXPOSED_CUT_COPPER),
        BlockId::WAXED_CUT_COPPER => Some(BlockId::CUT_COPPER),
        BlockId::WAXED_OXIDIZED_CUT_COPPER_STAIRS => Some(BlockId::OXIDIZED_CUT_COPPER_STAIRS),
        BlockId::WAXED_WEATHERED_CUT_COPPER_STAIRS => Some(BlockId::WEATHERED_CUT_COPPER_STAIRS),
        BlockId::WAXED_EXPOSED_CUT_COPPER_STAIRS => Some(BlockId::EXPOSED_CUT_COPPER_STAIRS),
        BlockId::WAXED_CUT_COPPER_STAIRS => Some(BlockId::CUT_COPPER_STAIRS),
        BlockId::WAXED_OXIDIZED_CUT_COPPER_SLAB => Some(BlockId::OXIDIZED_CUT_COPPER_SLAB),
        BlockId::WAXED_WEATHERED_CUT_COPPER_SLAB => Some(BlockId::WEATHERED_CUT_COPPER_SLAB),
        BlockId::WAXED_EXPOSED_CUT_COPPER_SLAB => Some(BlockId::EXPOSED_CUT_COPPER_SLAB),
        BlockId::WAXED_CUT_COPPER_SLAB => Some(BlockId::CUT_COPPER_SLAB),
        BlockId::WAXED_OXIDIZED_COPPER_BULB => Some(BlockId::OXIDIZED_COPPER_BULB),
        BlockId::WAXED_WEATHERED_COPPER_BULB => Some(BlockId::WEATHERED_COPPER_BULB),
        BlockId::WAXED_EXPOSED_COPPER_BULB => Some(BlockId::EXPOSED_COPPER_BULB),
        BlockId::WAXED_COPPER_BULB => Some(BlockId::COPPER_BULB),
        BlockId::WAXED_OXIDIZED_COPPER_DOOR => Some(BlockId::OXIDIZED_COPPER_DOOR),
        BlockId::WAXED_WEATHERED_COPPER_DOOR => Some(BlockId::WEATHERED_COPPER_DOOR),
        BlockId::WAXED_EXPOSED_COPPER_DOOR => Some(BlockId::EXPOSED_COPPER_DOOR),
        BlockId::WAXED_COPPER_DOOR => Some(BlockId::COPPER_DOOR),
        BlockId::WAXED_OXIDIZED_COPPER_TRAPDOOR => Some(BlockId::OXIDIZED_COPPER_TRAPDOOR),
        BlockId::WAXED_WEATHERED_COPPER_TRAPDOOR => Some(BlockId::WEATHERED_COPPER_TRAPDOOR),
        BlockId::WAXED_EXPOSED_COPPER_TRAPDOOR => Some(BlockId::EXPOSED_COPPER_TRAPDOOR),
        BlockId::WAXED_COPPER_TRAPDOOR => Some(BlockId::COPPER_TRAPDOOR),
        _ => None,
    }
}
