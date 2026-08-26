use crate::block::entities::block_entity_from_nbt;
use crate::command::CommandResult;
use crate::command::args::entity::EntityArgumentConsumer;
use crate::command::args::nbt_compound::NbtCompoundArgumentConsumer;
use crate::command::args::nbt_path::NbtPathArgumentConsumer;
use crate::command::args::nbt_tag::NbtTagArgumentConsumer;
use crate::command::args::position_block::BlockPosArgumentConsumer;
use crate::command::args::resource_location::ResourceLocationArgumentConsumer;
use crate::command::args::{FindArg, bounded_num::BoundedNumArgumentConsumer};
use crate::command::nbt_path::{NbtMutationError, NbtPath};
use crate::command::tree::builder::{NonLeafNodeBuilder, literal};
use crate::command::{
    CommandError, CommandExecutor, CommandSender,
    args::{Arg, ConsumedArgs},
    tree::{CommandTree, builder::argument},
};
use CommandError::InvalidConsumption;
use pumpkin_data::translation;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::identifier::Identifier;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::text::TextComponent;
use pumpkin_util::text::color::NamedColor;

const NAMES: [&str; 1] = ["data"];
const DESCRIPTION: &str = "Query and modify data of entities and blocks";

const ARG_ENTITY: &str = "entity";
const ARG_BLOCK: &str = "block";
const ARG_STORAGE: &str = "storage";
const ARG_PATH: &str = "path";
const ARG_SCALE: &str = "scale";
const ARG_NBT: &str = "nbt";
const ARG_TARGET_PATH: &str = "targetPath";
const ARG_VALUE: &str = "value";
const ARG_INDEX: &str = "index";
const ARG_SOURCE_ENTITY: &str = "sourceEntity";
const ARG_SOURCE_BLOCK: &str = "sourceBlock";
const ARG_SOURCE_STORAGE: &str = "sourceStorage";
const ARG_SOURCE_PATH: &str = "sourcePath";
const ARG_START: &str = "start";
const ARG_END: &str = "end";

enum DataTarget {
    Entity(TextComponent),
    Block(BlockPos),
    Storage(Identifier),
}

impl DataTarget {
    fn query_key(&self) -> &'static str {
        match self {
            Self::Entity(_) => translation::java::COMMANDS_DATA_ENTITY_QUERY,
            Self::Block(_) => translation::java::COMMANDS_DATA_BLOCK_QUERY,
            Self::Storage(_) => translation::java::COMMANDS_DATA_STORAGE_QUERY,
        }
    }

    fn get_key(&self) -> &'static str {
        match self {
            Self::Entity(_) => translation::java::COMMANDS_DATA_ENTITY_GET,
            Self::Block(_) => translation::java::COMMANDS_DATA_BLOCK_GET,
            Self::Storage(_) => translation::java::COMMANDS_DATA_STORAGE_GET,
        }
    }

    fn modified_key(&self) -> &'static str {
        match self {
            Self::Entity(_) => translation::java::COMMANDS_DATA_ENTITY_MODIFIED,
            Self::Block(_) => translation::java::COMMANDS_DATA_BLOCK_MODIFIED,
            Self::Storage(_) => translation::java::COMMANDS_DATA_STORAGE_MODIFIED,
        }
    }

    fn target_arguments(&self) -> Vec<TextComponent> {
        match self {
            Self::Entity(name) => vec![name.clone()],
            Self::Block(position) => vec![
                TextComponent::text(position.0.x.to_string()),
                TextComponent::text(position.0.y.to_string()),
                TextComponent::text(position.0.z.to_string()),
            ],
            Self::Storage(id) => vec![TextComponent::text(id.to_string())],
        }
    }
}

struct GetDataExecutor;
struct GetPathDataExecutor;
struct GetScaledDataExecutor;
struct RemoveDataExecutor;
struct MergeDataExecutor;

#[derive(Clone, Copy)]
enum ModifyOperation {
    Insert,
    Prepend,
    Append,
    Set,
    Merge,
}

struct ModifyValueExecutor(ModifyOperation);
struct ModifyFromExecutor {
    operation: ModifyOperation,
    with_path: bool,
}

#[derive(Clone, Copy)]
enum StringSlice {
    Whole,
    Start,
    StartEnd,
}

struct ModifyStringExecutor {
    operation: ModifyOperation,
    with_path: bool,
    slice: StringSlice,
}

async fn send_modified(target: &DataTarget, sender: &CommandSender) {
    sender
        .send_message(TextComponent::translate_cross(
            target.modified_key(),
            target.modified_key(),
            target.target_arguments(),
        ))
        .await;
}

fn merge_failed() -> CommandError {
    CommandError::CommandFailed(TextComponent::translate_cross(
        translation::java::COMMANDS_DATA_MERGE_FAILED,
        translation::java::COMMANDS_DATA_MERGE_FAILED,
        [],
    ))
}

fn merge_compound(target: &mut NbtCompound, source: &NbtCompound) {
    for (key, source_value) in &source.child_tags {
        if let NbtTag::Compound(source_child) = source_value
            && let Some(NbtTag::Compound(target_child)) = target.child_tags.get_mut(key.as_ref())
        {
            merge_compound(target_child, source_child);
        } else {
            target.child_tags.insert(key.clone(), source_value.clone());
        }
    }
}

fn mutation_error(error: NbtMutationError, path: &NbtPath) -> CommandError {
    let (key, arguments) = match error {
        NbtMutationError::NothingFound => (
            translation::java::ARGUMENTS_NBTPATH_NOTHING_FOUND,
            vec![TextComponent::text(path.as_str().to_owned())],
        ),
        NbtMutationError::TooDeep => (translation::java::ARGUMENTS_NBTPATH_TOO_DEEP, Vec::new()),
        NbtMutationError::ExpectedList(actual) => (
            translation::java::COMMANDS_DATA_MODIFY_EXPECTED_LIST,
            vec![TextComponent::text(actual)],
        ),
        NbtMutationError::ExpectedObject(actual) => (
            translation::java::COMMANDS_DATA_MODIFY_EXPECTED_OBJECT,
            vec![TextComponent::text(actual)],
        ),
        NbtMutationError::InvalidIndex(index) => (
            translation::java::COMMANDS_DATA_MODIFY_INVALID_INDEX,
            vec![TextComponent::text(index.to_string())],
        ),
    };
    CommandError::CommandFailed(TextComponent::translate_cross(key, key, arguments))
}

