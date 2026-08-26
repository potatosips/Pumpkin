use crate::block::blocks::redstone::block_receives_redstone_power;
use crate::block::{BlockBehaviour, BlockFuture, BlockMetadata, OnNeighborUpdateArgs, OnPlaceArgs};
use pumpkin_data::BlockId;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::BlockProperties;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_world::world::BlockFlags;

type CopperBulbLikeProperties = pumpkin_data::block_properties::CopperBulbLikeProperties;

pub struct CopperBulbBlock;

impl BlockMetadata for CopperBulbBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::COPPER_BULB,
            BlockId::EXPOSED_COPPER_BULB,
            BlockId::WEATHERED_COPPER_BULB,
            BlockId::OXIDIZED_COPPER_BULB,
            BlockId::WAXED_COPPER_BULB,
            BlockId::WAXED_EXPOSED_COPPER_BULB,
            BlockId::WAXED_WEATHERED_COPPER_BULB,
            BlockId::WAXED_OXIDIZED_COPPER_BULB,
        ]
        .into()
    }
}

impl BlockBehaviour for CopperBulbBlock {
    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            let mut props = CopperBulbLikeProperties::default(args.block);
            let is_receiving_power = block_receives_redstone_power(args.world, args.position).await;
            if is_receiving_power {
                props.lit = true;
                args.world.play_block_sound(
                    Sound::BlockCopperBulbTurnOn,
                    SoundCategory::Blocks,
                    *args.position,
                );
                props.powered = true;
            }
            props.to_state_id(args.block)
        })
    }

    fn on_neighbor_update<'a>(&'a self, args: OnNeighborUpdateArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let state = args.world.get_block_state(args.position);
            let mut props = CopperBulbLikeProperties::from_state_id(state.id, args.block);
            let is_receiving_power = block_receives_redstone_power(args.world, args.position).await;
            if props.powered != is_receiving_power {
                if !props.powered {
                    props.lit = !props.lit;
                    args.world.play_block_sound(
                        if props.lit {
                            Sound::BlockCopperBulbTurnOn
                        } else {
                            Sound::BlockCopperBulbTurnOff
                        },
                        SoundCategory::Blocks,
                        *args.position,
                    );
                }
                props.powered = is_receiving_power;
                args.world
                    .set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;
    use pumpkin_data::block_properties::{BlockProperties, CopperBulbLikeProperties};

    #[test]
    fn copper_bulb_ids_parity() {
        assert_eq!(Block::COPPER_BULB.name, "copper_bulb");
        assert_eq!(Block::EXPOSED_COPPER_BULB.name, "exposed_copper_bulb");
        assert_eq!(Block::WEATHERED_COPPER_BULB.name, "weathered_copper_bulb");
        assert_eq!(Block::OXIDIZED_COPPER_BULB.name, "oxidized_copper_bulb");
        assert_eq!(Block::WAXED_COPPER_BULB.name, "waxed_copper_bulb");
        assert_eq!(
            Block::WAXED_EXPOSED_COPPER_BULB.name,
            "waxed_exposed_copper_bulb"
        );
        assert_eq!(
            Block::WAXED_WEATHERED_COPPER_BULB.name,
            "waxed_weathered_copper_bulb"
        );
        assert_eq!(
            Block::WAXED_OXIDIZED_COPPER_BULB.name,
            "waxed_oxidized_copper_bulb"
        );
    }

    #[test]
    fn copper_bulb_default_state_parity() {
        assert_ne!(
            Block::COPPER_BULB.default_state.id,
            Block::AIR.default_state.id
        );
        assert_ne!(
            Block::OXIDIZED_COPPER_BULB.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn copper_bulb_properties_roundtrip_parity() {
        for lit in [true, false] {
            for powered in [true, false] {
                let props = CopperBulbLikeProperties { lit, powered };
                let state_id = props.to_state_id(&Block::COPPER_BULB);
                let rt = CopperBulbLikeProperties::from_state_id(state_id, &Block::COPPER_BULB);
                assert_eq!(rt.lit, lit);
                assert_eq!(rt.powered, powered);
            }
        }
    }
}
