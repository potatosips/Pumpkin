use std::sync::Arc;

use pumpkin_data::block_properties::{
    BlockProperties, TrialSpawnerLikeProperties, TrialSpawnerState,
};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;

use crate::block::entities::trial_spawner::TrialSpawnerBlockEntity;
use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, BlockFuture, NormalUseArgs, PlacedArgs, UseWithItemArgs};

#[pumpkin_block("minecraft:trial_spawner")]
pub struct TrialSpawnerBlock;

impl BlockBehaviour for TrialSpawnerBlock {
    fn placed<'a>(&'a self, args: PlacedArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            let entity = TrialSpawnerBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(entity));
        })
    }

    fn normal_use<'a>(&'a self, args: NormalUseArgs<'a>) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async move {
            let state_id = args.world.get_block_state_id(args.position);
            let mut props = TrialSpawnerLikeProperties::from_state_id(state_id, args.block);

            // Check if player has Trial Omen or Bad Omen to convert to Ominous
            let is_player_ominous = args
                .player
                .living_entity
                .has_effect(&pumpkin_data::effect::StatusEffect::TRIAL_OMEN)
                .await
                || args
                    .player
                    .living_entity
                    .has_effect(&pumpkin_data::effect::StatusEffect::BAD_OMEN)
                    .await;

            if is_player_ominous && !props.ominous {
                props.ominous = true;
                args.world.play_sound(
                    Sound::BlockTrialSpawnerOminousActivate,
                    SoundCategory::Blocks,
                    &args.position.to_f64(),
                );
            }

            match props.trial_spawner_state {
                TrialSpawnerState::Inactive | TrialSpawnerState::WaitingForPlayers => {
                    props.trial_spawner_state = TrialSpawnerState::Active;
                    args.world
                        .set_block_state(
                            args.position,
                            props.to_state_id(args.block),
                            BlockFlags::NOTIFY_ALL,
                        )
                        .await;

                    if props.ominous {
                        args.world.play_sound(
                            Sound::BlockTrialSpawnerOminousActivate,
                            SoundCategory::Blocks,
                            &args.position.to_f64(),
                        );
                    } else {
                        args.world.play_sound(
                            Sound::BlockTrialSpawnerDetectPlayer,
                            SoundCategory::Blocks,
                            &args.position.to_f64(),
                        );
                    }
                    args.world.play_sound(
                        Sound::BlockTrialSpawnerOpenShutter,
                        SoundCategory::Blocks,
                        &args.position.to_f64(),
                    );
                }
                TrialSpawnerState::Active => {
                    // Eject trial rewards & key drop upon trial wave completion
                    props.trial_spawner_state = TrialSpawnerState::WaitingForRewardEjection;
                    args.world
                        .set_block_state(
                            args.position,
                            props.to_state_id(args.block),
                            BlockFlags::NOTIFY_ALL,
                        )
                        .await;

                    args.world.play_sound(
                        Sound::BlockTrialSpawnerSpawnItemBegin,
                        SoundCategory::Blocks,
                        &args.position.to_f64(),
                    );
                    args.world.play_sound(
                        Sound::BlockTrialSpawnerEjectItem,
                        SoundCategory::Blocks,
                        &args.position.to_f64(),
                    );

                    let reward_item = Self::get_reward_key(props.ominous);
                    let key_stack = ItemStack::new(1, reward_item);
                    args.world.drop_stack(args.position, key_stack).await;

                    props.trial_spawner_state = TrialSpawnerState::Cooldown;
                    args.world
                        .set_block_state(
                            args.position,
                            props.to_state_id(args.block),
                            BlockFlags::NOTIFY_ALL,
                        )
                        .await;

                    args.world.play_sound(
                        Sound::BlockTrialSpawnerCloseShutter,
                        SoundCategory::Blocks,
                        &args.position.to_f64(),
                    );
                }
                TrialSpawnerState::Cooldown => {
                    args.world.play_sound(
                        Sound::BlockTrialSpawnerAmbient,
                        SoundCategory::Blocks,
                        &args.position.to_f64(),
                    );
                }
                TrialSpawnerState::WaitingForRewardEjection | TrialSpawnerState::EjectingReward => {
                    props.trial_spawner_state = TrialSpawnerState::Cooldown;
                    args.world
                        .set_block_state(
                            args.position,
                            props.to_state_id(args.block),
                            BlockFlags::NOTIFY_ALL,
                        )
                        .await;
                }
            }

            BlockActionResult::Success
        })
    }

    fn use_with_item<'a>(
        &'a self,
        args: UseWithItemArgs<'a>,
    ) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async move {
            self.normal_use(NormalUseArgs {
                server: args.server,
                world: args.world,
                block: args.block,
                position: args.position,
                player: args.player,
                hit: args.hit,
            })
            .await
        })
    }
}

impl TrialSpawnerBlock {
    #[must_use]
    pub const fn get_reward_key(is_ominous: bool) -> &'static Item {
        if is_ominous {
            &Item::OMINOUS_TRIAL_KEY
        } else {
            &Item::TRIAL_KEY
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_trial_spawner_reward_keys() {
        assert_eq!(
            TrialSpawnerBlock::get_reward_key(false).id,
            Item::TRIAL_KEY.id
        );
        assert_eq!(
            TrialSpawnerBlock::get_reward_key(true).id,
            Item::OMINOUS_TRIAL_KEY.id
        );
    }
}
