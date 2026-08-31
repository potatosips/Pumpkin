use crate::block::entities::skull::SkullBlockEntity;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockFuture, GetStateForNeighborUpdateArgs, NormalUseArgs, OnNeighborUpdateArgs, OnPlaceArgs,
    UseWithItemArgs,
};
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{Axis, NoteblockInstrument};
use pumpkin_data::data_component_impl::{IdOr, SoundEvent};
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{
    Block,
    block_properties::{BlockProperties, NoteBlockLikeProperties},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;

use crate::{
    block::{BlockBehaviour, OnSyncedBlockEventArgs},
    world::World,
};

use super::redstone::block_receives_redstone_power;

#[pumpkin_block("minecraft:note_block")]
pub struct NoteBlock;

impl NoteBlock {
    pub async fn play_note(props: &NoteBlockLikeProperties, world: &World, pos: &BlockPos) {
        if !is_base_block(props.instrument) || world.get_block_state(&pos.up()).is_air() {
            let mut event = crate::plugin::api::events::block::note_play::NotePlayEvent::new(
                *pos,
                format!("{:?}", props.instrument),
                props.note,
            );
            if let Some(server) = world.server.upgrade() {
                server.plugin_manager.fire(&server, &mut event).await;
            }
            if event.cancelled {
                return;
            }
            world.add_synced_block_event(*pos, 0, 0).await;
        }
    }
    fn get_note_pitch(note: u16) -> f32 {
        ((f32::from(note) - 12.0) / 12.0).exp2()
    }

    fn get_state_with_instrument(
        world: &World,
        pos: &BlockPos,
        state: BlockStateId,
        block: &Block,
    ) -> BlockStateId {
        let upper_instrument = world.get_block_state(&pos.up()).instrument;

        let mut note_props = NoteBlockLikeProperties::from_state_id(state, block);
        if !is_base_block(upper_instrument) {
            note_props.instrument = upper_instrument;
            return note_props.to_state_id(block);
        }
        let below_instrument = world.get_block_state(&pos.down()).instrument;
        let below_instrument = if is_base_block(below_instrument) {
            below_instrument
        } else {
            NoteblockInstrument::Harp
        };
        note_props.instrument = below_instrument;
        note_props.to_state_id(block)
    }
}

impl BlockBehaviour for NoteBlock {
    fn on_neighbor_update<'a>(&'a self, args: OnNeighborUpdateArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let block_state = args.world.get_block_state(args.position);
            let mut note_props = NoteBlockLikeProperties::from_state_id(block_state.id, args.block);
            let powered = block_receives_redstone_power(args.world, args.position).await;
            // check if powered state changed
            if note_props.powered != powered {
                if powered {
                    Self::play_note(&note_props, args.world, args.position).await;
                }
                note_props.powered = powered;
                args.world
                    .set_block_state(
                        args.position,
                        note_props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    )
                    .await;
            }
        })
    }

    fn normal_use<'a>(&'a self, args: NormalUseArgs<'a>) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async move {
            let block_state = args.world.get_block_state(args.position);
            let mut note_props = NoteBlockLikeProperties::from_state_id(block_state.id, args.block);
            note_props.note = (note_props.note + 1) % 25;
            args.world
                .set_block_state(
                    args.position,
                    note_props.to_state_id(args.block),
                    BlockFlags::NOTIFY_ALL,
                )
                .await;
            Self::play_note(&note_props, args.world, args.position).await;

            args.player
                .increment_stat(
                    pumpkin_data::statistic::StatisticCategory::Custom,
                    pumpkin_data::statistic::CustomStatistic::TuneNoteblock as i32,
                    1,
                )
                .await;

            BlockActionResult::Success
        })
    }

    fn use_with_item<'a>(
        &'a self,
        _args: UseWithItemArgs<'a>,
    ) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async move {
            // TODO
            BlockActionResult::PassToDefaultBlockAction
        })
    }

    fn on_synced_block_event<'a>(
        &'a self,
        args: OnSyncedBlockEventArgs<'a>,
    ) -> BlockFuture<'a, bool> {
        Box::pin(async move {
            let block_state = args.world.get_block_state(args.position);
            let note_props = NoteBlockLikeProperties::from_state_id(block_state.id, args.block);
            let instrument = note_props.instrument;
            let pitch = if is_base_block(instrument) {
                // checks if can be pitched
                Self::get_note_pitch(u16::from(note_props.note))
            } else {
                1.0 // default pitch
            };
            if instrument == NoteblockInstrument::CustomHead {
                let Some(block_entity) = args.world.get_block_entity(&args.position.up()) else {
                    return false;
                };
                let Some(skull) = block_entity.as_any().downcast_ref::<SkullBlockEntity>() else {
                    return false;
                };
                let Some(sound_name) = skull.note_block_sound.lock().await.clone() else {
                    return false;
                };
                args.world.play_sound_event_fine(
                    &IdOr::Value(SoundEvent::new(sound_name, None)),
                    SoundCategory::Records,
                    &args.position.to_f64(),
                    3.0,
                    1.0,
                );
            } else {
                args.world.play_sound_raw(
                    convert_instrument_to_sound(instrument) as u16,
                    SoundCategory::Records,
                    &args.position.to_f64(),
                    3.0,
                    pitch,
                );
            }
            args.world.spawn_particle(
                Vector3::new(
                    f64::from(args.position.0.x) + 0.5,
                    f64::from(args.position.0.y) + 1.2,
                    f64::from(args.position.0.z) + 0.5,
                ),
                Vector3::new(f32::from(note_props.note) / 24.0, 0.0, 0.0),
                1.0,
                0,
                Particle::Note,
            );
            true
        })
    }

    fn on_place<'a>(&'a self, args: OnPlaceArgs<'a>) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            Self::get_state_with_instrument(
                args.world,
                args.position,
                Block::NOTE_BLOCK.default_state.id,
                args.block,
            )
        })
    }

    fn get_state_for_neighbor_update<'a>(
        &'a self,
        args: GetStateForNeighborUpdateArgs<'a>,
    ) -> BlockFuture<'a, BlockStateId> {
        Box::pin(async move {
            if args.direction.to_axis() == Axis::Y {
                return Self::get_state_with_instrument(
                    args.world,
                    args.position,
                    args.state_id,
                    args.block,
                );
            }
            args.state_id
        })
    }
}

