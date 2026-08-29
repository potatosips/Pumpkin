use core::f64;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use pumpkin_protocol::java::client::play::ArgumentType;
use pumpkin_util::text::{TextComponent, TranslationArgument};

use crate::command::CommandSender;
use crate::command::args::{ConsumeResult, ConsumeResultWithSyntax};
use crate::command::dispatcher::CommandError;
use crate::command::errors::command_syntax_error::{CommandSyntaxError, CommandSyntaxErrorContext};
use crate::command::errors::error_types;
use crate::command::tree::{RawArg, RawArgs};
use crate::server::Server;

use super::super::args::ArgumentConsumer;
use super::{Arg, DefaultNameArgConsumer, FindArg, GetClientSideArgParser};

/// Consumes a single generic num, but only if it's in bounds.
pub struct BoundedNumArgumentConsumer<T: ToFromNumber> {
    min_inclusive: Option<T>,
    max_inclusive: Option<T>,
    name: Option<&'static str>,
}

impl<T: ToFromNumber> ArgumentConsumer for BoundedNumArgumentConsumer<T>
where
    Self: GetClientSideArgParser,
{
    fn consume<'a, 'b>(
        &'a self,
        _sender: &'a CommandSender,
        _server: &'a Server,
        args: &'b mut RawArgs<'a>,
    ) -> ConsumeResult<'a> {
        let s_opt: Option<&'a str> = args.pop().map(|arg| arg.value);

        let result: Option<Arg<'a>> = s_opt
            // Replace args.pop()?.parse::<T>().ok()?
            .and_then(|s| s.parse::<T>().ok())
            .map(|x| {
                // Check Upper Bound (max_inclusive)
                if let Some(max) = self.max_inclusive
                    && x > max
                {
                    return Arg::Num(Err(NotInBounds::UpperBound(x.to_number(), max.to_number())));
                }

                // Check Lower Bound (min_inclusive)
                if let Some(min) = self.min_inclusive
                    && x < min
                {
                    return Arg::Num(Err(NotInBounds::LowerBound(x.to_number(), min.to_number())));
                }

                // Success case
                Arg::Num(Ok(x.to_number()))
            });

        Box::pin(async move { result })
    }

    fn consume_with_syntax<'a>(
        &'a self,
        _sender: &'a CommandSender,
        _server: &'a Server,
        args: &mut RawArgs<'a>,
    ) -> ConsumeResultWithSyntax<'a> {
        let result = args.pop().map(|raw| self.parse_raw(raw));
        Box::pin(async move { result.transpose() })
    }
}

