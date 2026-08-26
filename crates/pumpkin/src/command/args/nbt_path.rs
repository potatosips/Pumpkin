use pumpkin_protocol::java::client::play::{ArgumentType, SuggestionProviders};

use crate::{
    command::{
        CommandSender,
        args::{Arg, ArgumentConsumer, ConsumeResult, FindArg, GetClientSideArgParser},
        dispatcher::CommandError,
        nbt_path::NbtPath,
        tree::RawArgs,
    },
    server::Server,
};

pub struct NbtPathArgumentConsumer;

impl GetClientSideArgParser for NbtPathArgumentConsumer {
    fn get_client_side_parser(&self) -> ArgumentType {
        ArgumentType::NbtPath
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<SuggestionProviders> {
        None
    }
}

impl ArgumentConsumer for NbtPathArgumentConsumer {
    fn consume<'a>(
        &'a self,
        _sender: &'a CommandSender,
        _server: &'a Server,
        args: &mut RawArgs<'a>,
    ) -> ConsumeResult<'a> {
        let parsed = args
            .pop()
            .and_then(|raw| NbtPath::parse(raw.value).ok())
            .map(Arg::NbtPath);
        Box::pin(async move { parsed })
    }
}

impl<'a> FindArg<'a> for NbtPathArgumentConsumer {
    type Data = &'a NbtPath;

    fn find_arg(args: &'a super::ConsumedArgs, name: &str) -> Result<Self::Data, CommandError> {
        match args.get(name) {
            Some(Arg::NbtPath(path)) => Ok(path),
            _ => Err(CommandError::InvalidConsumption(Some(name.to_owned()))),
        }
    }
}