fn string_value(tag: &NbtTag) -> Result<String, CommandError> {
    match tag {
        NbtTag::Byte(_)
        | NbtTag::Short(_)
        | NbtTag::Int(_)
        | NbtTag::Long(_)
        | NbtTag::Float(_)
        | NbtTag::Double(_) => Ok(tag.to_string()),
        NbtTag::String(value) => Ok(value.to_string()),
        _ => Err(CommandError::CommandFailed(TextComponent::translate_cross(
            translation::java::COMMANDS_DATA_MODIFY_EXPECTED_VALUE,
            translation::java::COMMANDS_DATA_MODIFY_EXPECTED_VALUE,
            [TextComponent::text(tag.to_string())],
        ))),
    }
}

fn substring_utf16(value: &str, start: i32, end: Option<i32>) -> Result<String, CommandError> {
    let units: Vec<u16> = value.encode_utf16().collect();
    let length = units.len() as i64;
    let resolve = |index: i32| {
        if index < 0 {
            length + i64::from(index)
        } else {
            i64::from(index)
        }
    };
    let start = resolve(start);
    let end = end.map_or(length, resolve);
    if start < 0 || end > length || start > end {
        return Err(CommandError::CommandFailed(TextComponent::translate_cross(
            translation::java::COMMANDS_DATA_MODIFY_INVALID_SUBSTRING,
            translation::java::COMMANDS_DATA_MODIFY_INVALID_SUBSTRING,
            [
                TextComponent::text(start.to_string()),
                TextComponent::text(end.to_string()),
            ],
        )));
    }
    // Rust strings cannot represent Java's possible unpaired UTF-16 code
    // units. Valid scalar-aligned slices are lossless; split surrogate pairs
    // use the Unicode replacement scalar at this representation boundary.
    Ok(String::from_utf16_lossy(
        &units[start as usize..end as usize],
    ))
}

#[expect(clippy::too_many_lines)]
pub fn snbt_colorful_display(tag: &NbtTag, depth: usize) -> Result<TextComponent, String> {
    let folded = TextComponent::text("<...>").color_named(NamedColor::Gray);
    match tag {
        NbtTag::End => Err("Unexpected end tag".into()),
        NbtTag::Byte(value) => {
            let byte_format = TextComponent::text("b").color_named(NamedColor::Red);
            Ok(TextComponent::text(format!("{value}"))
                .color_named(NamedColor::Gold)
                .add_child(byte_format))
        }
        NbtTag::Short(value) => {
            let short_format = TextComponent::text("s").color_named(NamedColor::Red);
            Ok(TextComponent::text(format!("{value}"))
                .color_named(NamedColor::Gold)
                .add_child(short_format))
        }
        NbtTag::Int(value) => {
            Ok(TextComponent::text(format!("{value}")).color_named(NamedColor::Gold))
        }
        NbtTag::Long(value) => {
            let long_format = TextComponent::text("L").color_named(NamedColor::Red);
            Ok(TextComponent::text(format!("{value}"))
                .color_named(NamedColor::Gold)
                .add_child(long_format))
        }
        NbtTag::Float(value) => {
            let float_format = TextComponent::text("f").color_named(NamedColor::Red);
            Ok(TextComponent::text(format!("{value}"))
                .color_named(NamedColor::Gold)
                .add_child(float_format))
        }
        NbtTag::Double(value) => {
            let double_format = TextComponent::text("d").color_named(NamedColor::Red);
            Ok(TextComponent::text(format!("{value}"))
                .color_named(NamedColor::Gold)
                .add_child(double_format))
        }
        NbtTag::ByteArray(value) => {
            let byte_array_format = TextComponent::text("B").color_named(NamedColor::Red);
            let mut content = TextComponent::text("[")
                .add_child(byte_array_format.clone())
                .add_child(TextComponent::text("; "));

            for (index, byte) in value.iter().take(128).enumerate() {
                content = content
                    .add_child(TextComponent::text(format!("{byte}")))
                    .add_child(byte_array_format.clone());
                if index < value.len() - 1 {
                    content = content.add_child(TextComponent::text(", "));
                }
            }

            if value.len() > 128 {
                content = content.add_child(folded);
            }

            content = content.add_child(TextComponent::text("]"));
            Ok(content)
        }
        NbtTag::String(value) => {
            let escaped_value = value
                .replace('"', "\\\"")
                .replace('\\', "\\\\")
                .replace('\n', "\\n")
                .replace('\t', "\\t")
                .replace('\r', "\\r")
                .replace('\x0c', "\\f")
                .replace('\x08', "\\b");

            Ok(TextComponent::text(format!("\"{escaped_value}\"")).color_named(NamedColor::Green))
        }
        NbtTag::List(value) => {
            if value.is_empty() {
                Ok(TextComponent::text("[]"))
            } else if depth >= 64 {
                Ok(TextComponent::text("[")
                    .add_child(folded)
                    .add_child(TextComponent::text("]")))
            } else {
                let mut content = TextComponent::text("[");

                for (index, item) in value.iter().take(128).enumerate() {
                    let item_display = snbt_colorful_display(item, depth + 1)
                        .map_err(|string| format!("Error displaying item.[{index}]: {string}"))?;
                    content = content.add_child(item_display);

                    if index < value.len() - 1 {
                        content = content.add_child(TextComponent::text(", "));
                    }
                }

                if value.len() > 128 {
                    content = content.add_child(folded);
                }

                content = content.add_child(TextComponent::text("]"));
                Ok(content)
            }
        }
        NbtTag::Compound(value) => {
            if value.is_empty() {
                Ok(TextComponent::text("{}"))
            } else if depth >= 64 {
                Ok(TextComponent::text("{")
                    .add_child(folded)
                    .add_child(TextComponent::text("}")))
            } else {
                let mut content = TextComponent::text("{");

                for (index, (key, item)) in value.child_tags.iter().take(128).enumerate() {
                    let item_display = snbt_colorful_display(item, depth + 1)
                        .map_err(|string| format!("Error displaying item.{key}: {string}"))?;
                    content = content
                        .add_child(
                            TextComponent::text(key.to_string()).color_named(NamedColor::Aqua),
                        )
                        .add_child(TextComponent::text(": "))
                        .add_child(item_display);

                    if index < value.child_tags.len() - 1 {
                        content = content.add_child(TextComponent::text(", "));
                    }
                }

                if value.child_tags.len() > 128 {
                    content = content.add_child(folded);
                }

                content = content.add_child(TextComponent::text("}"));
                Ok(content)
            }
        }
        NbtTag::IntArray(value) => {
            let int_array_format = TextComponent::text("I").color_named(NamedColor::Red);
            let mut content = TextComponent::text("[")
                .add_child(int_array_format)
                .add_child(TextComponent::text("; "));

            for (index, int) in value.iter().take(128).enumerate() {
                content = content
                    .add_child(TextComponent::text(format!("{int}")).color_named(NamedColor::Gold));
                if index < value.len() - 1 {
                    content = content.add_child(TextComponent::text(", "));
                }
            }

            if value.len() > 128 {
                content = content.add_child(folded);
            }

            content = content.add_child(TextComponent::text("]"));
            Ok(content)
        }
        NbtTag::LongArray(value) => {
            let long_array_format = TextComponent::text("L").color_named(NamedColor::Red);
            let mut content = TextComponent::text("[")
                .add_child(long_array_format.clone())
                .add_child(TextComponent::text("; "));

            for (index, long) in value.iter().take(128).enumerate() {
                content = content
                    .add_child(TextComponent::text(format!("{long}")))
                    .add_child(long_array_format.clone());
                if index < value.len() - 1 {
                    content = content.add_child(TextComponent::text(", "));
                }
            }

            if value.len() > 128 {
                content = content.add_child(folded);
            }

            content = content.add_child(TextComponent::text("]"));
            Ok(content)
        }
    }
}

