use pumpkin_protocol::java::client::play::{ArgumentType, SuggestionProviders};

use crate::command::tree::RawArgs;
use crate::command::{CommandSender, args::ConsumeResult};
use crate::server::Server;

use super::{Arg, ArgumentConsumer, GetClientSideArgParser};

fn wrap_degrees(mut angle: f32) -> f32 {
    angle %= 360.0;
    if angle >= 180.0 {
        angle -= 360.0;
    } else if angle < -180.0 {
        angle += 360.0;
    }
    angle
}

/// A single yaw angle, matching Minecraft's `minecraft:angle` argument.
pub struct AngleArgumentConsumer;

impl GetClientSideArgParser for AngleArgumentConsumer {
    fn get_client_side_parser(&self) -> ArgumentType {
        ArgumentType::Angle
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<SuggestionProviders> {
        None
    }
}

impl ArgumentConsumer for AngleArgumentConsumer {
    fn consume<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a Server,
        args: &mut RawArgs<'a>,
    ) -> ConsumeResult<'a> {
        let value = args.pop().and_then(|argument| {
            let (text, relative) = argument
                .value
                .strip_prefix('~')
                .map_or((argument.value, false), |rest| (rest, true));
            let offset = if text.is_empty() {
                0.0
            } else {
                text.parse::<f32>().ok()?
            };
            if !offset.is_finite() {
                return None;
            }
            let angle = if relative {
                sender.rotation()?.0 + offset
            } else {
                offset
            };
            Some(Arg::Angle(wrap_degrees(angle)))
        });
        Box::pin(async move { value })
    }
}

#[cfg(test)]
mod tests {
    use super::wrap_degrees;

    #[test]
    fn absolute_angle_normalization_matches_vanilla_wrap() {
        assert_eq!(wrap_degrees(450.0), 90.0);
        assert_eq!(wrap_degrees(270.0), -90.0);
        assert_eq!(wrap_degrees(-270.0), 90.0);
    }
}
