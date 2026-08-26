use std::sync::Arc;

use crate::command::args::entities::parse_target_selector;
use crate::command::argument_types::argument_type::ArgumentType;
use crate::command::argument_types::coordinates::block_pos::BlockPosArgumentType;
use crate::command::context::command_source::CommandSource;
use crate::command::nbt_path::NbtPath;
use crate::command::string_reader::StringReader;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use pumpkin_data::data_component_impl::WrittenBookContentImpl;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::identifier::Identifier;
use pumpkin_util::math::position::BlockPos;

/// Maximum allowed length of a resolved page component in characters,
/// matching Vanilla 1.21.4 `WrittenBookContent.PAGE_LENGTH_LIMIT` (32767).
pub const MAX_PAGE_LENGTH: usize = 32767;

/// Maximum recursion depth for nested component resolution, matching
/// Vanilla 1.21.4 `ComponentUtils.MAX_DEPTH` (100).
pub const MAX_RECURSION_DEPTH: usize = 100;

/// Resolves written-book components (selectors, scoreboard scores, translations,
/// extra siblings, and hover events) against a `CommandSource`, matching
/// Vanilla 1.21.4 `WrittenBookItem.resolveBookComponents(ItemStack, CommandSourceStack, Player)`.
///
/// Returns `true` if dynamic components were resolved and modified, or `false`
/// if already resolved, not a written book, or if resolution failed (in which case
/// the book is marked resolved to avoid repeated failing attempts while preserving
/// all content intact).
pub async fn resolve_book_components(
    stack: &mut ItemStack,
    source: &CommandSource,
    player: Option<&Arc<Player>>,
) -> bool {
    if stack.item.id != Item::WRITTEN_BOOK.id {
        return false;
    }

    let Some(content) = stack.get_data_component::<WrittenBookContentImpl>() else {
        return false;
    };

    if content.resolved {
        return false;
    }

    let mut resolved_pages = Vec::with_capacity(content.pages.len());
    let mut resolved_filtered_pages = Vec::with_capacity(content.filtered_pages.len());

    let mut all_succeeded = true;

    for (index, raw_page) in content.pages.iter().enumerate() {
        let Some(resolved_raw) = resolve_page_tag(raw_page, source, player, 0).await else {
            all_succeeded = false;
            break;
        };
        resolved_pages.push(resolved_raw);

        if let Some(Some(filtered_page)) = content.filtered_pages.get(index) {
            let Some(resolved_filtered) = resolve_page_tag(filtered_page, source, player, 0).await
            else {
                all_succeeded = false;
                break;
            };
            resolved_filtered_pages.push(Some(resolved_filtered));
        } else {
            resolved_filtered_pages.push(None);
        }
    }

    if all_succeeded {
        let updated_content = WrittenBookContentImpl {
            title: content.title.clone(),
            filtered_title: content.filtered_title.clone(),
            author: content.author.clone(),
            generation: content.generation,
            pages: resolved_pages,
            filtered_pages: resolved_filtered_pages,
            resolved: true,
        };
        stack.set_data_component(updated_content);
        true
    } else {
        // Vanilla fallback: mark resolved = true without changing pages to prevent
        // infinite re-evaluations while safely preserving user data.
        let mut fallback_content = content.clone();
        fallback_content.resolved = true;
        stack.set_data_component(fallback_content);
        false
    }
}

/// Recursively resolves a single page NbtTag against the command source and player.
pub async fn resolve_page_tag(
    tag: &NbtTag,
    source: &CommandSource,
    player: Option<&Arc<Player>>,
    depth: usize,
) -> Option<NbtTag> {
    if depth > MAX_RECURSION_DEPTH {
        return Some(tag.clone());
    }

    match tag {
        NbtTag::String(s) => {
            // In the 1.21.4 component codec an NBT string is already a literal
            // component. JSON-looking text must remain literal rather than be
            // parsed a second time. Java's limit uses UTF-16 code units, not
            // UTF-8 bytes.
            if java_string_len(s) > MAX_PAGE_LENGTH {
                None
            } else {
                Some(tag.clone())
            }
        }
        NbtTag::Compound(compound) => {
            let resolved = resolve_compound(compound, source, player, depth).await?;
            let res_tag = NbtTag::Compound(resolved);
            let json_str = nbt_tag_to_json_string(&res_tag);
            if java_string_len(&json_str) > MAX_PAGE_LENGTH {
                None
            } else {
                Some(res_tag)
            }
        }
        _ => Some(tag.clone()),
    }
}

