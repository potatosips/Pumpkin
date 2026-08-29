use crate::command::CommandSender;
use crate::command::args::{
    Arg, ArgumentConsumer, ConsumeResult, ConsumeResultWithSyntax, FindArg, GetClientSideArgParser,
};
use crate::command::dispatcher::CommandError;
use crate::command::errors::command_syntax_error::CommandSyntaxErrorContext;
use crate::command::errors::error_types::READER_INVALID_BOOL;
use crate::command::tree::RawArgs;
use crate::server::Server;
use pumpkin_protocol::java::client::play::{ArgumentType, SuggestionProviders};
use pumpkin_util::text::TranslationArgument;

pub struct BoolArgConsumer;

fn parse_raw_bool<'a>(
    raw: crate::command::tree::RawArg<'a>,
) -> Result<Arg<'a>, crate::command::errors::command_syntax_error::CommandSyntaxError> {
    match raw.value {
        "false" => Ok(Arg::Bool(false)),
        "true" => Ok(Arg::Bool(true)),
        invalid => Err(READER_INVALID_BOOL.create_translation_args(
            &CommandSyntaxErrorContext {
                input: raw.input.to_string(),
                cursor: raw.start,
            },
            [TranslationArgument::from(invalid.to_string())],
        )),
    }
}

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

    fn consume_with_syntax<'a>(
        &'a self,
        _sender: &'a CommandSender,
        _server: &'a Server,
        args: &mut RawArgs<'a>,
    ) -> ConsumeResultWithSyntax<'a> {
        let result = args.pop().map(parse_raw_bool);
        Box::pin(async move { result.transpose() })
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

    #[test]
    fn invalid_bool_preserves_brigadier_type_value_and_cursor() {
        let input = "gamerule doDaylightCycle 1";
        let start = input.rfind('1').unwrap();
        let error = parse_raw_bool(crate::command::tree::RawArg {
            value: "1",
            start,
            end: input.len(),
            input,
        })
        .err()
        .expect("invalid boolean must be a syntax error");

        assert!(error.is(&READER_INVALID_BOOL));
        assert_eq!(error.context.unwrap().cursor, start);
        let nbt = error.message.0.to_nbt_compound();
        assert_eq!(nbt.get_string("translate"), Some("parsing.bool.invalid"));
        assert_eq!(
            nbt.get_list("with").unwrap().first(),
            Some(&pumpkin_nbt::tag::NbtTag::String("1".into()))
        );
    }
}