async fn send_query(
    tag: &NbtTag,
    target: &DataTarget,
    sender: &CommandSender,
) -> Result<(), CommandError> {
    let display = snbt_colorful_display(tag, 0)
        .map_err(|string| CommandError::CommandFailed(TextComponent::text(string)))?;
    let mut arguments = target.target_arguments();
    arguments.push(display);
    sender
        .send_message(TextComponent::translate_cross(
            target.query_key(),
            target.query_key(),
            arguments,
        ))
        .await;

    Ok(())
}

async fn send_scaled_result(
    path: &NbtPath,
    scale: f64,
    result: i32,
    target: &DataTarget,
    sender: &CommandSender,
) {
    let mut arguments = vec![TextComponent::text(path.as_str().to_owned())];
    arguments.extend(target.target_arguments());
    arguments.push(TextComponent::text(format!("{scale:.2}")));
    arguments.push(TextComponent::text(result.to_string()));
    sender
        .send_message(TextComponent::translate_cross(
            target.get_key(),
            target.get_key(),
            arguments,
        ))
        .await;
}

async fn target_data(
    sender: &CommandSender,
    server: &crate::server::Server,
    args: &ConsumedArgs<'_>,
) -> Result<(DataTarget, NbtTag), CommandError> {
    if let Some(Arg::Entity(entity)) = args.get(&ARG_ENTITY) {
        let mut nbt = NbtCompound::new();
        entity.write_nbt(&mut nbt).await;
        return Ok((
            DataTarget::Entity(entity.get_display_name().await),
            NbtTag::Compound(nbt),
        ));
    }
    if args.contains_key(&ARG_BLOCK) {
        let Some(world) = sender.world() else {
            return Err(InvalidConsumption(Some(ARG_BLOCK.to_owned())));
        };
        let position = BlockPosArgumentConsumer::find_loaded_arg(args, ARG_BLOCK, &world)?;
        let Some(block_entity) = world.get_block_entity(&position) else {
            return Err(CommandError::CommandFailed(TextComponent::translate_cross(
                translation::java::COMMANDS_DATA_BLOCK_INVALID,
                translation::java::COMMANDS_DATA_BLOCK_INVALID,
                [],
            )));
        };
        let mut nbt = NbtCompound::new();
        block_entity.write_internal(&mut nbt).await;
        return Ok((DataTarget::Block(position), NbtTag::Compound(nbt)));
    }
    if let Some(Arg::ResourceLocation(raw_id)) = args.get(&ARG_STORAGE) {
        let id = Identifier::parse(raw_id)
            .map_err(|error| CommandError::CommandFailed(TextComponent::text(error.to_string())))?;
        let value = server.command_storage.get(&id).await;
        return Ok((DataTarget::Storage(id), NbtTag::Compound(value)));
    }
    Err(InvalidConsumption(None))
}

async fn source_values(
    sender: &CommandSender,
    server: &crate::server::Server,
    args: &ConsumedArgs<'_>,
    with_path: bool,
) -> Result<Vec<NbtTag>, CommandError> {
    let root = if let Some(Arg::Entity(entity)) = args.get(&ARG_SOURCE_ENTITY) {
        let mut nbt = NbtCompound::new();
        entity.write_nbt(&mut nbt).await;
        NbtTag::Compound(nbt)
    } else if args.contains_key(&ARG_SOURCE_BLOCK) {
        let Some(world) = sender.world() else {
            return Err(InvalidConsumption(Some(ARG_SOURCE_BLOCK.to_owned())));
        };
        let position = BlockPosArgumentConsumer::find_loaded_arg(args, ARG_SOURCE_BLOCK, &world)?;
        let Some(block_entity) = world.get_block_entity(&position) else {
            return Err(CommandError::CommandFailed(TextComponent::translate_cross(
                translation::java::COMMANDS_DATA_BLOCK_INVALID,
                translation::java::COMMANDS_DATA_BLOCK_INVALID,
                [],
            )));
        };
        let mut nbt = NbtCompound::new();
        block_entity.write_internal(&mut nbt).await;
        NbtTag::Compound(nbt)
    } else if let Some(Arg::ResourceLocation(raw_id)) = args.get(&ARG_SOURCE_STORAGE) {
        let id = Identifier::parse(raw_id)
            .map_err(|error| CommandError::CommandFailed(TextComponent::text(error.to_string())))?;
        NbtTag::Compound(server.command_storage.get(&id).await)
    } else {
        return Err(InvalidConsumption(None));
    };

    if !with_path {
        return Ok(vec![root]);
    }
    let path = NbtPathArgumentConsumer::find_arg(args, ARG_SOURCE_PATH)?;
    let values = path.get(&root);
    if values.is_empty() {
        Err(mutation_error(NbtMutationError::NothingFound, path))
    } else {
        Ok(values)
    }
}