/// Recursively resolves dynamic fields inside an NbtCompound text component.
async fn resolve_compound(
    compound: &NbtCompound,
    source: &CommandSource,
    player: Option<&Arc<Player>>,
    depth: usize,
) -> Option<NbtCompound> {
    if depth > MAX_RECURSION_DEPTH {
        return Some(compound.clone());
    }

    let mut result = compound.clone();

    // NBT content backed by an entity selector or block position. Invalid
    // paths/sources and unavailable context produce the empty component,
    // matching the empty stream used by Vanilla's data sources.
    if let Some(path_text) = compound.get_string("nbt")
        && (compound.get_string("entity").is_some()
            || compound.get_string("block").is_some()
            || compound.get_string("storage").is_some())
    {
        result.child_tags.retain(|key, _| &**key != "extra");
        let mut selected_values = Vec::new();
        if let Ok(path) = NbtPath::parse(path_text) {
            if let Some(entity_selector) = compound.get_string("entity") {
                if let (Ok(selector), Some(server)) = (
                    parse_target_selector(entity_selector),
                    source.server.as_ref(),
                ) {
                    for entity in server
                        .select_entities(&selector, Some(&source.output))
                        .await
                    {
                        let mut entity_nbt = NbtCompound::new();
                        entity.write_nbt(&mut entity_nbt).await;
                        selected_values.extend(path.get(&NbtTag::Compound(entity_nbt)));
                    }
                }
            } else if let Some(block_position) = compound.get_string("block")
                && let (Some(world), Some(position)) = (
                    source.world.as_ref(),
                    parse_block_position(block_position, source),
                )
                && world.is_loaded(&position)
                && let Some(block_entity) = world.get_block_entity(&position)
            {
                let mut block_nbt = NbtCompound::new();
                block_entity.write_internal(&mut block_nbt).await;
                selected_values.extend(path.get(&NbtTag::Compound(block_nbt)));
            } else if let Some(storage_id) = compound.get_string("storage")
                && let (Some(server), Ok(storage_id)) =
                    (source.server.as_ref(), Identifier::parse(storage_id))
            {
                let storage_nbt = server.command_storage.get(&storage_id).await;
                selected_values.extend(path.get(&NbtTag::Compound(storage_nbt)));
            }
        }

        let separator = if let Some(separator) = compound.get("separator") {
            Box::pin(resolve_page_tag(separator, source, player, depth + 1)).await?
        } else {
            plain_separator()
        };
        let interpret = compound.get_bool("interpret").unwrap_or(false);
        let rendered = resolve_nbt_values(
            selected_values,
            interpret,
            &separator,
            source,
            player,
            depth + 1,
        )
        .await;
        apply_resolved_contents(&mut result, rendered);
        result.child_tags.retain(|key, _| {
            !matches!(
                &**key,
                "nbt" | "entity" | "block" | "storage" | "interpret" | "separator"
            )
        });
    }

    // 1. Selector resolution: {"selector": "@p", "separator": optional_component}
    if let Some(selector_str) = compound.get_string("selector") {
        if let Ok(selector) = parse_target_selector(selector_str) {
            result.child_tags.retain(|key, _| &**key != "extra");
            let entities = if let Some(server) = &source.server {
                server
                    .select_entities(&selector, Some(&source.output))
                    .await
            } else {
                Vec::new()
            };

            let separator = if let Some(sep_tag) = compound.get("separator") {
                Box::pin(resolve_page_tag(sep_tag, source, player, depth + 1))
                    .await
                    .unwrap_or_else(default_separator)
            } else {
                default_separator()
            };

            if entities.is_empty() {
                result.put_string("text", String::new());
            } else if entities.len() == 1 {
                let display_name = entities[0].get_display_name().await;
                let display_compound = display_name.0.to_nbt_compound();
                if let Some(text) = display_compound.get_string("text") {
                    result.put_string("text", text.to_string());
                } else {
                    result.put(
                        "extra",
                        NbtTag::List(vec![NbtTag::Compound(display_compound)]),
                    );
                    result.put_string("text", String::new());
                }
            } else {
                let mut extra_list = Vec::new();
                for (idx, entity) in entities.iter().enumerate() {
                    if idx > 0 {
                        extra_list.push(separator.clone());
                    }
                    let display_name = entity.get_display_name().await;
                    extra_list.push(NbtTag::Compound(display_name.0.to_nbt_compound()));
                }
                result.put_string("text", String::new());
                result.put("extra", NbtTag::List(extra_list));
            }
            // Remove unresolved selector fields
            result
                .child_tags
                .retain(|k, _| &**k != "selector" && &**k != "separator");
        }
    }

    // 2. Score resolution: {"score": {"name": "@p", "objective": "points"}}
    if let Some(score_compound) = compound.get_compound("score") {
        let name_spec = score_compound.get_string("name").unwrap_or("*");
        let objective = score_compound.get_string("objective").unwrap_or_default();

        let target_name = if name_spec == "*" {
            player.map_or_else(|| "*".to_string(), |player| player.gameprofile.name.clone())
        } else if name_spec.starts_with('@') {
            let Ok(selector) = parse_target_selector(name_spec) else {
                // ComponentUtils propagates selector syntax failures; written
                // books catch that failure and preserve the original page.
                return Some(compound.clone());
            };
            let entities = if let Some(server) = &source.server {
                server
                    .select_entities(&selector, Some(&source.output))
                    .await
            } else {
                Vec::new()
            };
            let Ok(name) = score_holder_name(name_spec, &entities) else {
                // ScoreContents requires at most one selected holder. More
                // than one is a command-syntax failure, so preserve this page.
                return Some(compound.clone());
            };
            name
        } else {
            name_spec.to_string()
        };

        let score_val = if let Some(world) = &source.world {
            let scoreboard = world.scoreboard.lock().await;
            scoreboard.get_score_value(&target_name, objective)
        } else {
            None
        };

        let score_str = score_val.map(|v| v.to_string()).unwrap_or_default();
        result.child_tags.retain(|key, _| &**key != "extra");
        result.put_string("text", score_str);
        result.child_tags.retain(|k, _| &**k != "score");
    }

    // 3. Translate arguments resolution: {"translate": "key", "with": [...]}
    if compound.get_string("translate").is_some() {
        if let Some(NbtTag::List(with_list)) = compound.get("with") {
            let mut resolved_with = Vec::with_capacity(with_list.len());
            for arg in with_list {
                let resolved_arg =
                    Box::pin(resolve_page_tag(arg, source, player, depth + 1)).await?;
                resolved_with.push(resolved_arg);
            }
            result.put("with", NbtTag::List(resolved_with));
        }
    }

    // 4. Extra siblings resolution: {"text": "...", "extra": [...]}
    if let Some(NbtTag::List(extra_list)) = compound.get("extra") {
        let mut resolved_extra = match result.get("extra") {
            Some(NbtTag::List(generated)) if generated.as_slice() != extra_list.as_slice() => {
                generated.clone()
            }
            _ => Vec::new(),
        };
        resolved_extra.reserve(extra_list.len());
        for child in extra_list {
            let resolved_child =
                Box::pin(resolve_page_tag(child, source, player, depth + 1)).await?;
            resolved_extra.push(resolved_child);
        }
        result.put("extra", NbtTag::List(resolved_extra));
    }

    // 5. HoverEvent resolution. Vanilla 1.21.4 accepts the modern
    // `contents` payload and the legacy `value` alternative.
    if let Some(hover_compound) = compound.get_compound("hover_event") {
        if hover_compound.get_string("action") == Some("show_text") {
            let payload_key = if hover_compound.get("contents").is_some() {
                Some("contents")
            } else if hover_compound.get("value").is_some() {
                Some("value")
            } else {
                None
            };
            if let Some(payload_key) = payload_key {
                let val = hover_compound.get(payload_key)?;
                let mut new_hover = hover_compound.clone();
                let resolved_val =
                    Box::pin(resolve_page_tag(val, source, player, depth + 1)).await?;
                new_hover.put(payload_key, resolved_val);
                result.put_compound("hover_event", new_hover);
            }
        }
    }

    Some(result)
}

