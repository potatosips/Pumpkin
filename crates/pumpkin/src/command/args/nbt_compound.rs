use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
use pumpkin_protocol::java::client::play::{ArgumentType, SuggestionProviders};

use crate::{
    command::{
        CommandSender,
        args::{
            Arg, ArgumentConsumer, ConsumeResult, ConsumeResultWithSyntax, FindArg,
            GetClientSideArgParser,
        },
        dispatcher::CommandError,
        snbt::SnbtParser,
        string_reader::StringReader,
        tree::RawArgs,
    },
    server::Server,
};

pub struct NbtCompoundArgumentConsumer;

impl GetClientSideArgParser for NbtCompoundArgumentConsumer {
    fn get_client_side_parser(&self) -> ArgumentType {
        ArgumentType::NbtCompound
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<SuggestionProviders> {
        None
    }
}

impl ArgumentConsumer for NbtCompoundArgumentConsumer {
    fn consume<'a, 'b>(
        &'a self,
        _sender: &'a CommandSender,
        _server: &'a Server,
        args: &'b mut RawArgs<'a>,
    ) -> ConsumeResult<'a> {
        let Some(raw) = args.pop() else {
            return Box::pin(async { None });
        };
        let mut reader = StringReader::new(raw.value);
        let compound = match SnbtParser::parse_for_commands(&mut reader) {
            Ok(NbtTag::Compound(compound)) => Some(Arg::NbtCompound(compound)),
            _ => None,
        };
        Box::pin(async move { compound })
    }

    fn consume_with_syntax<'a>(
        &'a self,
        _sender: &'a CommandSender,
        _server: &'a Server,
        args: &mut RawArgs<'a>,
    ) -> ConsumeResultWithSyntax<'a> {
        let Some(raw) = args.pop() else {
            return Box::pin(async { Ok(None) });
        };
        let mut reader = StringReader::new(raw.input);
        reader.set_cursor(raw.start);
        Box::pin(async move {
            SnbtParser::parse_for_commands(&mut reader).and_then(|tag| match tag {
                NbtTag::Compound(compound) => Ok(Some(Arg::NbtCompound(compound))),
                _ => Err(
                    crate::command::argument_types::nbt::EXPECTED_COMPOUND_ERROR_TYPE
                        .create(&reader),
                ),
            })
        })
    }
}

impl<'a> FindArg<'a> for NbtCompoundArgumentConsumer {
    type Data = &'a NbtCompound;

    fn find_arg(args: &'a super::ConsumedArgs, name: &str) -> Result<Self::Data, CommandError> {
        match args.get(name) {
            Some(Arg::NbtCompound(compound)) => Ok(compound),
            _ => Err(CommandError::InvalidConsumption(Some(name.to_string()))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_summon_creeper_compound_preserves_numeric_tag_types() {
        let mut reader = StringReader::new("{powered:1b,ignited:1b,Fuse:40s,ExplosionRadius:1b}");
        let NbtTag::Compound(compound) =
            SnbtParser::parse_for_commands(&mut reader).expect("valid summon SNBT")
        else {
            panic!("summon NBT must be a compound");
        };

        assert_eq!(compound.get_bool("powered"), Some(true));
        assert_eq!(compound.get_bool("ignited"), Some(true));
        assert_eq!(compound.get_short("Fuse"), Some(40));
        assert_eq!(compound.get_byte("ExplosionRadius"), Some(1));
    }
}
