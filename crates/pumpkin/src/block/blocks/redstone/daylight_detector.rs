use std::sync::Arc;

use crate::block::entities::daylight_detector::DaylightDetectorBlockEntity;
use pumpkin_data::{Block, block_properties::BlockProperties};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;

use crate::block::{
    BlockActionResult, BlockBehaviour, BlockFuture, BrokenArgs, EmitsRedstonePowerArgs,
    GetRedstonePowerArgs, NormalUseArgs, PlacedArgs,
};
use crate::world::World;

type DaylightDetectorProperties = pumpkin_data::block_properties::DaylightDetectorLikeProperties;

#[pumpkin_block("minecraft:daylight_detector")]
pub struct DaylightDetectorBlock;

impl BlockBehaviour for DaylightDetectorBlock {
    fn placed<'a>(&'a self, args: PlacedArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            args.world
                .add_block_entity(Arc::new(DaylightDetectorBlockEntity::new(*args.position)));
        })
    }

    fn broken<'a>(&'a self, args: BrokenArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            args.world.remove_block_entity(args.position);
        })
    }

    fn normal_use<'a>(&'a self, args: NormalUseArgs<'a>) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async {
            let player_abilities = args.player.abilities.lock();
            if !player_abilities.await.allow_modify_world {
                return BlockActionResult::Pass;
            }

            let state = args.world.get_block_state(args.position);
            let props = DaylightDetectorProperties::from_state_id(state.id, args.block);

            self.update_inverted(props, args.world, args.position, args.block)
                .await;

            DaylightDetectorBlockEntity::update_power(args.world, args.position).await;

            BlockActionResult::Success
        })
    }

    fn get_weak_redstone_power<'a>(
        &'a self,
        args: GetRedstonePowerArgs<'a>,
    ) -> BlockFuture<'a, u8> {
        Box::pin(async move {
            let props = DaylightDetectorProperties::from_state_id(args.state.id, args.block);

            props.power
        })
    }

    fn emits_redstone_power<'a>(
        &'a self,
        _args: EmitsRedstonePowerArgs<'a>,
    ) -> BlockFuture<'a, bool> {
        Box::pin(async move { true })
    }
}

impl DaylightDetectorBlock {
    async fn update_inverted(
        &self,
        props: DaylightDetectorProperties,
        world: &Arc<World>,
        block_pos: &BlockPos,
        block: &Block,
    ) {
        let mut props = props;
        props.inverted = !props.inverted;

        let state = props.to_state_id(block);

        world
            .set_block_state(block_pos, state, BlockFlags::NOTIFY_LISTENERS)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;
    use pumpkin_data::block_properties::{BlockProperties, DaylightDetectorLikeProperties};

    #[test]
    fn daylight_detector_block_id_parity() {
        assert_eq!(Block::DAYLIGHT_DETECTOR.name, "daylight_detector");
    }

    #[test]
    fn daylight_detector_default_state_parity() {
        assert_ne!(
            Block::DAYLIGHT_DETECTOR.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn daylight_detector_properties_roundtrip_parity() {
        for inverted in [true, false] {
            for power in 0..=15 {
                let props = DaylightDetectorLikeProperties { inverted, power };
                let state_id = props.to_state_id(&Block::DAYLIGHT_DETECTOR);
                let rt = DaylightDetectorLikeProperties::from_state_id(
                    state_id,
                    &Block::DAYLIGHT_DETECTOR,
                );
                assert_eq!(rt.inverted, inverted);
                assert_eq!(rt.power, power);
            }
        }
    }
}