fn score_holder_name(selector_text: &str, entities: &[Arc<dyn EntityBase>]) -> Result<String, ()> {
    match entities {
        [] => Ok(selector_text.to_string()),
        [entity] => Ok(entity.get_player().map_or_else(
            || entity.get_entity().entity_uuid.to_string(),
            |player| player.gameprofile.name.clone(),
        )),
        _ => Err(()),
    }
}

fn default_separator() -> NbtTag {
    let mut sep = NbtCompound::new();
    sep.put_string("text", ", ".to_string());
    sep.put_string("color", "gray".to_string());
    NbtTag::Compound(sep)
}

fn plain_separator() -> NbtTag {
    let mut separator = NbtCompound::new();
    separator.put_string("text", ", ".to_owned());
    NbtTag::Compound(separator)
}

fn parse_block_position(input: &str, source: &CommandSource) -> Option<BlockPos> {
    let mut reader = StringReader::new(input);
    let coordinates = BlockPosArgumentType.parse(&mut reader).ok()?;
    if reader.can_read_char() {
        return None;
    }
    Some(BlockPos::floored_v(coordinates.resolve(source)))
}

async fn resolve_nbt_values(
    values: Vec<NbtTag>,
    interpret: bool,
    separator: &NbtTag,
    source: &CommandSource,
    player: Option<&Arc<Player>>,
    depth: usize,
) -> Vec<NbtTag> {
    let mut components = Vec::new();
    for value in values {
        let raw = nbt_value_as_string(&value);
        let component = if interpret {
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) else {
                continue;
            };
            let Some(component) = json_component_to_nbt(&json) else {
                continue;
            };
            let Some(resolved) =
                Box::pin(resolve_page_tag(&component, source, player, depth + 1)).await
            else {
                continue;
            };
            resolved
        } else {
            NbtTag::String(raw.into_boxed_str())
        };

        if !components.is_empty() {
            components.push(separator.clone());
        }
        components.push(component);
    }
    components
}

