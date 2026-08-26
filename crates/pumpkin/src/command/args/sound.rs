use pumpkin_data::sound::Sound;
use pumpkin_protocol::java::client::play::{ArgumentType, SuggestionProviders};
use pumpkin_util::text::TextComponent;

use crate::{
    command::{args::ConsumeResult, dispatcher::CommandError},
    server::Server,
};

use super::{
    super::{
        CommandSender,
        args::{ArgumentConsumer, RawArgs},
    },
    Arg, DefaultNameArgConsumer, FindArg, GetClientSideArgParser,
};

pub struct SoundArgumentConsumer;

impl GetClientSideArgParser for SoundArgumentConsumer {
    fn get_client_side_parser(&self) -> ArgumentType {
        ArgumentType::ResourceLocation
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<SuggestionProviders> {
        Some(SuggestionProviders::AvailableSounds)
    }
}

impl ArgumentConsumer for SoundArgumentConsumer {
    fn consume<'a, 'b>(
        &'a self,
        _sender: &'a CommandSender,
        _server: &'a Server,
        args: &'b mut RawArgs<'a>,
    ) -> ConsumeResult<'a> {
        let s_opt: Option<&'a str> = args.pop().map(|arg| arg.value);

        Box::pin(async move { s_opt.map(Arg::Block) })
    }
}

impl DefaultNameArgConsumer for SoundArgumentConsumer {
    fn default_name(&self) -> &'static str {
        "available_sounds"
    }
}

impl<'a> FindArg<'a> for SoundArgumentConsumer {
    type Data = Sound;

    fn find_arg(args: &'a super::ConsumedArgs, name: &str) -> Result<Self::Data, CommandError> {
        match args.get(name) {
            Some(Arg::Block(name)) => {
                Sound::from_name(name.strip_prefix("minecraft:").unwrap_or(name)).map_or_else(
                    || {
                        Err(CommandError::CommandFailed(TextComponent::translate_cross(
                            pumpkin_data::translation::java::ARGUMENT_ID_INVALID,
                            pumpkin_data::translation::java::ARGUMENT_ID_INVALID,
                            [],
                        )))
                    },
                    Result::Ok,
                )
            }
            _ => Err(CommandError::InvalidConsumption(Some(name.to_string()))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn vanilla_sound_lookup_and_invalid_sound_parity() {
        let mut args = HashMap::new();
        args.insert(
            "available_sounds",
            Arg::Block("entity.experience_orb.pickup"),
        );

        let res = SoundArgumentConsumer::find_arg(&args, "available_sounds");
        assert!(res.is_ok());

        let mut invalid_args = HashMap::new();
        invalid_args.insert("available_sounds", Arg::Block("invalid.nonexistent.sound"));
        let err_res = SoundArgumentConsumer::find_arg(&invalid_args, "available_sounds");
        assert!(err_res.is_err());
    }
}