async fn mutate_target<F>(
    sender: &CommandSender,
    server: &crate::server::Server,
    args: &ConsumedArgs<'_>,
    mutate: F,
) -> Result<(DataTarget, usize), CommandError>
where
    F: Fn(&mut NbtTag) -> Result<usize, CommandError>,
{
    if let Some(Arg::Entity(entity)) = args.get(&ARG_ENTITY) {
        if entity.get_player().is_some() {
            return Err(CommandError::CommandFailed(TextComponent::translate_cross(
                translation::java::COMMANDS_DATA_ENTITY_INVALID,
                translation::java::COMMANDS_DATA_ENTITY_INVALID,
                [],
            )));
        }
        let target = DataTarget::Entity(entity.get_display_name().await);
        let mut nbt = NbtCompound::new();
        entity.write_nbt(&mut nbt).await;
        let mut root = NbtTag::Compound(nbt);
        let changed = mutate(&mut root)?;
        let NbtTag::Compound(nbt) = root else {
            unreachable!()
        };
        entity.read_nbt_non_mut(&nbt).await;
        return Ok((target, changed));
    }
    if args.contains_key(&ARG_BLOCK) {
        let Some(world) = sender.world() else {
            return Err(InvalidConsumption(Some(ARG_BLOCK.to_owned())));
        };
        let position = BlockPosArgumentConsumer::find_loaded_arg(args, ARG_BLOCK, &world)?;
        let Some(block_entity) = world.get_block_entity(&position) else {
            return Err(CommandError::CommandFailed(TextComponent::translate_cross(
                translation::java::COMMANDS_DATA_BLOCK_INVALID,
                translation::java::COMMANDS_DATA_BLOCK_INVALID,
                [],
            )));
        };
        let mut nbt = NbtCompound::new();
        block_entity.write_internal(&mut nbt).await;
        let id = nbt.get_string("id").map(str::to_owned);
        let mut root = NbtTag::Compound(nbt);
        let changed = mutate(&mut root)?;
        let NbtTag::Compound(mut nbt) = root else {
            unreachable!()
        };
        if let Some(id) = id {
            nbt.put_string("id", id);
        }
        nbt.put_int("x", position.0.x);
        nbt.put_int("y", position.0.y);
        nbt.put_int("z", position.0.z);
        let Some(updated) = block_entity_from_nbt(&nbt) else {
            return Err(CommandError::CommandFailed(TextComponent::translate_cross(
                translation::java::COMMANDS_DATA_BLOCK_INVALID,
                translation::java::COMMANDS_DATA_BLOCK_INVALID,
                [],
            )));
        };
        world.add_block_entity(updated);
        return Ok((DataTarget::Block(position), changed));
    }
    if let Some(Arg::ResourceLocation(raw_id)) = args.get(&ARG_STORAGE) {
        let id = Identifier::parse(raw_id)
            .map_err(|error| CommandError::CommandFailed(TextComponent::text(error.to_string())))?;
        let mut root = NbtTag::Compound(server.command_storage.get(&id).await);
        let changed = mutate(&mut root)?;
        let NbtTag::Compound(nbt) = root else {
            unreachable!()
        };
        server.command_storage.set(&id, nbt).await;
        return Ok((DataTarget::Storage(id), changed));
    }
    Err(InvalidConsumption(None))
}

fn get_single_path_tag(path: &NbtPath, root: &NbtTag) -> Result<NbtTag, CommandError> {
    match path.get(root).as_slice() {
        [] => Err(CommandError::CommandFailed(TextComponent::translate_cross(
            translation::java::ARGUMENTS_NBTPATH_NOTHING_FOUND,
            translation::java::ARGUMENTS_NBTPATH_NOTHING_FOUND,
            [TextComponent::text(path.as_str().to_owned())],
        ))),
        [tag] => Ok(tag.clone()),
        _ => Err(CommandError::CommandFailed(TextComponent::translate_cross(
            translation::java::COMMANDS_DATA_GET_MULTIPLE,
            translation::java::COMMANDS_DATA_GET_MULTIPLE,
            [],
        ))),
    }
}

fn numeric_value(tag: &NbtTag) -> Option<f64> {
    match tag {
        NbtTag::Byte(value) => Some(f64::from(*value)),
        NbtTag::Short(value) => Some(f64::from(*value)),
        NbtTag::Int(value) => Some(f64::from(*value)),
        NbtTag::Long(value) => Some(*value as f64),
        NbtTag::Float(value) => Some(f64::from(*value)),
        NbtTag::Double(value) => Some(*value),
        _ => None,
    }
}

fn floor_to_i32(value: f64) -> i32 {
    let truncated = value as i32;
    if value < f64::from(truncated) {
        truncated.wrapping_sub(1)
    } else {
        truncated
    }
}

impl CommandExecutor for GetDataExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        server: &'a crate::server::Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let (target, root) = target_data(sender, server, args).await?;
            send_query(&root, &target, sender).await?;
            Ok(1)
        })
    }
}

impl CommandExecutor for GetPathDataExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        server: &'a crate::server::Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let path = NbtPathArgumentConsumer::find_arg(args, ARG_PATH)?;
            let (target, root) = target_data(sender, server, args).await?;
            let selected = get_single_path_tag(path, &root)?;
            let result = get_i32_result(&selected)?;
            send_query(&selected, &target, sender).await?;
            Ok(result)
        })
    }
}

