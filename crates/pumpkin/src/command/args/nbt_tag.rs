use pumpkin_nbt::tag::NbtTag;
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

pub struct NbtTagArgumentConsumer;

impl GetClientSideArgParser for NbtTagArgumentConsumer {
    fn get_client_side_parser(&self) -> ArgumentType {
        ArgumentType::NbtTag
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<SuggestionProviders> {
        None
    }
}

impl ArgumentConsumer for NbtTagArgumentConsumer {
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
        let value = SnbtParser::parse_for_commands(&mut reader)
            .ok()
            .map(Arg::NbtTag);
        Box::pin(async move { value })
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
            SnbtParser::parse_for_commands(&mut reader).map(|tag| Some(Arg::NbtTag(tag)))
        })
    }
}

impl<'a> FindArg<'a> for NbtTagArgumentConsumer {
    type Data = &'a NbtTag;

    fn find_arg(args: &'a super::ConsumedArgs, name: &str) -> Result<Self::Data, CommandError> {
        match args.get(name) {
            Some(Arg::NbtTag(tag)) => Ok(tag),
            _ => Err(CommandError::InvalidConsumption(Some(name.to_owned()))),
        }
    }
}
