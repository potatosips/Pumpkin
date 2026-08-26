use std::any::Any;
use std::future::Future;
use std::pin::Pin;

use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::data_component_impl::InstrumentImpl;
use pumpkin_data::item::Item;
use pumpkin_data::sound::{Sound, SoundCategory};

pub struct GoatHornItem;

impl ItemMetadata for GoatHornItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::GOAT_HORN.id])
    }
}

impl ItemBehaviour for GoatHornItem {
    fn normal_use<'a>(
        &'a self,
        _item: &'a Item,
        player: &'a Player,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let inventory = player.inventory();
            let main_hand = inventory.held_item().await;
            let (stack, hand) = if main_hand.item.id == Item::GOAT_HORN.id {
                (main_hand, pumpkin_util::Hand::Right)
            } else {
                (inventory.off_hand_item().await, pumpkin_util::Hand::Left)
            };
            let instrument = stack
                .get_data_component::<InstrumentImpl>()
                .map_or("ponder_goat_horn", |component| {
                    component.instrument.as_ref()
                });
            let properties = instrument_properties(instrument);

            player.world().play_sound_fine(
                properties.sound,
                SoundCategory::Records,
                &player.position(),
                properties.range / 16.0,
                1.0,
            );
            player
                .living_entity
                .set_active_hand(hand, stack, properties.use_duration_ticks)
                .await;
            player
                .start_cooldown(
                    Item::GOAT_HORN.registry_key.to_string(),
                    properties.use_duration_ticks,
                )
                .await;
        })
    }

    fn get_use_duration(&self) -> i32 {
        Self::USE_DURATION
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl GoatHornItem {
    pub const USE_DURATION: i32 = 140;
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct InstrumentProperties {
    sound: Sound,
    use_duration_ticks: i32,
    range: f32,
}

fn instrument_properties(instrument: &str) -> InstrumentProperties {
    let sound = match instrument.strip_prefix("minecraft:").unwrap_or(instrument) {
        "sing_goat_horn" => Sound::ItemGoatHornSound1,
        "seek_goat_horn" => Sound::ItemGoatHornSound2,
        "feel_goat_horn" => Sound::ItemGoatHornSound3,
        "admire_goat_horn" => Sound::ItemGoatHornSound4,
        "call_goat_horn" => Sound::ItemGoatHornSound5,
        "yearn_goat_horn" => Sound::ItemGoatHornSound6,
        "dream_goat_horn" => Sound::ItemGoatHornSound7,
        _ => Sound::ItemGoatHornSound0,
    };
    InstrumentProperties {
        sound,
        use_duration_ticks: GoatHornItem::USE_DURATION,
        range: 256.0,
    }
}

#[cfg(test)]
mod tests {
    use super::{GoatHornItem, instrument_properties};
    use pumpkin_data::sound::Sound;

    #[test]
    fn vanilla_goat_horn_instruments_select_their_sound() {
        let expected = [
            ("ponder_goat_horn", Sound::ItemGoatHornSound0),
            ("sing_goat_horn", Sound::ItemGoatHornSound1),
            ("seek_goat_horn", Sound::ItemGoatHornSound2),
            ("feel_goat_horn", Sound::ItemGoatHornSound3),
            ("admire_goat_horn", Sound::ItemGoatHornSound4),
            ("call_goat_horn", Sound::ItemGoatHornSound5),
            ("yearn_goat_horn", Sound::ItemGoatHornSound6),
            ("dream_goat_horn", Sound::ItemGoatHornSound7),
        ];
        for (instrument, sound) in expected {
            let properties = instrument_properties(instrument);
            assert_eq!(properties.sound, sound);
            assert_eq!(properties.use_duration_ticks, GoatHornItem::USE_DURATION);
            assert_eq!(properties.range, 256.0);
        }
    }
}