impl CommandExecutor for GetScaledDataExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        server: &'a crate::server::Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let path = NbtPathArgumentConsumer::find_arg(args, ARG_PATH)?;
            let scale = BoundedNumArgumentConsumer::<f64>::find_arg(args, ARG_SCALE)??;
            let (target, root) = target_data(sender, server, args).await?;
            let selected = get_single_path_tag(path, &root)?;
            let Some(value) = numeric_value(&selected) else {
                return Err(CommandError::CommandFailed(TextComponent::translate_cross(
                    translation::java::COMMANDS_DATA_GET_INVALID,
                    translation::java::COMMANDS_DATA_GET_INVALID,
                    [TextComponent::text(path.as_str().to_owned())],
                )));
            };
            let result = floor_to_i32(value * scale);
            send_scaled_result(path, scale, result, &target, sender).await;
            Ok(result)
        })
    }
}

impl CommandExecutor for RemoveDataExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        server: &'a crate::server::Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let path = NbtPathArgumentConsumer::find_arg(args, ARG_PATH)?;

            let (target, changed) = if let Some(Arg::Entity(entity)) = args.get(&ARG_ENTITY) {
                if entity.get_player().is_some() {
                    return Err(CommandError::CommandFailed(TextComponent::translate_cross(
                        translation::java::COMMANDS_DATA_ENTITY_INVALID,
                        translation::java::COMMANDS_DATA_ENTITY_INVALID,
                        [],
                    )));
                }
                let target = DataTarget::Entity(entity.get_display_name().await);
                let mut root = NbtTag::Compound(NbtCompound::new());
                let NbtTag::Compound(nbt) = &mut root else {
                    unreachable!()
                };
                entity.write_nbt(nbt).await;
                let changed = path.remove(&mut root);
                if changed == 0 {
                    return Err(merge_failed());
                }
                let NbtTag::Compound(nbt) = root else {
                    unreachable!()
                };
                entity.read_nbt_non_mut(&nbt).await;
                (target, changed)
            } else if args.contains_key(&ARG_BLOCK) {
                let Some(world) = sender.world() else {
                    return Err(InvalidConsumption(Some(ARG_BLOCK.to_owned())));
                };
                let position = BlockPosArgumentConsumer::find_loaded_arg(args, ARG_BLOCK, &world)?;
                let Some(block_entity) = world.get_block_entity(&position) else {
                    return Err(CommandError::CommandFailed(TextComponent::translate_cross(
                        translation::java::COMMANDS_DATA_BLOCK_INVALID,
                        translation::java::COMMANDS_DATA_BLOCK_INVALID,
                        [],
                    )));
                };
                let mut nbt = NbtCompound::new();
                block_entity.write_internal(&mut nbt).await;
                let id = nbt.get_string("id").map(str::to_owned);
                let mut root = NbtTag::Compound(nbt);
                let changed = path.remove(&mut root);
                if changed == 0 {
                    return Err(merge_failed());
                }
                let NbtTag::Compound(mut nbt) = root else {
                    unreachable!()
                };
                // Pumpkin rebuilds block entities from serialized NBT. Vanilla's
                // accessor keeps the existing type and position out-of-band.
                if let Some(id) = id {
                    nbt.put_string("id", id);
                }
                nbt.put_int("x", position.0.x);
                nbt.put_int("y", position.0.y);
                nbt.put_int("z", position.0.z);
                let Some(updated) = block_entity_from_nbt(&nbt) else {
                    return Err(CommandError::CommandFailed(TextComponent::translate_cross(
                        translation::java::COMMANDS_DATA_BLOCK_INVALID,
                        translation::java::COMMANDS_DATA_BLOCK_INVALID,
                        [],
                    )));
                };
                world.add_block_entity(updated);
                (DataTarget::Block(position), changed)
            } else if let Some(Arg::ResourceLocation(raw_id)) = args.get(&ARG_STORAGE) {
                let id = Identifier::parse(raw_id).map_err(|error| {
                    CommandError::CommandFailed(TextComponent::text(error.to_string()))
                })?;
                let mut root = NbtTag::Compound(server.command_storage.get(&id).await);
                let changed = path.remove(&mut root);
                if changed == 0 {
                    return Err(merge_failed());
                }
                let NbtTag::Compound(nbt) = root else {
                    unreachable!()
                };
                server.command_storage.set(&id, nbt).await;
                (DataTarget::Storage(id), changed)
            } else {
                return Err(InvalidConsumption(None));
            };

            send_modified(&target, sender).await;
            Ok(changed as i32)
        })
    }
}

impl CommandExecutor for MergeDataExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        server: &'a crate::server::Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let source = NbtCompoundArgumentConsumer::find_arg(args, ARG_NBT)?;
            let (target, changed) = mutate_target(sender, server, args, |root| {
                let NbtTag::Compound(target) = root else {
                    unreachable!()
                };
                let original = target.clone();
                merge_compound(target, source);
                if *target == original {
                    Err(merge_failed())
                } else {
                    Ok(1)
                }
            })
            .await?;
            send_modified(&target, sender).await;
            Ok(changed as i32)
        })
    }
}

impl CommandExecutor for ModifyValueExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        server: &'a crate::server::Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let path = NbtPathArgumentConsumer::find_arg(args, ARG_TARGET_PATH)?;
            let value = NbtTagArgumentConsumer::find_arg(args, ARG_VALUE)?.clone();
            let index = match self.0 {
                ModifyOperation::Insert => {
                    BoundedNumArgumentConsumer::<i32>::find_arg(args, ARG_INDEX)??
                }
                ModifyOperation::Prepend => 0,
                ModifyOperation::Append => -1,
                ModifyOperation::Set => 0,
                ModifyOperation::Merge => 0,
            };
            let (target, changed) = mutate_target(sender, server, args, |root| {
                let changed = match self.0 {
                    ModifyOperation::Set => path.set(root, value.clone()),
                    ModifyOperation::Merge => path.merge(root, std::slice::from_ref(&value)),
                    ModifyOperation::Insert
                    | ModifyOperation::Prepend
                    | ModifyOperation::Append => {
                        path.insert(index, root, std::slice::from_ref(&value))
                    }
                }
                .map_err(|error| mutation_error(error, path))?;
                if changed == 0 {
                    Err(merge_failed())
                } else {
                    Ok(changed)
                }
            })
            .await?;
            send_modified(&target, sender).await;
            Ok(changed as i32)
        })
    }
}