impl<'a, T: 'static + ToFromNumber> FindArg<'a> for BoundedNumArgumentConsumer<T> {
    type Data = Result<T, NotInBounds>;

    fn find_arg(args: &'a super::ConsumedArgs, name: &str) -> Result<Self::Data, CommandError> {
        match args.get(name) {
            Some(Arg::Num(data)) => match data {
                Ok(num) => T::from_number(num).map_or_else(
                    || Err(CommandError::InvalidConsumption(Some(name.to_string()))),
                    |x| Ok(Ok(x)),
                ),
                Err(err) => Ok(Err(*err)),
            },
            _ => Err(CommandError::InvalidConsumption(Some(name.to_string()))),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum NotInBounds {
    /// Number is lower than the lower bound
    LowerBound(Number, Number),
    /// Number is higher then the upper bound
    UpperBound(Number, Number),
}

impl From<NotInBounds> for CommandError {
    fn from(value: NotInBounds) -> Self {
        match value {
            NotInBounds::LowerBound(val, min) => {
                if let (Number::I32(value), Number::I32(minimum)) = (val, min) {
                    let key = pumpkin_data::translation::java::ARGUMENT_INTEGER_LOW;
                    return Self::CommandFailed(TextComponent::translate_cross_args(
                        key,
                        key,
                        vec![
                            TranslationArgument::from(minimum),
                            TranslationArgument::from(value),
                        ],
                    ));
                }
                let (key, min_text, val_text) = match val {
                    Number::F64(_) | Number::F32(_) => (
                        pumpkin_data::translation::java::ARGUMENT_DOUBLE_LOW,
                        min.to_string(),
                        val.to_string(),
                    ),
                    Number::I32(_) => (
                        pumpkin_data::translation::java::ARGUMENT_INTEGER_LOW,
                        min.to_string(),
                        val.to_string(),
                    ),
                    Number::I64(_) => (
                        pumpkin_data::translation::java::ARGUMENT_LONG_LOW,
                        min.to_string(),
                        val.to_string(),
                    ),
                };
                Self::CommandFailed(TextComponent::translate_cross(
                    key,
                    key,
                    [TextComponent::text(min_text), TextComponent::text(val_text)],
                ))
            }
            NotInBounds::UpperBound(val, max) => {
                if let (Number::I32(value), Number::I32(maximum)) = (val, max) {
                    let key = pumpkin_data::translation::java::ARGUMENT_INTEGER_BIG;
                    return Self::CommandFailed(TextComponent::translate_cross_args(
                        key,
                        key,
                        vec![
                            TranslationArgument::from(maximum),
                            TranslationArgument::from(value),
                        ],
                    ));
                }
                let (key, max_text, val_text) = match val {
                    Number::F64(_) | Number::F32(_) => (
                        pumpkin_data::translation::java::ARGUMENT_DOUBLE_BIG,
                        max.to_string(),
                        val.to_string(),
                    ),
                    Number::I32(_) => (
                        pumpkin_data::translation::java::ARGUMENT_INTEGER_BIG,
                        max.to_string(),
                        val.to_string(),
                    ),
                    Number::I64(_) => (
                        pumpkin_data::translation::java::ARGUMENT_LONG_BIG,
                        max.to_string(),
                        val.to_string(),
                    ),
                };
                Self::CommandFailed(TextComponent::translate_cross(
                    key,
                    key,
                    [TextComponent::text(max_text), TextComponent::text(val_text)],
                ))
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Number {
    F64(f64),
    F32(f32),
    I32(i32),
    I64(i64),
}

impl Number {
    #[must_use]
    pub const fn qualifier(&self) -> &'static str {
        match self {
            Self::F64(_) | Self::F32(_) => "Float",
            Self::I32(_) | Self::I64(_) => "Integer",
        }
    }
}

impl Display for Number {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::F64(v) => write!(f, "{v}"),
            Self::F32(v) => write!(f, "{v}"),
            Self::I32(v) => write!(f, "{v}"),
            Self::I64(v) => write!(f, "{v}"),
        }
    }
}

impl<T: ToFromNumber> BoundedNumArgumentConsumer<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            min_inclusive: None,
            max_inclusive: None,
            name: None,
        }
    }

    #[must_use]
    pub const fn min(mut self, min_inclusive: T) -> Self {
        self.min_inclusive = Some(min_inclusive);
        self
    }

    #[must_use]
    pub const fn max(mut self, max_inclusive: T) -> Self {
        self.max_inclusive = Some(max_inclusive);
        self
    }

    #[must_use]
    pub const fn name(mut self, name: &'static str) -> Self {
        self.name = Some(name);
        self
    }

    fn parse_raw<'a>(&self, raw: RawArg<'a>) -> Result<Arg<'a>, CommandSyntaxError> {
        let context = CommandSyntaxErrorContext {
            input: raw.input.to_string(),
            cursor: raw.start,
        };
        let value = raw
            .value
            .parse::<T>()
            .map_err(|_| T::invalid_error(&context, raw.value))?;
        if let Some(maximum) = self.max_inclusive
            && value > maximum
        {
            return Err(bound_error(
                &context,
                value.to_number(),
                maximum.to_number(),
                false,
            ));
        }
        if let Some(minimum) = self.min_inclusive
            && value < minimum
        {
            return Err(bound_error(
                &context,
                value.to_number(),
                minimum.to_number(),
                true,
            ));
        }
        Ok(Arg::Num(Ok(value.to_number())))
    }
}

