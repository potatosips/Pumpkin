use crate::block::blocks::fire::FireBlockBase;
use crate::block::blocks::fire::fire::FireBlock;
use crate::world::World;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockStateId, tag};
use pumpkin_util::math::position::BlockPos;
use std::sync::Arc;

pub struct Ignition;

impl Ignition {
    /// Lights `block` at `location` itself if it can be lit (campfires, candles, candle
    /// cakes), otherwise places a fire block at `fire_pos`.
    pub async fn ignite_block<F, Fut>(
        ignite_logic: F,
        world: &Arc<World>,
        location: BlockPos,
        fire_pos: BlockPos,
        block: &Block,
    ) -> bool
    where
        F: FnOnce(Arc<World>, BlockPos, BlockStateId) -> Fut,
        Fut: Future<Output = ()>,
    {
        if world.get_fluid(&location).name != Fluid::EMPTY.name {
            return false;
        }
        let fire_block = FireBlockBase::get_fire_type(world, &fire_pos);

        let state_id = world.get_block_state_id(&location);

        if let Some(new_state_id) = can_be_lit(block, state_id) {
            ignite_logic(world.clone(), location, new_state_id).await;
            return true;
        }

        let state_id = FireBlock.get_state_for_position(world, &fire_block, &fire_pos);
        if FireBlockBase::can_place_at(world, &fire_pos) {
            ignite_logic(world.clone(), fire_pos, state_id).await;
            return true;
        }

        false
    }
}

pub(crate) fn can_be_lit(block: &Block, state_id: BlockStateId) -> Option<BlockStateId> {
    // Vanilla only lights the clicked block itself for campfires, candles and candle cakes.
    // See `CampfireBlock::canLight`, `CandleBlock::canLight` and `CandleCakeBlock::canLight`.
    // Everything else that merely carries a `lit` property (furnaces, redstone lamps, copper
    // bulbs, ...) must fall through to placing a fire block instead.
    if !block.has_tag(&tag::Block::MINECRAFT_CAMPFIRES)
        && !block.has_tag(&tag::Block::MINECRAFT_CANDLES)
        && !block.has_tag(&tag::Block::MINECRAFT_CANDLE_CAKES)
    {
        return None;
    }

    let mut props = {
        let props = &block.properties(state_id)?;
        props.to_props()
    };

    if props
        .iter()
        .any(|(key, value)| *key == "waterlogged" && *value == "true")
    {
        return None;
    }

    let (_, value) = props.iter_mut().find(|(k, _)| *k == "lit")?;
    *value = "true";

    let new_state_id = block.from_properties(&props).to_state_id(block);

    (new_state_id != state_id).then_some(new_state_id)
}

pub(crate) fn can_be_extinguished(block: &Block, state_id: BlockStateId) -> Option<BlockStateId> {
    if !block.has_tag(&tag::Block::MINECRAFT_CAMPFIRES)
        && !block.has_tag(&tag::Block::MINECRAFT_CANDLES)
        && !block.has_tag(&tag::Block::MINECRAFT_CANDLE_CAKES)
    {
        return None;
    }

    let mut props = {
        let props = &block.properties(state_id)?;
        props.to_props()
    };

    let (_, value) = props.iter_mut().find(|(k, _)| *k == "lit")?;
    if *value == "false" {
        return None;
    }
    *value = "false";

    let new_state_id = block.from_properties(&props).to_state_id(block);
    (new_state_id != state_id).then_some(new_state_id)
}

#[cfg(test)]
mod tests {
    use super::{can_be_extinguished, can_be_lit};
    use pumpkin_data::{
        Block,
        block_properties::{BlockProperties, CampfireLikeProperties},
    };

    #[test]
    fn vanilla_projectile_lighting_rejects_lit_and_waterlogged_campfires() {
        let block = &Block::CAMPFIRE;
        let mut props = CampfireLikeProperties::from_state_id(block.default_state.id, block);

        props.lit = false;
        props.waterlogged = false;
        let unlit = props.to_state_id(block);
        let lit = can_be_lit(block, unlit).expect("dry unlit campfire should light");
        assert!(CampfireLikeProperties::from_state_id(lit, block).lit);
        assert!(can_be_lit(block, lit).is_none());

        props.waterlogged = true;
        assert!(can_be_lit(block, props.to_state_id(block)).is_none());
    }

    #[test]
    fn vanilla_extinguishing_toggles_lit_campfires_and_rejects_unlit() {
        let block = &Block::CAMPFIRE;
        let mut props = CampfireLikeProperties::from_state_id(block.default_state.id, block);

        props.lit = true;
        props.waterlogged = false;
        let lit = props.to_state_id(block);
        let unlit = can_be_extinguished(block, lit).expect("lit campfire should extinguish");
        assert!(!CampfireLikeProperties::from_state_id(unlit, block).lit);
        assert!(can_be_extinguished(block, unlit).is_none());
    }
}