const fn convert_instrument_to_sound(instrument: NoteblockInstrument) -> Sound {
    match instrument {
        NoteblockInstrument::Harp => Sound::BlockNoteBlockHarp,
        NoteblockInstrument::Basedrum => Sound::BlockNoteBlockBasedrum,
        NoteblockInstrument::Snare => Sound::BlockNoteBlockSnare,
        NoteblockInstrument::Hat => Sound::BlockNoteBlockHat,
        NoteblockInstrument::Bass => Sound::BlockNoteBlockBass,
        NoteblockInstrument::Flute => Sound::BlockNoteBlockFlute,
        NoteblockInstrument::Bell => Sound::BlockNoteBlockBell,
        NoteblockInstrument::Guitar => Sound::BlockNoteBlockGuitar,
        NoteblockInstrument::Chime => Sound::BlockNoteBlockChime,
        NoteblockInstrument::Xylophone => Sound::BlockNoteBlockXylophone,
        NoteblockInstrument::IronXylophone => Sound::BlockNoteBlockIronXylophone,
        NoteblockInstrument::CowBell => Sound::BlockNoteBlockCowBell,
        NoteblockInstrument::Didgeridoo => Sound::BlockNoteBlockDidgeridoo,
        NoteblockInstrument::Bit => Sound::BlockNoteBlockBit,
        NoteblockInstrument::Banjo => Sound::BlockNoteBlockBanjo,
        NoteblockInstrument::Pling => Sound::BlockNoteBlockPling,
        NoteblockInstrument::Zombie => Sound::BlockNoteBlockImitateZombie,
        NoteblockInstrument::Skeleton => Sound::BlockNoteBlockImitateSkeleton,
        NoteblockInstrument::Creeper => Sound::BlockNoteBlockImitateCreeper,
        NoteblockInstrument::Dragon => Sound::BlockNoteBlockImitateEnderDragon,
        NoteblockInstrument::WitherSkeleton => Sound::BlockNoteBlockImitateWitherSkeleton,
        NoteblockInstrument::Piglin => Sound::BlockNoteBlockImitatePiglin,
        NoteblockInstrument::CustomHead => Sound::UiButtonClick,
        NoteblockInstrument::Trumpet => Sound::BlockNoteBlockTrumpet,
        NoteblockInstrument::TrumpetExposed => Sound::BlockNoteBlockTrumpetExposed,
        NoteblockInstrument::TrumpetOxidized => Sound::BlockNoteBlockTrumpetOxidized,
        NoteblockInstrument::TrumpetWeathered => Sound::BlockNoteBlockTrumpetWeathered,
    }
}