fn nbt_value_as_string(value: &NbtTag) -> String {
    match value {
        NbtTag::String(value) => value.to_string(),
        _ => value.to_string(),
    }
}

fn json_component_to_nbt(value: &serde_json::Value) -> Option<NbtTag> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(value) => Some(NbtTag::Byte(i8::from(*value))),
        serde_json::Value::Number(value) => value
            .as_i64()
            .map(|number| i32::try_from(number).map_or(NbtTag::Long(number), NbtTag::Int))
            .or_else(|| value.as_f64().map(NbtTag::Double)),
        serde_json::Value::String(value) => Some(NbtTag::String(value.clone().into_boxed_str())),
        serde_json::Value::Array(values) => values
            .iter()
            .map(json_component_to_nbt)
            .collect::<Option<Vec<_>>>()
            .map(NbtTag::List),
        serde_json::Value::Object(values) => {
            let mut compound = NbtCompound::new();
            for (key, value) in values {
                compound.put(key, json_component_to_nbt(value)?);
            }
            Some(NbtTag::Compound(compound))
        }
    }
}

fn apply_resolved_contents(result: &mut NbtCompound, components: Vec<NbtTag>) {
    result.put_string("text", String::new());
    if components.is_empty() {
        result.child_tags.retain(|key, _| &**key != "extra");
    } else {
        result.put("extra", NbtTag::List(components));
    }
}

fn java_string_len(value: &str) -> usize {
    value.encode_utf16().count()
}