impl CommandExecutor for ModifyFromExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        server: &'a crate::server::Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let path = NbtPathArgumentConsumer::find_arg(args, ARG_TARGET_PATH)?;
            let values = source_values(sender, server, args, self.with_path).await?;
            let index = match self.operation {
                ModifyOperation::Insert => {
                    BoundedNumArgumentConsumer::<i32>::find_arg(args, ARG_INDEX)??
                }
                ModifyOperation::Prepend => 0,
                ModifyOperation::Append => -1,
                ModifyOperation::Set => 0,
                ModifyOperation::Merge => 0,
            };
            let (target, changed) = mutate_target(sender, server, args, |root| {
                let changed = match self.operation {
                    ModifyOperation::Set => path.set(
                        root,
                        values
                            .last()
                            .expect("Vanilla source paths are non-empty")
                            .clone(),
                    ),
                    ModifyOperation::Merge => path.merge(root, &values),
                    ModifyOperation::Insert
                    | ModifyOperation::Prepend
                    | ModifyOperation::Append => path.insert(index, root, &values),
                }
                .map_err(|error| mutation_error(error, path))?;
                if changed == 0 {
                    Err(merge_failed())
                } else {
                    Ok(changed)
                }
            })
            .await?;
            send_modified(&target, sender).await;
            Ok(changed as i32)
        })
    }
}

impl CommandExecutor for ModifyStringExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        server: &'a crate::server::Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let path = NbtPathArgumentConsumer::find_arg(args, ARG_TARGET_PATH)?;
            let raw_values = source_values(sender, server, args, self.with_path).await?;
            let start =
                match self.slice {
                    StringSlice::Whole => None,
                    StringSlice::Start | StringSlice::StartEnd => Some(
                        BoundedNumArgumentConsumer::<i32>::find_arg(args, ARG_START)??,
                    ),
                };
            let end = match self.slice {
                StringSlice::StartEnd => {
                    Some(BoundedNumArgumentConsumer::<i32>::find_arg(args, ARG_END)??)
                }
                StringSlice::Whole | StringSlice::Start => None,
            };
            let values: Vec<NbtTag> = raw_values
                .iter()
                .map(|tag| {
                    let value = string_value(tag)?;
                    let value = start.map_or(Ok(value.clone()), |start| {
                        substring_utf16(&value, start, end)
                    })?;
                    Ok(NbtTag::String(value.into()))
                })
                .collect::<Result<_, CommandError>>()?;
            let index = match self.operation {
                ModifyOperation::Insert => {
                    BoundedNumArgumentConsumer::<i32>::find_arg(args, ARG_INDEX)??
                }
                ModifyOperation::Prepend => 0,
                ModifyOperation::Append => -1,
                ModifyOperation::Set => 0,
                ModifyOperation::Merge => 0,
            };
            let (target, changed) = mutate_target(sender, server, args, |root| {
                let changed = match self.operation {
                    ModifyOperation::Set => path.set(
                        root,
                        values.last().expect("string sources are non-empty").clone(),
                    ),
                    ModifyOperation::Merge => path.merge(root, &values),
                    ModifyOperation::Insert
                    | ModifyOperation::Prepend
                    | ModifyOperation::Append => path.insert(index, root, &values),
                }
                .map_err(|error| mutation_error(error, path))?;
                if changed == 0 {
                    Err(merge_failed())
                } else {
                    Ok(changed)
                }
            })
            .await?;
            send_modified(&target, sender).await;
            Ok(changed as i32)
        })
    }
}

fn get_i32_result(tag: &NbtTag) -> Result<i32, CommandError> {
    if let Some(value) = numeric_value(tag) {
        return Ok(floor_to_i32(value));
    }

    match tag {
        NbtTag::End => Err(CommandError::CommandFailed(TextComponent::translate_cross(
            translation::java::COMMANDS_DATA_GET_UNKNOWN,
            translation::java::COMMANDS_DATA_GET_UNKNOWN,
            [],
        ))),

        NbtTag::Byte(_)
        | NbtTag::Short(_)
        | NbtTag::Int(_)
        | NbtTag::Long(_)
        | NbtTag::Float(_)
        | NbtTag::Double(_) => unreachable!(),

        NbtTag::ByteArray(items) => Ok(items.len() as i32),
        NbtTag::IntArray(items) => Ok(items.len() as i32),
        NbtTag::LongArray(items) => Ok(items.len() as i32),

        NbtTag::String(string) => Ok(string.encode_utf16().count() as i32),
        NbtTag::List(nbt_tags) => Ok(nbt_tags.len() as i32),
        NbtTag::Compound(nbt_compound) => Ok(nbt_compound.child_tags.len() as i32),
    }
}

fn from_sources(operation: ModifyOperation) -> NonLeafNodeBuilder {
    literal("from")
        .then(
            literal("entity").then(
                argument(ARG_SOURCE_ENTITY, EntityArgumentConsumer)
                    .execute(ModifyFromExecutor {
                        operation,
                        with_path: false,
                    })
                    .then(argument(ARG_SOURCE_PATH, NbtPathArgumentConsumer).execute(
                        ModifyFromExecutor {
                            operation,
                            with_path: true,
                        },
                    )),
            ),
        )
        .then(
            literal("block").then(
                argument(ARG_SOURCE_BLOCK, BlockPosArgumentConsumer)
                    .execute(ModifyFromExecutor {
                        operation,
                        with_path: false,
                    })
                    .then(argument(ARG_SOURCE_PATH, NbtPathArgumentConsumer).execute(
                        ModifyFromExecutor {
                            operation,
                            with_path: true,
                        },
                    )),
            ),
        )
        .then(
            literal("storage").then(
                argument(ARG_SOURCE_STORAGE, ResourceLocationArgumentConsumer)
                    .execute(ModifyFromExecutor {
                        operation,
                        with_path: false,
                    })
                    .then(argument(ARG_SOURCE_PATH, NbtPathArgumentConsumer).execute(
                        ModifyFromExecutor {
                            operation,
                            with_path: true,
                        },
                    )),
            ),
        )
}