const fn is_base_block(instrument: NoteblockInstrument) -> bool {
    matches!(
        instrument,
        NoteblockInstrument::Harp
            | NoteblockInstrument::Basedrum
            | NoteblockInstrument::Snare
            | NoteblockInstrument::Hat
            | NoteblockInstrument::Bass
            | NoteblockInstrument::Flute
            | NoteblockInstrument::Bell
            | NoteblockInstrument::Guitar
            | NoteblockInstrument::Chime
            | NoteblockInstrument::Xylophone
            | NoteblockInstrument::IronXylophone
            | NoteblockInstrument::CowBell
            | NoteblockInstrument::Didgeridoo
            | NoteblockInstrument::Bit
            | NoteblockInstrument::Banjo
            | NoteblockInstrument::Pling
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;
    use pumpkin_data::block_properties::{
        BlockProperties, NoteBlockLikeProperties, NoteblockInstrument,
    };

    #[test]
    fn note_block_id_parity() {
        assert_eq!(Block::NOTE_BLOCK.name, "note_block");
    }

    #[test]
    fn note_block_default_state_parity() {
        assert_ne!(
            Block::NOTE_BLOCK.default_state.id,
            Block::AIR.default_state.id
        );
    }

    #[test]
    fn note_block_properties_roundtrip_parity() {
        for instrument in [
            NoteblockInstrument::Harp,
            NoteblockInstrument::Bass,
            NoteblockInstrument::Snare,
            NoteblockInstrument::Hat,
            NoteblockInstrument::Basedrum,
            NoteblockInstrument::Bell,
            NoteblockInstrument::Flute,
            NoteblockInstrument::Chime,
            NoteblockInstrument::Guitar,
            NoteblockInstrument::Xylophone,
            NoteblockInstrument::IronXylophone,
            NoteblockInstrument::CowBell,
            NoteblockInstrument::Didgeridoo,
            NoteblockInstrument::Bit,
            NoteblockInstrument::Banjo,
            NoteblockInstrument::Pling,
        ] {
            for note in [0, 12, 24] {
                for powered in [true, false] {
                    let props = NoteBlockLikeProperties {
                        instrument,
                        note,
                        powered,
                    };
                    let state_id = props.to_state_id(&Block::NOTE_BLOCK);
                    let rt = NoteBlockLikeProperties::from_state_id(state_id, &Block::NOTE_BLOCK);
                    assert_eq!(rt.instrument, instrument);
                    assert_eq!(rt.note, note);
                    assert_eq!(rt.powered, powered);
                }
            }
        }
    }

    #[test]
    fn pling_is_a_pitched_base_instrument() {
        assert!(is_base_block(NoteblockInstrument::Pling));
        assert_eq!(NoteBlock::get_note_pitch(12), 1.0);
        assert_eq!(NoteBlock::get_note_pitch(24), 2.0);
    }
}
