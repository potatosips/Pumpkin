use pumpkin_data::tag::{RegistryKey, get_tag_ids};
use pumpkin_data::{Block, BlockStateId, translation};
use pumpkin_protocol::java::client::play::{ArgumentType, SuggestionProviders};
use pumpkin_util::text::TextComponent;
use pumpkin_world::generation::structure::template::{BlockStateResolver, PaletteEntry};

use crate::command::args::ConsumeResult;
use crate::{command::dispatcher::CommandError, server::Server};

use super::{
    super::{
        CommandSender,
        args::{ArgumentConsumer, RawArgs},
    },
    Arg, ConsumedArgs, DefaultNameArgConsumer, FindArg, GetClientSideArgParser,
};

pub struct BlockArgumentConsumer;

impl BlockArgumentConsumer {
    pub fn find_state_arg(
        args: &ConsumedArgs<'_>,
        name: &str,
    ) -> Result<(&'static Block, BlockStateId), CommandError> {
        let Some(Arg::Block(raw)) = args.get(name) else {
            return Err(CommandError::InvalidConsumption(Some(name.to_string())));
        };
        let (block_name, properties) = if let Some(open) = raw.find('[') {
            let Some(close) = raw.rfind(']') else {
                return Err(invalid_block(raw));
            };
            if close + 1 != raw.len() {
                return Err(invalid_block(raw));
            }
            let properties = raw[open + 1..close]
                .split(',')
                .filter(|property| !property.is_empty())
                .map(|property| {
                    let (key, value) =
                        property.split_once('=').ok_or_else(|| invalid_block(raw))?;
                    Ok((key.to_owned(), value.to_owned()))
                })
                .collect::<Result<Vec<_>, CommandError>>()?;
            (&raw[..open], properties)
        } else {
            (*raw, Vec::new())
        };
        let block = Block::from_name(block_name).ok_or_else(|| invalid_block(raw))?;
        let state_id = BlockStateResolver::resolve_simple(&PaletteEntry::with_properties(
            block_name.to_owned(),
            properties,
        ))
        .map(|state| state.id)
        .ok_or_else(|| invalid_block(raw))?;
        Ok((block, state_id))
    }
}

fn invalid_block(name: &str) -> CommandError {
    let name = if name.starts_with("minecraft:") {
        name.to_owned()
    } else {
        "minecraft:".to_owned() + name
    };
    CommandError::CommandFailed(TextComponent::translate_cross(
        translation::java::ARGUMENT_BLOCK_ID_INVALID,
        translation::java::ARGUMENT_BLOCK_ID_INVALID,
        [TextComponent::text(name)],
    ))
}

impl GetClientSideArgParser for BlockArgumentConsumer {
    fn get_client_side_parser(&self) -> ArgumentType {
        ArgumentType::BlockState
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<SuggestionProviders> {
        None
    }
}

impl ArgumentConsumer for BlockArgumentConsumer {
    fn consume<'a>(
        &'a self,
        _sender: &'a CommandSender,
        _server: &'a Server,
        args: &mut RawArgs<'a>,
    ) -> ConsumeResult<'a> {
        let block = args.pop().map(|arg| arg.value);
        match block {
            Some(s) => Box::pin(async move { Some(Arg::Block(s)) }),
            None => Box::pin(async move { None }),
        }
    }
}

impl DefaultNameArgConsumer for BlockArgumentConsumer {
    fn default_name(&self) -> &'static str {
        "block"
    }
}

impl<'a> FindArg<'a> for BlockArgumentConsumer {
    type Data = &'static Block;

    fn find_arg(args: &'a super::ConsumedArgs, name: &str) -> Result<Self::Data, CommandError> {
        Self::find_state_arg(args, name).map(|(block, _)| block)
    }
}

pub struct BlockPredicateArgumentConsumer;

#[derive(Debug, Clone)]
pub enum BlockPredicate {
    Tag {
        blocks: Vec<u16>,
        properties: Vec<(String, String)>,
    },
    Block {
        block: &'static Block,
        properties: Vec<(String, String)>,
    },
}

