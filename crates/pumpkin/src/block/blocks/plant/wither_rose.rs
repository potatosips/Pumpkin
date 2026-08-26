use crate::block::{GetStateForNeighborUpdateArgs, blocks::plant::PlantBlockBase};
use pumpkin_data::BlockStateId;
use pumpkin_data::{
    effect::StatusEffect,
    entity::EntityType,
    tag::{self, Taggable},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::Difficulty;

use crate::block::{BlockBehaviour, BlockFuture, CanPlaceAtArgs, OnEntityCollisionArgs};
#[pumpkin_block("minecraft:wither_rose")]
pub struct WitherRoseBlock;

impl BlockBehaviour for WitherRoseBlock {
    fn on_entity_collision<'a>(&'a self, args: OnEntityCollisionArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if let Some(living_entity) = args.entity.get_living_entity() {
                if args.world.level_info.load().difficulty == Difficulty::Peaceful {
                    return;
                }
                let entity_type = args.entity.get_entity().entity_type;
                if entity_type == &EntityType::ENDER_DRAGON
                    || entity_type == &EntityType::WITHER
                    || entity_type == &EntityType::WITHER_SKELETON
                {
                    return;
                }
                let effect = pumpkin_data::potion::Effect {
                    effect_type: &StatusEffect::WITHER,
                    duration: 40,
                    amplifier: 0,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: true,
                };
                if let Some(player) = args.entity.get_player() {
                    player.send_effect(effect.clone()).await;
                }
                living_entity.add_effect(effect).await;
            }
        })
    }
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
    }
    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            <Self as PlantBlockBase>::get_state_for_neighbor_update(
                self,
                args.world,
                args.position,
                args.state_id,
            )
            .await
        })
    }
}

impl PlantBlockBase for WitherRoseBlock {
    fn can_plant_on_top(
        &self,
        block_accessor: &dyn pumpkin_world::world::BlockAccessor,
        pos: &pumpkin_util::math::position::BlockPos,
    ) -> bool {
        let support_block = block_accessor.get_block(pos);
        support_block.has_tag(&tag::Block::MINECRAFT_SUPPORTS_WITHER_ROSE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn wither_rose_block_id_parity() {
        assert_eq!(Block::WITHER_ROSE.name, "wither_rose");
    }

    #[test]
    fn wither_rose_default_state_parity() {
        assert_ne!(
            Block::WITHER_ROSE.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn wither_rose_supports_tag_parity() {
        assert!(Block::SOUL_SAND.has_tag(&tag::Block::MINECRAFT_SUPPORTS_WITHER_ROSE));
        assert!(Block::SOUL_SOIL.has_tag(&tag::Block::MINECRAFT_SUPPORTS_WITHER_ROSE));
        assert!(Block::DIRT.has_tag(&tag::Block::MINECRAFT_SUPPORTS_WITHER_ROSE));
        assert!(Block::NETHERRACK.has_tag(&tag::Block::MINECRAFT_SUPPORTS_WITHER_ROSE));
    }
}