fn string_path(operation: ModifyOperation) -> NonLeafNodeBuilder {
    argument(ARG_SOURCE_PATH, NbtPathArgumentConsumer)
        .execute(ModifyStringExecutor {
            operation,
            with_path: true,
            slice: StringSlice::Whole,
        })
        .then(
            argument(
                ARG_START,
                BoundedNumArgumentConsumer::<i32>::new().name(ARG_START),
            )
            .execute(ModifyStringExecutor {
                operation,
                with_path: true,
                slice: StringSlice::Start,
            })
            .then(
                argument(
                    ARG_END,
                    BoundedNumArgumentConsumer::<i32>::new().name(ARG_END),
                )
                .execute(ModifyStringExecutor {
                    operation,
                    with_path: true,
                    slice: StringSlice::StartEnd,
                }),
            ),
        )
}

fn string_sources(operation: ModifyOperation) -> NonLeafNodeBuilder {
    literal("string")
        .then(
            literal("entity").then(
                argument(ARG_SOURCE_ENTITY, EntityArgumentConsumer)
                    .execute(ModifyStringExecutor {
                        operation,
                        with_path: false,
                        slice: StringSlice::Whole,
                    })
                    .then(string_path(operation)),
            ),
        )
        .then(
            literal("block").then(
                argument(ARG_SOURCE_BLOCK, BlockPosArgumentConsumer)
                    .execute(ModifyStringExecutor {
                        operation,
                        with_path: false,
                        slice: StringSlice::Whole,
                    })
                    .then(string_path(operation)),
            ),
        )
        .then(
            literal("storage").then(
                argument(ARG_SOURCE_STORAGE, ResourceLocationArgumentConsumer)
                    .execute(ModifyStringExecutor {
                        operation,
                        with_path: false,
                        slice: StringSlice::Whole,
                    })
                    .then(string_path(operation)),
            ),
        )
}

fn modify_operation(name: &'static str, operation: ModifyOperation) -> NonLeafNodeBuilder {
    literal(name)
        .then(literal("value").then(
            argument(ARG_VALUE, NbtTagArgumentConsumer).execute(ModifyValueExecutor(operation)),
        ))
        .then(from_sources(operation))
        .then(string_sources(operation))
}

fn modify_operations() -> NonLeafNodeBuilder {
    argument(ARG_TARGET_PATH, NbtPathArgumentConsumer)
        .then(
            literal("insert").then(
                argument(
                    ARG_INDEX,
                    BoundedNumArgumentConsumer::<i32>::new().name(ARG_INDEX),
                )
                .then(
                    literal("value").then(
                        argument(ARG_VALUE, NbtTagArgumentConsumer)
                            .execute(ModifyValueExecutor(ModifyOperation::Insert)),
                    ),
                )
                .then(from_sources(ModifyOperation::Insert))
                .then(string_sources(ModifyOperation::Insert)),
            ),
        )
        .then(modify_operation("prepend", ModifyOperation::Prepend))
        .then(modify_operation("append", ModifyOperation::Append))
        .then(modify_operation("set", ModifyOperation::Set))
        .then(modify_operation("merge", ModifyOperation::Merge))
}

