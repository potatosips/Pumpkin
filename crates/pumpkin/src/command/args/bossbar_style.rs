use crate::command::CommandSender;
use crate::command::args::{
    Arg, ArgumentConsumer, ConsumeResult, DefaultNameArgConsumer, FindArg, GetClientSideArgParser,
    SuggestResult,
};
use crate::command::dispatcher::CommandError;
use crate::command::tree::RawArgs;
use crate::server::Server;
use crate::world::bossbar::BossbarDivisions;
use pumpkin_protocol::java::client::play::{ArgumentType, CommandSuggestion, SuggestionProviders};

pub struct BossbarStyleArgumentConsumer;

impl GetClientSideArgParser for BossbarStyleArgumentConsumer {
    fn get_client_side_parser(&self) -> ArgumentType {
        // Not sure if this is right...
        ArgumentType::ResourceLocation
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<SuggestionProviders> {
        Some(SuggestionProviders::AskServer)
    }
}

impl ArgumentConsumer for BossbarStyleArgumentConsumer {
    fn consume<'a, 'b>(
        &'a self,
        _sender: &'a CommandSender,
        _server: &'a Server,
        args: &'b mut RawArgs<'a>,
    ) -> ConsumeResult<'a> {
        let s_opt: Option<&'a str> = args.pop().map(|arg| arg.value);

        let result: Option<Arg<'a>> = s_opt.map_or_else(
            || None,
            |s| {
                let style = match s {
                    "notched_10" => Some(BossbarDivisions::Notches10),
                    "notched_12" => Some(BossbarDivisions::Notches12),
                    "notched_20" => Some(BossbarDivisions::Notches20),
                    "notched_6" => Some(BossbarDivisions::Notches6),
                    "progress" => Some(BossbarDivisions::NoDivision),
                    _ => None,
                };

                style.map(Arg::BossbarStyle)
            },
        );

        Box::pin(async move { result })
    }

    fn suggest<'a>(
        &'a self,
        _sender: &CommandSender,
        _server: &'a Server,
        _input: &'a str,
    ) -> SuggestResult<'a> {
        Box::pin(async move {
            let styles = [
                "notched_10",
                "notched_12",
                "notched_20",
                "notched_6",
                "progress",
            ];
            let suggestions: Vec<CommandSuggestion> = styles
                .iter()
                .map(|style| CommandSuggestion::new((*style).to_string(), None))
                .collect();
            Ok(Some(suggestions))
        })
    }
}

impl DefaultNameArgConsumer for BossbarStyleArgumentConsumer {
    fn default_name(&self) -> &'static str {
        "style"
    }
}

impl<'a> FindArg<'a> for BossbarStyleArgumentConsumer {
    type Data = &'a BossbarDivisions;

    fn find_arg(args: &'a super::ConsumedArgs, name: &str) -> Result<Self::Data, CommandError> {
        match args.get(name) {
            Some(Arg::BossbarStyle(data)) => Ok(data),
            _ => Err(CommandError::InvalidConsumption(Some(name.to_string()))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bossbar_style_arg_parsing_parity() {
        let parse_style = |s: &str| -> Option<BossbarDivisions> {
            match s {
                "notched_10" => Some(BossbarDivisions::Notches10),
                "notched_12" => Some(BossbarDivisions::Notches12),
                "notched_20" => Some(BossbarDivisions::Notches20),
                "notched_6" => Some(BossbarDivisions::Notches6),
                "progress" => Some(BossbarDivisions::NoDivision),
                _ => None,
            }
        };

        assert!(matches!(
            parse_style("progress"),
            Some(BossbarDivisions::NoDivision)
        ));
        assert!(matches!(
            parse_style("notched_6"),
            Some(BossbarDivisions::Notches6)
        ));
        assert!(matches!(
            parse_style("notched_10"),
            Some(BossbarDivisions::Notches10)
        ));
        assert!(matches!(
            parse_style("notched_12"),
            Some(BossbarDivisions::Notches12)
        ));
        assert!(matches!(
            parse_style("notched_20"),
            Some(BossbarDivisions::Notches20)
        ));
        assert!(parse_style("notched_8").is_none());
    }
}
