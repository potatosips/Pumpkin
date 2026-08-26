use crate::command::CommandSender;
use crate::command::args::{Arg, ArgumentConsumer, ConsumeResult, FindArg, GetClientSideArgParser};
use crate::command::dispatcher::CommandError;
use crate::command::tree::RawArgs;
use crate::server::Server;
use pumpkin_protocol::java::client::play::{ArgumentType, SuggestionProviders};

pub struct BoolArgConsumer;

impl GetClientSideArgParser for BoolArgConsumer {
    fn get_client_side_parser(&self) -> ArgumentType {
        ArgumentType::Bool
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<SuggestionProviders> {
        None
    }
}

impl ArgumentConsumer for BoolArgConsumer {
    fn consume<'a, 'b>(
        &'a self,
        _sender: &'a CommandSender,
        _server: &'a Server,
        args: &'b mut RawArgs<'a>,
    ) -> ConsumeResult<'a> {
        let s_opt: Option<&'a str> = args.pop().map(|arg| arg.value);

        let result: Option<Arg<'a>> = s_opt.map_or_else(
            || None,
            |s| match s {
                "false" => Some(Arg::Bool(false)),
                "true" => Some(Arg::Bool(true)),
                _ => None,
            },
        );

        Box::pin(async move { result })
    }
}

impl<'a> FindArg<'a> for BoolArgConsumer {
    type Data = bool;

    fn find_arg(args: &'a super::ConsumedArgs, name: &str) -> Result<Self::Data, CommandError> {
        match args.get(name) {
            Some(Arg::Bool(data)) => Ok(*data),
            _ => Err(CommandError::InvalidConsumption(Some(name.to_string()))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn vanilla_bool_arg_lookup_parity() {
        let mut true_args = HashMap::new();
        true_args.insert("flag", Arg::Bool(true));
        let res = BoolArgConsumer::find_arg(&true_args, "flag");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), true);

        let mut false_args = HashMap::new();
        false_args.insert("flag", Arg::Bool(false));
        let res_false = BoolArgConsumer::find_arg(&false_args, "flag");
        assert!(res_false.is_ok());
        assert_eq!(res_false.unwrap(), false);
    }
}