/// Helper serializing an NbtTag to its JSON string representation.
fn nbt_tag_to_json_string(tag: &NbtTag) -> String {
    match tag {
        NbtTag::String(s) => s.to_string(),
        NbtTag::Compound(c) => {
            let mut map = serde_json::Map::new();
            for (k, v) in &c.child_tags {
                map.insert(k.to_string(), nbt_to_json_value(v));
            }
            serde_json::to_string(&serde_json::Value::Object(map)).unwrap_or_default()
        }
        _ => String::new(),
    }
}

fn nbt_to_json_value(tag: &NbtTag) -> serde_json::Value {
    match tag {
        NbtTag::Byte(b) => serde_json::Value::Bool(*b != 0),
        NbtTag::Short(s) => serde_json::Value::Number((*s).into()),
        NbtTag::Int(i) => serde_json::Value::Number((*i).into()),
        NbtTag::Long(l) => serde_json::Value::Number((*l).into()),
        NbtTag::Float(f) => serde_json::Number::from_f64(f64::from(*f))
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        NbtTag::Double(d) => serde_json::Number::from_f64(*d)
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        NbtTag::String(s) => serde_json::Value::String(s.to_string()),
        NbtTag::List(list) => {
            serde_json::Value::Array(list.iter().map(nbt_to_json_value).collect())
        }
        NbtTag::Compound(c) => {
            let mut map = serde_json::Map::new();
            for (k, v) in &c.child_tags {
                map.insert(k.to_string(), nbt_to_json_value(v));
            }
            serde_json::Value::Object(map)
        }
        _ => serde_json::Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::data_component_impl::WrittenBookContentImpl;

    #[tokio::test]
    async fn resolve_simple_text_page_preserves_content_and_marks_resolved() {
        let mut book = ItemStack::new(1, &Item::WRITTEN_BOOK);
        let mut page = NbtCompound::new();
        page.put_string("text", "Hello World".to_string());
        page.put_string("color", "gold".to_string());

        let content = WrittenBookContentImpl {
            title: "Test".into(),
            filtered_title: None,
            author: "Player".into(),
            generation: 0,
            pages: vec![NbtTag::Compound(page.clone())],
            filtered_pages: vec![None],
            resolved: false,
        };
        book.set_data_component(content);

        let source = CommandSource::dummy();
        let changed = resolve_book_components(&mut book, &source, None).await;
        assert!(changed);

        let resolved_content = book.get_data_component::<WrittenBookContentImpl>().unwrap();
        assert!(resolved_content.resolved);
        assert_eq!(resolved_content.pages.len(), 1);
        if let NbtTag::Compound(c) = &resolved_content.pages[0] {
            assert_eq!(c.get_string("text"), Some("Hello World"));
            assert_eq!(c.get_string("color"), Some("gold"));
        } else {
            panic!("Expected compound tag");
        }
    }

    #[tokio::test]
    async fn resolve_already_resolved_book_returns_false() {
        let mut book = ItemStack::new(1, &Item::WRITTEN_BOOK);
        let content = WrittenBookContentImpl {
            title: "Test".into(),
            filtered_title: None,
            author: "Player".into(),
            generation: 0,
            pages: vec![NbtTag::String("Page 1".into())],
            filtered_pages: vec![None],
            resolved: true,
        };
        book.set_data_component(content);

        let source = CommandSource::dummy();
        assert!(!resolve_book_components(&mut book, &source, None).await);
    }

    #[tokio::test]
    async fn resolve_page_over_max_length_safely_marks_resolved_without_corrupting() {
        let mut book = ItemStack::new(1, &Item::WRITTEN_BOOK);
        let huge_text = "a".repeat(MAX_PAGE_LENGTH + 10);
        let content = WrittenBookContentImpl {
            title: "Huge".into(),
            filtered_title: None,
            author: "Player".into(),
            generation: 0,
            pages: vec![NbtTag::String(huge_text.clone().into_boxed_str())],
            filtered_pages: vec![None],
            resolved: false,
        };
        book.set_data_component(content);

        let source = CommandSource::dummy();
        let changed = resolve_book_components(&mut book, &source, None).await;
        assert!(!changed); // Failed resolution

        let res_content = book.get_data_component::<WrittenBookContentImpl>().unwrap();
        assert!(res_content.resolved); // Marked resolved to avoid loops
        assert_eq!(res_content.pages.len(), 1);
        assert_eq!(
            res_content.pages[0],
            NbtTag::String(huge_text.into_boxed_str())
        );
    }

    #[tokio::test]
    async fn resolve_selector_replaces_with_text_and_cleans_tag() {
        let mut page = NbtCompound::new();
        page.put_string("selector", "@p".to_string());
        page.put_string("color", "aqua".to_string());

        let source = CommandSource::dummy();
        let resolved = resolve_page_tag(&NbtTag::Compound(page), &source, None, 0).await;
        assert!(resolved.is_some());

        if let Some(NbtTag::Compound(c)) = resolved {
            assert_eq!(c.get_string("selector"), None);
            assert_eq!(c.get_string("text"), Some(""));
            assert_eq!(c.get_string("color"), Some("aqua"));
        } else {
            panic!("Expected compound tag");
        }
    }

    #[tokio::test]
    async fn resolve_score_replaces_with_score_value_and_cleans_tag() {
        let mut score_c = NbtCompound::new();
        score_c.put_string("name", "*".to_string());
        score_c.put_string("objective", "dummy_points".to_string());

        let mut page = NbtCompound::new();
        page.put_compound("score", score_c);

        let source = CommandSource::dummy();
        let resolved = resolve_page_tag(&NbtTag::Compound(page), &source, None, 0).await;
        assert!(resolved.is_some());

        if let Some(NbtTag::Compound(c)) = resolved {
            assert_eq!(c.get_compound("score"), None);
            assert_eq!(c.get_string("text"), Some(""));
        } else {
            panic!("Expected compound tag");
        }
    }

    #[test]
    fn empty_score_selector_uses_literal_selector_holder_name() {
        let entities: Vec<Arc<dyn EntityBase>> = Vec::new();
        assert_eq!(score_holder_name("@p", &entities), Ok("@p".to_string()));
    }

    #[tokio::test]
    async fn resolve_translate_with_nested_args() {
        let mut child_arg = NbtCompound::new();
        child_arg.put_string("text", "arg1".to_string());

        let mut page = NbtCompound::new();
        page.put_string("translate", "chat.type.text".to_string());
        page.put("with", NbtTag::List(vec![NbtTag::Compound(child_arg)]));

        let source = CommandSource::dummy();
        let resolved = resolve_page_tag(&NbtTag::Compound(page), &source, None, 0).await;
        assert!(resolved.is_some());

        if let Some(NbtTag::Compound(c)) = resolved {
            assert_eq!(c.get_string("translate"), Some("chat.type.text"));
            if let Some(NbtTag::List(with_list)) = c.get("with") {
                assert_eq!(with_list.len(), 1);
            } else {
                panic!("Expected with list");
            }
        } else {
            panic!("Expected compound tag");
        }
    }

    #[tokio::test]
    async fn resolves_modern_show_text_hover_contents() {
        let mut hover_contents = NbtCompound::new();
        hover_contents.put_string("selector", "@p".to_string());
        let mut hover = NbtCompound::new();
        hover.put_string("action", "show_text".to_string());
        hover.put_compound("contents", hover_contents);
        let mut page = NbtCompound::new();
        page.put_string("text", "hover me".to_string());
        page.put_compound("hover_event", hover);

        let resolved = resolve_page_tag(&NbtTag::Compound(page), &CommandSource::dummy(), None, 0)
            .await
            .unwrap();
        let NbtTag::Compound(page) = resolved else {
            panic!("expected component compound");
        };
        let contents = page
            .get_compound("hover_event")
            .and_then(|hover| hover.get_compound("contents"))
            .expect("modern hover contents must remain under contents");
        assert_eq!(contents.get_string("selector"), None);
        assert_eq!(contents.get_string("text"), Some(""));
    }

    #[tokio::test]
    async fn resolves_legacy_show_text_hover_value() {
        let mut hover_value = NbtCompound::new();
        hover_value.put_string("selector", "@p".to_string());
        let mut hover = NbtCompound::new();
        hover.put_string("action", "show_text".to_string());
        hover.put_compound("value", hover_value);
        let mut page = NbtCompound::new();
        page.put_string("text", "hover me".to_string());
        page.put_compound("hover_event", hover);

        let resolved = resolve_page_tag(&NbtTag::Compound(page), &CommandSource::dummy(), None, 0)
            .await
            .unwrap();
        let NbtTag::Compound(page) = resolved else {
            panic!("expected component compound");
        };
        let value = page
            .get_compound("hover_event")
            .and_then(|hover| hover.get_compound("value"))
            .expect("legacy hover payload must remain under value");
        assert_eq!(value.get_string("selector"), None);
        assert_eq!(value.get_string("text"), Some(""));
    }

    #[tokio::test]
    async fn show_entity_name_is_not_recursively_resolved() {
        let mut entity_name = NbtCompound::new();
        entity_name.put_string("selector", "@p".to_string());
        let mut hover = NbtCompound::new();
        hover.put_string("action", "show_entity".to_string());
        hover.put_string("id", "minecraft:pig".to_string());
        hover.put_string("uuid", "00000000-0000-0000-0000-000000000001".to_string());
        hover.put_compound("name", entity_name);
        let mut page = NbtCompound::new();
        page.put_string("text", "hover me".to_string());
        page.put_compound("hover_event", hover);

        let resolved = resolve_page_tag(&NbtTag::Compound(page), &CommandSource::dummy(), None, 0)
            .await
            .unwrap();
        let NbtTag::Compound(page) = resolved else {
            panic!("expected component compound");
        };
        let name = page
            .get_compound("hover_event")
            .and_then(|hover| hover.get_compound("name"))
            .expect("show_entity name must be preserved");
        assert_eq!(name.get_string("selector"), Some("@p"));
        assert_eq!(name.get_string("text"), None);
    }

    #[tokio::test]
    async fn json_looking_string_component_remains_literal() {
        let json_page = r#"{"text":"Score: ","extra":[{"selector":"@p"}]}"#;
        let source = CommandSource::dummy();
        let resolved = resolve_page_tag(&NbtTag::String(json_page.into()), &source, None, 0).await;
        assert!(resolved.is_some());

        assert_eq!(resolved, Some(NbtTag::String(json_page.into())));
    }

    #[tokio::test]
    async fn page_limit_counts_java_utf16_units_not_utf8_bytes() {
        // 20,000 BMP characters are 20,000 Java chars but 40,000 UTF-8 bytes.
        let page = "é".repeat(20_000);
        let source = CommandSource::dummy();
        assert_eq!(
            resolve_page_tag(&NbtTag::String(page.clone().into()), &source, None, 0).await,
            Some(NbtTag::String(page.into_boxed_str()))
        );

        // Supplementary characters consume two UTF-16 code units each.
        let too_large = "😀".repeat((MAX_PAGE_LENGTH / 2) + 1);
        assert_eq!(
            resolve_page_tag(&NbtTag::String(too_large.into()), &source, None, 0).await,
            None
        );
    }

    #[tokio::test]
    async fn uninterpreted_nbt_values_use_raw_strings_and_plain_separator() {
        let values = vec![NbtTag::String("alpha".into()), NbtTag::Int(42)];
        let resolved = resolve_nbt_values(
            values,
            false,
            &plain_separator(),
            &CommandSource::dummy(),
            None,
            0,
        )
        .await;
        assert_eq!(
            resolved,
            vec![
                NbtTag::String("alpha".into()),
                plain_separator(),
                NbtTag::String("42".into())
            ]
        );
    }

    #[tokio::test]
    async fn interpreted_nbt_values_parse_components_and_drop_bad_json() {
        let values = vec![
            NbtTag::String(r#"{"text":"first","color":"gold"}"#.into()),
            NbtTag::String("not json".into()),
            NbtTag::String(r#"{"text":"second"}"#.into()),
        ];
        let resolved = resolve_nbt_values(
            values,
            true,
            &plain_separator(),
            &CommandSource::dummy(),
            None,
            0,
        )
        .await;
        assert_eq!(resolved.len(), 3);
        let NbtTag::Compound(first) = &resolved[0] else {
            panic!("first interpreted component must be a compound");
        };
        assert_eq!(first.get_string("text"), Some("first"));
        assert_eq!(first.get_string("color"), Some("gold"));
        assert_eq!(resolved[1], plain_separator());
        let NbtTag::Compound(second) = &resolved[2] else {
            panic!("second interpreted component must be a compound");
        };
        assert_eq!(second.get_string("text"), Some("second"));
    }

    #[tokio::test]
    async fn entity_nbt_component_with_no_context_is_empty_but_keeps_siblings() {
        let mut sibling = NbtCompound::new();
        sibling.put_string("text", "tail".to_owned());
        let mut page = NbtCompound::new();
        page.put_string("nbt", "Health".to_owned());
        page.put_string("entity", "@s".to_owned());
        page.put("extra", NbtTag::List(vec![NbtTag::Compound(sibling)]));

        let resolved = resolve_page_tag(&NbtTag::Compound(page), &CommandSource::dummy(), None, 0)
            .await
            .unwrap();
        let NbtTag::Compound(resolved) = resolved else {
            panic!("NBT component must remain a compound component");
        };
        assert_eq!(resolved.get_string("nbt"), None);
        assert_eq!(resolved.get_string("entity"), None);
        assert_eq!(resolved.get_string("text"), Some(""));
        let Some(NbtTag::List(extra)) = resolved.get("extra") else {
            panic!("original siblings must survive content replacement");
        };
        assert_eq!(extra.len(), 1);
    }

    #[test]
    fn block_nbt_positions_use_vanilla_block_position_coordinates() {
        let mut source = CommandSource::dummy();
        source.position = pumpkin_util::math::vector3::Vector3::new(10.8, 64.2, -4.1);
        assert_eq!(
            parse_block_position("1 2 3", &source),
            Some(BlockPos(pumpkin_util::math::vector3::Vector3::new(1, 2, 3)))
        );
        assert_eq!(
            parse_block_position("~ ~1 ~-2", &source),
            Some(BlockPos(pumpkin_util::math::vector3::Vector3::new(
                10, 65, -7
            )))
        );
        assert!(parse_block_position("1 2 3 trailing", &source).is_none());
        assert!(parse_block_position("1 2", &source).is_none());
    }

    #[tokio::test]
    async fn block_nbt_component_without_world_resolves_to_empty() {
        let mut page = NbtCompound::new();
        page.put_string("nbt", "Items[]".to_owned());
        page.put_string("block", "~ ~ ~".to_owned());
        page.put_bool("interpret", false);

        let resolved = resolve_page_tag(&NbtTag::Compound(page), &CommandSource::dummy(), None, 0)
            .await
            .unwrap();
        let NbtTag::Compound(resolved) = resolved else {
            panic!("block NBT component must remain a compound component");
        };
        assert_eq!(resolved.get_string("block"), None);
        assert_eq!(resolved.get_string("nbt"), None);
        assert_eq!(resolved.get_bool("interpret"), None);
        assert_eq!(resolved.get_string("text"), Some(""));
    }

    #[tokio::test]
    async fn storage_nbt_component_without_server_resolves_to_empty() {
        let mut page = NbtCompound::new();
        page.put_string("nbt", "message".to_owned());
        page.put_string("storage", "demo:book".to_owned());

        let resolved = resolve_page_tag(&NbtTag::Compound(page), &CommandSource::dummy(), None, 0)
            .await
            .unwrap();
        let NbtTag::Compound(resolved) = resolved else {
            panic!("storage NBT component must remain a compound component");
        };
        assert_eq!(resolved.get_string("storage"), None);
        assert_eq!(resolved.get_string("nbt"), None);
        assert_eq!(resolved.get_string("text"), Some(""));
    }

    #[tokio::test]
    async fn resolve_recursion_depth_limit_stops_at_100() {
        let mut page = NbtCompound::new();
        page.put_string("text", "deep".to_string());
        let source = CommandSource::dummy();
        let resolved = resolve_page_tag(&NbtTag::Compound(page), &source, None, 101).await;
        assert!(resolved.is_some());
    }
}