fn bound_error(
    context: &CommandSyntaxErrorContext,
    value: Number,
    bound: Number,
    low: bool,
) -> CommandSyntaxError {
    let text_args = || {
        [
            TextComponent::text(bound.to_string()),
            TextComponent::text(value.to_string()),
        ]
    };
    match (value, bound, low) {
        (Number::I32(value), Number::I32(bound), true) => error_types::INTEGER_TOO_LOW
            .create_translation_args(context, [bound.into(), value.into()]),
        (Number::I32(value), Number::I32(bound), false) => error_types::INTEGER_TOO_HIGH
            .create_translation_args(context, [bound.into(), value.into()]),
        (Number::I64(_), Number::I64(_), true) => {
            error_types::LONG_TOO_LOW.create_args_slice(context, &text_args())
        }
        (Number::I64(_), Number::I64(_), false) => {
            error_types::LONG_TOO_HIGH.create_args_slice(context, &text_args())
        }
        (Number::F32(_), Number::F32(_), true) => {
            error_types::FLOAT_TOO_LOW.create_args_slice(context, &text_args())
        }
        (Number::F32(_), Number::F32(_), false) => {
            error_types::FLOAT_TOO_HIGH.create_args_slice(context, &text_args())
        }
        (Number::F64(_), Number::F64(_), true) => {
            error_types::DOUBLE_TOO_LOW.create_args_slice(context, &text_args())
        }
        (Number::F64(_), Number::F64(_), false) => {
            error_types::DOUBLE_TOO_HIGH.create_args_slice(context, &text_args())
        }
        _ => unreachable!("numeric value and bound types must match"),
    }
}

pub trait ToFromNumber: PartialOrd + Copy + Send + Sync + FromStr {
    fn to_number(self) -> Number;
    fn from_number(arg: &Number) -> Option<Self>;
    fn invalid_error(context: &CommandSyntaxErrorContext, value: &str) -> CommandSyntaxError;
}

impl<T: ToFromNumber> Default for BoundedNumArgumentConsumer<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl ToFromNumber for f64 {
    fn to_number(self) -> Number {
        Number::F64(self)
    }

    fn from_number(arg: &Number) -> Option<Self> {
        match arg {
            Number::F64(x) => Some(*x),
            _ => None,
        }
    }

    fn invalid_error(context: &CommandSyntaxErrorContext, value: &str) -> CommandSyntaxError {
        error_types::READER_INVALID_DOUBLE.create(context, TextComponent::text(value.to_string()))
    }
}

impl GetClientSideArgParser for BoundedNumArgumentConsumer<f64> {
    fn get_client_side_parser(&self) -> ArgumentType {
        ArgumentType::Double {
            min: self.min_inclusive,
            max: self.max_inclusive,
        }
    }

    fn get_client_side_suggestion_type_override(
        &self,
    ) -> Option<pumpkin_protocol::java::client::play::SuggestionProviders> {
        None
    }
}

impl ToFromNumber for f32 {
    fn to_number(self) -> Number {
        Number::F32(self)
    }

    fn from_number(arg: &Number) -> Option<Self> {
        match arg {
            Number::F32(x) => Some(*x),
            _ => None,
        }
    }

    fn invalid_error(context: &CommandSyntaxErrorContext, value: &str) -> CommandSyntaxError {
        error_types::READER_INVALID_FLOAT.create(context, TextComponent::text(value.to_string()))
    }
}

impl GetClientSideArgParser for BoundedNumArgumentConsumer<f32> {
    fn get_client_side_parser(&self) -> ArgumentType {
        ArgumentType::Float {
            min: self.min_inclusive,
            max: self.max_inclusive,
        }
    }

    fn get_client_side_suggestion_type_override(
        &self,
    ) -> Option<pumpkin_protocol::java::client::play::SuggestionProviders> {
        None
    }
}

impl ToFromNumber for i32 {
    fn to_number(self) -> Number {
        Number::I32(self)
    }

    fn from_number(arg: &Number) -> Option<Self> {
        match arg {
            Number::I32(x) => Some(*x),
            _ => None,
        }
    }

    fn invalid_error(context: &CommandSyntaxErrorContext, value: &str) -> CommandSyntaxError {
        error_types::READER_INVALID_INT
            .create_translation_args(context, [TranslationArgument::from(value.to_string())])
    }
}

impl GetClientSideArgParser for BoundedNumArgumentConsumer<i32> {
    fn get_client_side_parser(&self) -> ArgumentType {
        ArgumentType::Integer {
            min: self.min_inclusive,
            max: self.max_inclusive,
        }
    }

    fn get_client_side_suggestion_type_override(
        &self,
    ) -> Option<pumpkin_protocol::java::client::play::SuggestionProviders> {
        None
    }
}

impl ToFromNumber for i64 {
    fn to_number(self) -> Number {
        Number::I64(self)
    }