impl BlockPredicate {
    #[must_use]
    pub fn matches(&self, block: &Block, state_id: BlockStateId) -> bool {
        match self {
            Self::Tag { blocks, properties } => {
                if !blocks.contains(&block.id.as_u16()) {
                    return false;
                }
                if properties.is_empty() {
                    return true;
                }
                let Some(block_props) = block.properties(state_id) else {
                    return false;
                };
                let props = block_props.to_props();
                for (req_k, req_v) in properties {
                    if !props
                        .iter()
                        .any(|(k, v)| *k == req_k.as_str() && *v == req_v.as_str())
                    {
                        return false;
                    }
                }
                true
            }
            Self::Block {
                block: target_block,
                properties,
            } => {
                if block.id != target_block.id {
                    return false;
                }
                if properties.is_empty() {
                    return true;
                }
                let Some(block_props) = block.properties(state_id) else {
                    return false;
                };
                let props = block_props.to_props();
                for (req_k, req_v) in properties {
                    if !props
                        .iter()
                        .any(|(k, v)| *k == req_k.as_str() && *v == req_v.as_str())
                    {
                        return false;
                    }
                }
                true
            }
        }
    }
}

impl GetClientSideArgParser for BlockPredicateArgumentConsumer {
    fn get_client_side_parser(&self) -> ArgumentType {
        ArgumentType::BlockPredicate
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<SuggestionProviders> {
        None
    }
}

impl ArgumentConsumer for BlockPredicateArgumentConsumer {
    fn consume<'a>(
        &'a self,
        _sender: &'a CommandSender,
        _server: &'a Server,
        args: &mut RawArgs<'a>,
    ) -> ConsumeResult<'a> {
        let block = args.pop().map(|arg| arg.value);
        match block {
            Some(s) => Box::pin(async move { Some(Arg::BlockPredicate(s)) }),
            None => Box::pin(async move { None }),
        }
    }
}

impl DefaultNameArgConsumer for BlockPredicateArgumentConsumer {
    fn default_name(&self) -> &'static str {
        "filter"
    }
}

impl<'a> FindArg<'a> for BlockPredicateArgumentConsumer {
    type Data = Option<BlockPredicate>;

    fn find_arg(args: &'a super::ConsumedArgs, name: &str) -> Result<Self::Data, CommandError> {
        let Some(Arg::BlockPredicate(raw)) = args.get(name) else {
            return Ok(None);
        };
        let (base, properties) = if let Some(open) = raw.find('[') {
            let Some(close) = raw.rfind(']') else {
                return Err(invalid_predicate(raw));
            };
            if close + 1 != raw.len() {
                return Err(invalid_predicate(raw));
            }
            let properties = raw[open + 1..close]
                .split(',')
                .filter(|property| !property.is_empty())
                .map(|property| {
                    let (key, value) = property
                        .split_once('=')
                        .ok_or_else(|| invalid_predicate(raw))?;
                    Ok((key.to_owned(), value.to_owned()))
                })
                .collect::<Result<Vec<_>, CommandError>>()?;
            (&raw[..open], properties)
        } else {
            (*raw, Vec::new())
        };

        if let Some(tag) = base.strip_prefix('#') {
            let blocks = get_tag_ids(RegistryKey::Block, tag).ok_or_else(|| {
                CommandError::CommandFailed(TextComponent::translate_cross(
                    translation::java::ARGUMENTS_BLOCK_TAG_UNKNOWN,
                    translation::java::ARGUMENTS_BLOCK_TAG_UNKNOWN,
                    [TextComponent::text((*tag).to_string())],
                ))
            })?;
            Ok(Some(BlockPredicate::Tag {
                blocks: blocks.to_vec(),
                properties,
            }))
        } else {
            let block = Block::from_name(base).ok_or_else(|| {
                let name = if base.starts_with("minecraft:") {
                    base.to_owned()
                } else {
                    "minecraft:".to_owned() + base
                };
                CommandError::CommandFailed(TextComponent::translate_cross(
                    translation::java::ARGUMENT_BLOCK_ID_INVALID,
                    translation::java::ARGUMENT_BLOCK_ID_INVALID,
                    [TextComponent::text(name)],
                ))
            })?;
            Ok(Some(BlockPredicate::Block { block, properties }))
        }
    }
}