pub fn init_command_tree() -> CommandTree {
    CommandTree::new(NAMES, DESCRIPTION)
        .then(
            literal("get")
                .then(
                    literal("entity").then(
                        argument(ARG_ENTITY, EntityArgumentConsumer)
                            .execute(GetDataExecutor)
                            .then(
                                argument(ARG_PATH, NbtPathArgumentConsumer)
                                    .execute(GetPathDataExecutor)
                                    .then(
                                        argument(
                                            ARG_SCALE,
                                            BoundedNumArgumentConsumer::<f64>::new()
                                                .name(ARG_SCALE),
                                        )
                                        .execute(GetScaledDataExecutor),
                                    ),
                            ),
                    ),
                )
                .then(
                    literal("block").then(
                        argument(ARG_BLOCK, BlockPosArgumentConsumer)
                            .execute(GetDataExecutor)
                            .then(
                                argument(ARG_PATH, NbtPathArgumentConsumer)
                                    .execute(GetPathDataExecutor)
                                    .then(
                                        argument(
                                            ARG_SCALE,
                                            BoundedNumArgumentConsumer::<f64>::new()
                                                .name(ARG_SCALE),
                                        )
                                        .execute(GetScaledDataExecutor),
                                    ),
                            ),
                    ),
                )
                .then(
                    literal("storage").then(
                        argument(ARG_STORAGE, ResourceLocationArgumentConsumer)
                            .execute(GetDataExecutor)
                            .then(
                                argument(ARG_PATH, NbtPathArgumentConsumer)
                                    .execute(GetPathDataExecutor)
                                    .then(
                                        argument(
                                            ARG_SCALE,
                                            BoundedNumArgumentConsumer::<f64>::new()
                                                .name(ARG_SCALE),
                                        )
                                        .execute(GetScaledDataExecutor),
                                    ),
                            ),
                    ),
                ),
        )
        .then(
            literal("remove")
                .then(literal("entity").then(
                    argument(ARG_ENTITY, EntityArgumentConsumer).then(
                        argument(ARG_PATH, NbtPathArgumentConsumer).execute(RemoveDataExecutor),
                    ),
                ))
                .then(literal("block").then(
                    argument(ARG_BLOCK, BlockPosArgumentConsumer).then(
                        argument(ARG_PATH, NbtPathArgumentConsumer).execute(RemoveDataExecutor),
                    ),
                ))
                .then(literal("storage").then(
                    argument(ARG_STORAGE, ResourceLocationArgumentConsumer).then(
                        argument(ARG_PATH, NbtPathArgumentConsumer).execute(RemoveDataExecutor),
                    ),
                )),
        )
        .then(
            literal("merge")
                .then(
                    literal("entity").then(argument(ARG_ENTITY, EntityArgumentConsumer).then(
                        argument(ARG_NBT, NbtCompoundArgumentConsumer).execute(MergeDataExecutor),
                    )),
                )
                .then(
                    literal("block").then(argument(ARG_BLOCK, BlockPosArgumentConsumer).then(
                        argument(ARG_NBT, NbtCompoundArgumentConsumer).execute(MergeDataExecutor),
                    )),
                )
                .then(literal("storage").then(
                    argument(ARG_STORAGE, ResourceLocationArgumentConsumer).then(
                        argument(ARG_NBT, NbtCompoundArgumentConsumer).execute(MergeDataExecutor),
                    ),
                )),
        )
        .then(
            literal("modify")
                .then(
                    literal("entity").then(
                        argument(ARG_ENTITY, EntityArgumentConsumer).then(modify_operations()),
                    ),
                )
                .then(
                    literal("block").then(
                        argument(ARG_BLOCK, BlockPosArgumentConsumer).then(modify_operations()),
                    ),
                )
                .then(
                    literal("storage").then(
                        argument(ARG_STORAGE, ResourceLocationArgumentConsumer)
                            .then(modify_operations()),
                    ),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::tree::NodeType;

    fn command_paths() -> Vec<Vec<String>> {
        let tree = init_command_tree();
        tree.iter_paths()
            .map(|path| {
                path.into_iter()
                    .filter_map(|index| match &tree.nodes[index].node_type {
                        NodeType::Literal { string } => Some(string.clone()),
                        NodeType::Argument { name, .. } => Some(format!("<{name}>")),
                        NodeType::ExecuteLeaf { .. } | NodeType::Require { .. } => None,
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn path_query_return_values_match_vanilla_types() {
        assert_eq!(get_i32_result(&NbtTag::Double(-1.2)).unwrap(), -2);
        assert_eq!(get_i32_result(&NbtTag::Float(2.9)).unwrap(), 2);
        assert_eq!(get_i32_result(&NbtTag::String("é😀".into())).unwrap(), 3);
        assert_eq!(
            get_i32_result(&NbtTag::List(vec![NbtTag::Int(1), NbtTag::Int(2)])).unwrap(),
            2
        );
        let mut compound = NbtCompound::new();
        compound.put_int("a", 1);
        compound.put_int("b", 2);
        assert_eq!(get_i32_result(&NbtTag::Compound(compound)).unwrap(), 2);
    }

    #[test]
    fn scaled_numeric_query_floors_after_multiplication() {
        assert_eq!(floor_to_i32(-1.2 * 2.0), -3);
        assert_eq!(floor_to_i32(1.9 * 2.0), 3);
        assert_eq!(floor_to_i32(f64::INFINITY), i32::MAX);
        assert_eq!(floor_to_i32(f64::NEG_INFINITY), i32::MAX);
    }

    #[test]
    fn path_query_requires_exactly_one_selected_tag() {
        let mut root = NbtCompound::new();
        root.put("values", NbtTag::List(vec![NbtTag::Int(4), NbtTag::Int(7)]));
        let root = NbtTag::Compound(root);
        let one = NbtPath::parse("values[0]").unwrap();
        assert_eq!(get_single_path_tag(&one, &root).unwrap(), NbtTag::Int(4));
        let many = NbtPath::parse("values[]").unwrap();
        assert!(get_single_path_tag(&many, &root).is_err());
        let missing = NbtPath::parse("missing").unwrap();
        assert!(get_single_path_tag(&missing, &root).is_err());
    }

    #[test]
    fn compound_merge_recurses_and_replaces_non_compounds() {
        let mut nested = NbtCompound::new();
        nested.put_int("kept", 1);
        nested.put_int("changed", 2);
        let mut target = NbtCompound::new();
        target.put_compound("nested", nested);
        target.put_string("replaced", "old".to_owned());

        let mut source_nested = NbtCompound::new();
        source_nested.put_int("changed", 3);
        source_nested.put_int("added", 4);
        let mut source = NbtCompound::new();
        source.put_compound("nested", source_nested);
        source.put_int("replaced", 5);

        merge_compound(&mut target, &source);
        let nested = target.get_compound("nested").unwrap();
        assert_eq!(nested.get_int("kept"), Some(1));
        assert_eq!(nested.get_int("changed"), Some(3));
        assert_eq!(nested.get_int("added"), Some(4));
        assert_eq!(target.get_int("replaced"), Some(5));

        let unchanged = target.clone();
        merge_compound(&mut target, &unchanged);
        assert_eq!(target, unchanged);
    }

    #[test]
    fn modify_value_and_from_grammar_covers_all_provider_pairs() {
        let paths = command_paths();
        let modify: Vec<_> = paths
            .iter()
            .filter(|path| path.first().is_some_and(|part| part == "modify"))
            .collect();
        assert_eq!(modify.len(), 285);
        for expected in [
            vec![
                "modify",
                "entity",
                "<entity>",
                "<targetPath>",
                "set",
                "from",
                "storage",
                "<sourceStorage>",
                "<sourcePath>",
            ],
            vec![
                "modify",
                "block",
                "<block>",
                "<targetPath>",
                "insert",
                "<index>",
                "from",
                "entity",
                "<sourceEntity>",
            ],
            vec![
                "modify",
                "storage",
                "<storage>",
                "<targetPath>",
                "append",
                "value",
                "<value>",
            ],
            vec![
                "modify",
                "entity",
                "<entity>",
                "<targetPath>",
                "prepend",
                "string",
                "block",
                "<sourceBlock>",
                "<sourcePath>",
                "<start>",
                "<end>",
            ],
            vec![
                "modify",
                "storage",
                "<storage>",
                "<targetPath>",
                "merge",
                "from",
                "block",
                "<sourceBlock>",
                "<sourcePath>",
            ],
        ] {
            assert!(
                paths.iter().any(|path| path == &expected),
                "missing command path: {expected:?}"
            );
        }
    }

    #[test]
    fn string_source_uses_java_utf16_offsets_and_negative_indices() {
        assert_eq!(substring_utf16("a😀z", 1, Some(3)).unwrap(), "😀");
        assert_eq!(substring_utf16("a😀z", -3, Some(-1)).unwrap(), "😀");
        assert_eq!(substring_utf16("abcdef", -3, None).unwrap(), "def");
        assert!(substring_utf16("abc", -4, None).is_err());
        assert!(substring_utf16("abc", 2, Some(1)).is_err());
        assert!(string_value(&NbtTag::List(Vec::new())).is_err());
        assert_eq!(string_value(&NbtTag::String("raw".into())).unwrap(), "raw");
    }
}