    fn from_number(arg: &Number) -> Option<Self> {
        match arg {
            Number::I64(x) => Some(*x),
            _ => None,
        }
    }

    fn invalid_error(context: &CommandSyntaxErrorContext, value: &str) -> CommandSyntaxError {
        error_types::READER_INVALID_LONG.create(context, TextComponent::text(value.to_string()))
    }
}

impl GetClientSideArgParser for BoundedNumArgumentConsumer<i64> {
    fn get_client_side_parser(&self) -> ArgumentType {
        ArgumentType::Long {
            min: self.min_inclusive,
            max: self.max_inclusive,
        }
    }

    fn get_client_side_suggestion_type_override(
        &self,
    ) -> Option<pumpkin_protocol::java::client::play::SuggestionProviders> {
        None
    }
}

impl<T: ToFromNumber> DefaultNameArgConsumer for BoundedNumArgumentConsumer<T>
where
    Self: ArgumentConsumer,
{
    fn default_name(&self) -> &'static str {
        // setting a single default name for all BoundedNumArgumentConsumer variants is probably a bad idea since it would lead to confusion
        self.name.expect("Only use *_default variants of methods with a BoundedNumArgumentConsumer that has a name.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_num_conversion_and_bounds_parity() {
        let consumer_i32 = BoundedNumArgumentConsumer::<i32>::new().min(1).max(10);
        assert_eq!(consumer_i32.min_inclusive, Some(1));
        assert_eq!(consumer_i32.max_inclusive, Some(10));

        let num_i32 = 5i32.to_number();
        assert_eq!(i32::from_number(&num_i32), Some(5));
        assert_eq!(f64::from_number(&num_i32), None);

        let consumer_f64 = BoundedNumArgumentConsumer::<f64>::new().min(0.0).max(1.0);
        assert_eq!(consumer_f64.min_inclusive, Some(0.0));
        assert_eq!(consumer_f64.max_inclusive, Some(1.0));

        let num_f64 = 0.75f64.to_number();
        assert_eq!(f64::from_number(&num_f64), Some(0.75));
        assert_eq!(i32::from_number(&num_f64), None);
    }

    #[test]
    fn integer_bound_errors_preserve_native_integer_arguments() {
        let error = CommandError::from(NotInBounds::LowerBound(Number::I32(-1), Number::I32(0)));
        let CommandError::CommandFailed(component) = error else {
            panic!("expected a command failure");
        };
        let nbt = component.0.to_nbt_compound();

        assert_eq!(nbt.get_string("translate"), Some("argument.integer.low"));
        assert_eq!(nbt.get_int_array("with"), Some(&[0, -1][..]));
    }

    #[test]
    fn integer_parser_errors_preserve_brigadier_type_arguments_and_cursor() {
        let input = "gamerule spawnChunkRadius -1";
        let start = input.rfind("-1").unwrap();
        let consumer = BoundedNumArgumentConsumer::<i32>::new().min(0).max(32);
        let low = consumer
            .parse_raw(RawArg {
                value: "-1",
                start,
                end: input.len(),
                input,
            })
            .err()
            .expect("out-of-range integer must be a syntax error");
        assert!(low.is(&error_types::INTEGER_TOO_LOW));
        assert_eq!(low.context.unwrap().cursor, start);
        assert_eq!(
            low.message.0.to_nbt_compound().get_int_array("with"),
            Some(&[0, -1][..])
        );

        let invalid_input = "gamerule snowAccumulationHeight 2147483648";
        let invalid_start = invalid_input.rfind("2147483648").unwrap();
        let invalid = BoundedNumArgumentConsumer::<i32>::new()
            .parse_raw(RawArg {
                value: "2147483648",
                start: invalid_start,
                end: invalid_input.len(),
                input: invalid_input,
            })
            .err()
            .expect("overflowing integer must be a syntax error");
        assert!(invalid.is(&error_types::READER_INVALID_INT));
        assert_eq!(invalid.context.unwrap().cursor, invalid_start);
        let nbt = invalid.message.0.to_nbt_compound();
        assert_eq!(nbt.get_string("translate"), Some("parsing.int.invalid"));
        assert_eq!(
            nbt.get_list("with").unwrap().first(),
            Some(&pumpkin_nbt::tag::NbtTag::String("2147483648".into()))
        );
    }
}