fn invalid_predicate(name: &str) -> CommandError {
    if let Some(tag) = name.strip_prefix('#') {
        let tag_name = if let Some(open) = tag.find('[') {
            &tag[..open]
        } else {
            tag
        };
        CommandError::CommandFailed(TextComponent::translate_cross(
            translation::java::ARGUMENTS_BLOCK_TAG_UNKNOWN,
            translation::java::ARGUMENTS_BLOCK_TAG_UNKNOWN,
            [TextComponent::text(tag_name.to_string())],
        ))
    } else {
        let block_name = if let Some(open) = name.find('[') {
            &name[..open]
        } else {
            name
        };
        let block_name = if block_name.starts_with("minecraft:") {
            block_name.to_owned()
        } else {
            "minecraft:".to_owned() + block_name
        };
        CommandError::CommandFailed(TextComponent::translate_cross(
            translation::java::ARGUMENT_BLOCK_ID_INVALID,
            translation::java::ARGUMENT_BLOCK_ID_INVALID,
            [TextComponent::text(block_name)],
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn vanilla_block_and_block_tag_predicate_parity() {
        let mut args = HashMap::new();
        args.insert("block", Arg::Block("minecraft:stone"));

        let res = BlockArgumentConsumer::find_arg(&args, "block");
        assert!(res.is_ok());
        let block = res.unwrap();
        assert_eq!(block.id, Block::STONE.id);

        let mut pred_args = HashMap::new();
        pred_args.insert("filter", Arg::BlockPredicate("#minecraft:logs"));
        let pred_res = BlockPredicateArgumentConsumer::find_arg(&pred_args, "filter");
        assert!(pred_res.is_ok());
        let pred = pred_res.unwrap().unwrap();
        assert!(matches!(pred, BlockPredicate::Tag { .. }));

        // Property-bearing block predicate
        let mut prop_pred_args = HashMap::new();
        prop_pred_args.insert(
            "filter",
            Arg::BlockPredicate("minecraft:anvil[facing=east]"),
        );
        let prop_pred = BlockPredicateArgumentConsumer::find_arg(&prop_pred_args, "filter")
            .unwrap()
            .unwrap();
        let (anvil_block, east_state) = BlockArgumentConsumer::find_state_arg(
            &args_with_block("minecraft:anvil[facing=east]"),
            "block",
        )
        .unwrap();
        let (_, north_state) = BlockArgumentConsumer::find_state_arg(
            &args_with_block("minecraft:anvil[facing=north]"),
            "block",
        )
        .unwrap();

        assert!(prop_pred.matches(anvil_block, east_state));
        assert!(!prop_pred.matches(anvil_block, north_state));
        assert!(!prop_pred.matches(&Block::STONE, Block::STONE.default_state.id));
    }

    fn args_with_block(block: &'static str) -> HashMap<&'static str, Arg<'static>> {
        let mut args = HashMap::new();
        args.insert("block", Arg::Block(block));
        args
    }

    #[test]
    fn property_bearing_block_argument_resolves_generated_state() {
        let mut args = HashMap::new();
        args.insert("block", Arg::Block("minecraft:anvil[facing=east]"));

        let (block, state) = BlockArgumentConsumer::find_state_arg(&args, "block").unwrap();
        assert_eq!(block.id, Block::ANVIL.id);
        assert_eq!(
            block
                .properties(state)
                .unwrap()
                .to_props()
                .into_iter()
                .find(|(name, _)| *name == "facing")
                .map(|(_, value)| value),
            Some("east")
        );
    }
}
