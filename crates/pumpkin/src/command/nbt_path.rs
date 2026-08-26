//! Vanilla-compatible NBT path parsing, selection, and mutation.

use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};

use super::{snbt::SnbtParser, string_reader::StringReader};

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Child(String),
    MatchingChild(String, NbtCompound),
    Index(i32),
    AllElements,
    MatchingElement(NbtCompound),
    MatchingRoot(NbtCompound),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NbtPath {
    original: String,
    nodes: Vec<Node>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbtPathError {
    InvalidNode,
    InvalidSnbt,
    MissingClosingBracket,
    TrailingInput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NbtMutationError {
    NothingFound,
    TooDeep,
    ExpectedList(String),
    ExpectedObject(String),
    InvalidIndex(i32),
}

impl NbtPath {
    pub fn parse(input: &str) -> Result<Self, NbtPathError> {
        let mut reader = StringReader::new(input);
        let mut nodes = Vec::new();
        let mut first = true;

        while reader.can_read_char() && reader.peek() != Some(' ') {
            nodes.push(parse_node(&mut reader, first)?);
            first = false;
            match reader.peek() {
                None | Some(' ') | Some('[' | '{') => {}
                Some('.') => reader.skip(),
                Some(_) => return Err(NbtPathError::TrailingInput),
            }
        }
        if nodes.is_empty() || reader.can_read_char() {
            return Err(NbtPathError::InvalidNode);
        }
        Ok(Self {
            original: input.to_owned(),
            nodes,
        })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.original
    }

    /// Returns all tags selected by this path, in Vanilla traversal order.
    #[must_use]
    pub fn get(&self, root: &NbtTag) -> Vec<NbtTag> {
        let mut current = vec![root.clone()];
        for node in &self.nodes {
            current = current.iter().flat_map(|tag| select(node, tag)).collect();
            if current.is_empty() {
                break;
            }
        }
        current
    }

    /// Removes every tag selected by this path and returns the number of
    /// removed values, matching Vanilla `NbtPath.remove`.
    pub fn remove(&self, root: &mut NbtTag) -> usize {
        remove_at(&self.nodes, root)
    }

    /// Sets every tag selected by this path, creating Vanilla's preferred
    /// intermediate parents where the node family permits it.
    pub fn set(&self, root: &mut NbtTag, value: NbtTag) -> Result<usize, NbtMutationError> {
        if is_too_deep(&value, self.nodes.len()) {
            return Err(NbtMutationError::TooDeep);
        }
        set_at(&self.nodes, root, &value).ok_or(NbtMutationError::NothingFound)
    }

    /// Inserts source tags into every list selected by this path. The result is
    /// the number of destination lists that accepted at least one value.
    pub fn insert(
        &self,
        index: i32,
        root: &mut NbtTag,
        values: &[NbtTag],
    ) -> Result<usize, NbtMutationError> {
        if values
            .iter()
            .any(|value| is_too_deep(value, self.nodes.len()))
        {
            return Err(NbtMutationError::TooDeep);
        }
        insert_at(&self.nodes, root, index, values).ok_or(NbtMutationError::NothingFound)?
    }

    /// Merges all source compounds, in order, into every compound selected by
    /// this path and returns the number of destination compounds changed.
    pub fn merge(&self, root: &mut NbtTag, values: &[NbtTag]) -> Result<usize, NbtMutationError> {
        let mut combined = NbtCompound::new();
        for value in values {
            if is_too_deep(value, 0) {
                return Err(NbtMutationError::TooDeep);
            }
            let NbtTag::Compound(source) = value else {
                return Err(NbtMutationError::ExpectedObject(value.to_string()));
            };
            merge_compounds(&mut combined, source);
        }
        merge_at(&self.nodes, root, &combined).ok_or(NbtMutationError::NothingFound)?
    }
}

fn merge_at(
    nodes: &[Node],
    current: &mut NbtTag,
    source: &NbtCompound,
) -> Option<Result<usize, NbtMutationError>> {
    let (node, remaining) = nodes.split_first()?;
    if remaining.is_empty() {
        return merge_selected(node, current, source);
    }
    let next = &remaining[0];
    let mut results = Vec::new();
    match node {
        Node::Child(name) => {
            let NbtTag::Compound(compound) = current else {
                return None;
            };
            let child = compound
                .child_tags
                .entry(name.clone().into())
                .or_insert_with(|| preferred_parent(next));
            if let Some(result) = merge_at(remaining, child, source) {
                results.push(result);
            }
        }
        Node::MatchingChild(name, pattern) => {
            let NbtTag::Compound(compound) = current else {
                return None;
            };
            if !compound.child_tags.contains_key(name.as_str()) {
                compound.put_compound(name, pattern.clone());
            }
            if let Some(child) = compound
                .child_tags
                .get_mut(name.as_str())
                .filter(|child| matches_compound(pattern, child))
                && let Some(result) = merge_at(remaining, child, source)
            {
                results.push(result);
            }
        }
        Node::Index(index) => {
            let NbtTag::List(items) = current else {
                return None;
            };
            if let Some(child) =
                resolved_index(items.len(), *index).and_then(|index| items.get_mut(index))
                && let Some(result) = merge_at(remaining, child, source)
            {
                results.push(result);
            }
        }
        Node::AllElements => {
            let NbtTag::List(items) = current else {
                return None;
            };
            if items.is_empty() {
                items.push(preferred_parent(next));
            }
            for child in items {
                if let Some(result) = merge_at(remaining, child, source) {
                    results.push(result);
                }
            }
        }
        Node::MatchingElement(pattern) => {
            let NbtTag::List(items) = current else {
                return None;
            };
            if !items.iter().any(|child| matches_compound(pattern, child)) {
                items.push(NbtTag::Compound(pattern.clone()));
            }
            for child in items
                .iter_mut()
                .filter(|child| matches_compound(pattern, child))
            {
                if let Some(result) = merge_at(remaining, child, source) {
                    results.push(result);
                }
            }
        }
        Node::MatchingRoot(pattern) => {
            if !matches_compound(pattern, current) {
                return None;
            }
            if let Some(result) = merge_at(remaining, current, source) {
                results.push(result);
            }
        }
    }
    combine_results(results)
}

fn merge_selected(
    node: &Node,
    current: &mut NbtTag,
    source: &NbtCompound,
) -> Option<Result<usize, NbtMutationError>> {
    let mut targets = Vec::new();
    match node {
        Node::Child(name) => {
            let NbtTag::Compound(compound) = current else {
                return None;
            };
            targets.push(
                compound
                    .child_tags
                    .entry(name.clone().into())
                    .or_insert_with(|| NbtTag::Compound(NbtCompound::new())),
            );
        }
        Node::MatchingChild(name, pattern) => {
            let NbtTag::Compound(compound) = current else {
                return None;
            };
            if !compound.child_tags.contains_key(name.as_str()) {
                compound.put_compound(name, pattern.clone());
            }
            if let Some(child) = compound
                .child_tags
                .get_mut(name.as_str())
                .filter(|child| matches_compound(pattern, child))
            {
                targets.push(child);
            }
        }
        Node::Index(index) => {
            let NbtTag::List(items) = current else {
                return None;
            };
            if let Some(child) =
                resolved_index(items.len(), *index).and_then(|index| items.get_mut(index))
            {
                targets.push(child);
            }
        }
        Node::AllElements => {
            let NbtTag::List(items) = current else {
                return None;
            };
            if items.is_empty() {
                items.push(NbtTag::Compound(NbtCompound::new()));
            }
            targets.extend(items.iter_mut());
        }
        Node::MatchingElement(pattern) => {
            let NbtTag::List(items) = current else {
                return None;
            };
            if !items.iter().any(|child| matches_compound(pattern, child)) {
                items.push(NbtTag::Compound(pattern.clone()));
            }
            targets.extend(
                items
                    .iter_mut()
                    .filter(|child| matches_compound(pattern, child)),
            );
        }
        Node::MatchingRoot(pattern) => {
            if matches_compound(pattern, current) {
                targets.push(current);
            }
        }
    }
    if targets.is_empty() {
        return None;
    }
    Some(targets.into_iter().try_fold(0, |total, target| {
        let NbtTag::Compound(target) = target else {
            return Err(NbtMutationError::ExpectedObject(target.to_string()));
        };
        let original = target.clone();
        merge_compounds(target, source);
        Ok(total + usize::from(*target != original))
    }))
}

fn combine_results(
    results: Vec<Result<usize, NbtMutationError>>,
) -> Option<Result<usize, NbtMutationError>> {
    if results.is_empty() {
        None
    } else {
        Some(
            results
                .into_iter()
                .try_fold(0, |total, result| result.map(|changed| total + changed)),
        )
    }
}

fn merge_compounds(target: &mut NbtCompound, source: &NbtCompound) {
    for (key, value) in &source.child_tags {
        if let NbtTag::Compound(source_child) = value
            && let Some(NbtTag::Compound(target_child)) = target.child_tags.get_mut(key.as_ref())
        {
            merge_compounds(target_child, source_child);
        } else {
            target.child_tags.insert(key.clone(), value.clone());
        }
    }
}

fn insert_at(
    nodes: &[Node],
    current: &mut NbtTag,
    index: i32,
    values: &[NbtTag],
) -> Option<Result<usize, NbtMutationError>> {
    let (node, remaining) = nodes.split_first()?;
    if remaining.is_empty() {
        return insert_selected(node, current, index, values);
    }
    let next = &remaining[0];
    let mut results = Vec::new();
    match node {
        Node::Child(name) => {
            let NbtTag::Compound(compound) = current else {
                return None;
            };
            let child = compound
                .child_tags
                .entry(name.clone().into())
                .or_insert_with(|| preferred_parent(next));
            if let Some(result) = insert_at(remaining, child, index, values) {
                results.push(result);
            }
        }
        Node::MatchingChild(name, pattern) => {
            let NbtTag::Compound(compound) = current else {
                return None;
            };
            if !compound.child_tags.contains_key(name.as_str()) {
                compound.put_compound(name, pattern.clone());
            }
            if let Some(child) = compound
                .child_tags
                .get_mut(name.as_str())
                .filter(|child| matches_compound(pattern, child))
                && let Some(result) = insert_at(remaining, child, index, values)
            {
                results.push(result);
            }
        }
        Node::Index(selected) => {
            let NbtTag::List(items) = current else {
                return None;
            };
            if let Some(child) =
                resolved_index(items.len(), *selected).and_then(|selected| items.get_mut(selected))
                && let Some(result) = insert_at(remaining, child, index, values)
            {
                results.push(result);
            }
        }
        Node::AllElements => {
            let NbtTag::List(items) = current else {
                return None;
            };
            if items.is_empty() {
                items.push(preferred_parent(next));
            }
            for child in items {
                if let Some(result) = insert_at(remaining, child, index, values) {
                    results.push(result);
                }
            }
        }
        Node::MatchingElement(pattern) => {
            let NbtTag::List(items) = current else {
                return None;
            };
            if !items.iter().any(|child| matches_compound(pattern, child)) {
                items.push(NbtTag::Compound(pattern.clone()));
            }
            for child in items
                .iter_mut()
                .filter(|child| matches_compound(pattern, child))
            {
                if let Some(result) = insert_at(remaining, child, index, values) {
                    results.push(result);
                }
            }
        }
        Node::MatchingRoot(pattern) => {
            if !matches_compound(pattern, current) {
                return None;
            }
            if let Some(result) = insert_at(remaining, current, index, values) {
                results.push(result);
            }
        }
    }
    if results.is_empty() {
        None
    } else {
        Some(
            results
                .into_iter()
                .try_fold(0, |total, result| result.map(|changed| total + changed)),
        )
    }
}

fn insert_selected(
    node: &Node,
    current: &mut NbtTag,
    index: i32,
    values: &[NbtTag],
) -> Option<Result<usize, NbtMutationError>> {
    let mut targets = Vec::new();
    match node {
        Node::Child(name) => {
            let NbtTag::Compound(compound) = current else {
                return None;
            };
            targets.push(
                compound
                    .child_tags
                    .entry(name.clone().into())
                    .or_insert_with(|| NbtTag::List(Vec::new())),
            );
        }
        Node::MatchingChild(name, pattern) => {
            let NbtTag::Compound(compound) = current else {
                return None;
            };
            if !compound.child_tags.contains_key(name.as_str()) {
                compound.put_compound(name, pattern.clone());
            }
            if let Some(child) = compound
                .child_tags
                .get_mut(name.as_str())
                .filter(|child| matches_compound(pattern, child))
            {
                targets.push(child);
            }
        }
        Node::Index(selected) => {
            let NbtTag::List(items) = current else {
                return None;
            };
            if let Some(child) =
                resolved_index(items.len(), *selected).and_then(|selected| items.get_mut(selected))
            {
                targets.push(child);
            }
        }
        Node::AllElements => {
            let NbtTag::List(items) = current else {
                return None;
            };
            if items.is_empty() {
                items.push(NbtTag::List(Vec::new()));
            }
            targets.extend(items.iter_mut());
        }
        Node::MatchingElement(pattern) => {
            let NbtTag::List(items) = current else {
                return None;
            };
            if !items.iter().any(|child| matches_compound(pattern, child)) {
                items.push(NbtTag::Compound(pattern.clone()));
            }
            targets.extend(
                items
                    .iter_mut()
                    .filter(|child| matches_compound(pattern, child)),
            );
        }
        Node::MatchingRoot(pattern) => {
            if matches_compound(pattern, current) {
                targets.push(current);
            }
        }
    }
    if targets.is_empty() {
        return None;
    }
    Some(targets.into_iter().try_fold(0, |total, target| {
        insert_into_list(target, index, values).map(|changed| total + usize::from(changed))
    }))
}

fn insert_into_list(
    target: &mut NbtTag,
    index: i32,
    values: &[NbtTag],
) -> Result<bool, NbtMutationError> {
    let NbtTag::List(list) = target else {
        return Err(NbtMutationError::ExpectedList(target.to_string()));
    };
    let insertion = if index < 0 {
        i64::try_from(list.len())
            .ok()
            .and_then(|length| length.checked_add(i64::from(index)))
            .and_then(|offset| offset.checked_add(1))
    } else {
        Some(i64::from(index))
    };
    let Some(insertion) = insertion.and_then(|index| usize::try_from(index).ok()) else {
        return Err(NbtMutationError::InvalidIndex(index));
    };
    if insertion > list.len() {
        return Err(NbtMutationError::InvalidIndex(insertion as i32));
    }

    let mut cursor = insertion;
    let mut changed = false;
    for value in values {
        if list_accepts(list, value) {
            list.insert(cursor, value.clone());
            cursor += 1;
            changed = true;
        }
    }
    Ok(changed)
}

fn list_accepts(list: &[NbtTag], value: &NbtTag) -> bool {
    !matches!(value, NbtTag::End)
        && list
            .first()
            .is_none_or(|first| std::mem::discriminant(first) == std::mem::discriminant(value))
}

fn preferred_parent(node: &Node) -> NbtTag {
    match node {
        Node::Index(_) | Node::AllElements | Node::MatchingElement(_) => NbtTag::List(Vec::new()),
        Node::Child(_) | Node::MatchingChild(_, _) | Node::MatchingRoot(_) => {
            NbtTag::Compound(NbtCompound::new())
        }
    }
}

fn set_at(nodes: &[Node], current: &mut NbtTag, value: &NbtTag) -> Option<usize> {
    let (node, remaining) = nodes.split_first()?;
    if remaining.is_empty() {
        return Some(set_selected(node, current, value));
    }
    let next = &remaining[0];
    let mut found = false;
    let changed = match node {
        Node::Child(name) => match current {
            NbtTag::Compound(compound) => {
                let child = compound
                    .child_tags
                    .entry(name.clone().into())
                    .or_insert_with(|| preferred_parent(next));
                set_at(remaining, child, value).map_or(0, |changed| {
                    found = true;
                    changed
                })
            }
            _ => 0,
        },
        Node::MatchingChild(name, pattern) => match current {
            NbtTag::Compound(compound) => {
                if !compound.child_tags.contains_key(name.as_str()) {
                    compound.put_compound(name, pattern.clone());
                }
                compound
                    .child_tags
                    .get_mut(name.as_str())
                    .filter(|child| matches_compound(pattern, child))
                    .map_or(0, |child| {
                        set_at(remaining, child, value).map_or(0, |changed| {
                            found = true;
                            changed
                        })
                    })
            }
            _ => 0,
        },
        Node::Index(index) => match current {
            NbtTag::List(values) => resolved_index(values.len(), *index)
                .and_then(|index| values.get_mut(index))
                .map_or(0, |child| {
                    set_at(remaining, child, value).map_or(0, |changed| {
                        found = true;
                        changed
                    })
                }),
            _ => 0,
        },
        Node::AllElements => match current {
            NbtTag::List(values) => {
                if values.is_empty() {
                    values.push(preferred_parent(next));
                }
                values
                    .iter_mut()
                    .filter_map(|child| set_at(remaining, child, value))
                    .inspect(|_| found = true)
                    .sum()
            }
            _ => 0,
        },
        Node::MatchingElement(pattern) => match current {
            NbtTag::List(values) => {
                if !values.iter().any(|child| matches_compound(pattern, child)) {
                    values.push(NbtTag::Compound(pattern.clone()));
                }
                values
                    .iter_mut()
                    .filter(|child| matches_compound(pattern, child))
                    .filter_map(|child| set_at(remaining, child, value))
                    .inspect(|_| found = true)
                    .sum()
            }
            _ => 0,
        },
        Node::MatchingRoot(pattern) => {
            if matches_compound(pattern, current) {
                set_at(remaining, current, value).map_or(0, |changed| {
                    found = true;
                    changed
                })
            } else {
                0
            }
        }
    };
    found.then_some(changed)
}

fn set_selected(node: &Node, current: &mut NbtTag, value: &NbtTag) -> usize {
    match node {
        Node::Child(name) => match current {
            NbtTag::Compound(compound) => {
                let old = compound
                    .child_tags
                    .insert(name.clone().into(), value.clone());
                usize::from(old.as_ref() != Some(value))
            }
            _ => 0,
        },
        Node::MatchingChild(name, pattern) => match current {
            NbtTag::Compound(compound) => {
                let matches = compound
                    .get(name)
                    .is_some_and(|old| matches_compound(pattern, old));
                if !matches {
                    return 0;
                }
                let old = compound
                    .child_tags
                    .insert(name.clone().into(), value.clone());
                usize::from(old.as_ref() != Some(value))
            }
            _ => 0,
        },
        Node::Index(index) => set_collection_index(current, *index, value),
        Node::AllElements => set_all_elements(current, value),
        Node::MatchingElement(pattern) => match current {
            NbtTag::List(values) if values.is_empty() => {
                values.push(value.clone());
                1
            }
            NbtTag::List(values) => values
                .iter_mut()
                .filter(|old| matches_compound(pattern, old))
                .map(|old| {
                    if old == value {
                        0
                    } else {
                        *old = value.clone();
                        1
                    }
                })
                .sum(),
            _ => 0,
        },
        Node::MatchingRoot(_) => 0,
    }
}

fn set_collection_index(tag: &mut NbtTag, index: i32, value: &NbtTag) -> usize {
    match tag {
        NbtTag::List(values) => resolved_index(values.len(), index)
            .and_then(|index| values.get_mut(index))
            .map_or(0, |old| {
                if old == value {
                    0
                } else {
                    *old = value.clone();
                    1
                }
            }),
        NbtTag::ByteArray(values) => match value {
            NbtTag::Byte(value) => resolved_index(values.len(), index).map_or(0, |index| {
                let changed = values[index] != *value;
                values[index] = *value;
                usize::from(changed)
            }),
            _ => 0,
        },
        NbtTag::IntArray(values) => match value {
            NbtTag::Int(value) => resolved_index(values.len(), index).map_or(0, |index| {
                let changed = values[index] != *value;
                values[index] = *value;
                usize::from(changed)
            }),
            _ => 0,
        },
        NbtTag::LongArray(values) => match value {
            NbtTag::Long(value) => resolved_index(values.len(), index).map_or(0, |index| {
                let changed = values[index] != *value;
                values[index] = *value;
                usize::from(changed)
            }),
            _ => 0,
        },
        _ => 0,
    }
}

fn set_all_elements(tag: &mut NbtTag, value: &NbtTag) -> usize {
    match tag {
        NbtTag::List(values) if values.is_empty() => {
            values.push(value.clone());
            1
        }
        NbtTag::List(values) => {
            let changed = values.iter().filter(|old| *old != value).count();
            for old in values {
                *old = value.clone();
            }
            changed
        }
        NbtTag::ByteArray(values) => match value {
            NbtTag::Byte(value) => {
                let changed = values.iter().filter(|old| **old != *value).count();
                values.fill(*value);
                changed
            }
            _ => 0,
        },
        NbtTag::IntArray(values) => match value {
            NbtTag::Int(value) => {
                let changed = values.iter().filter(|old| **old != *value).count();
                values.fill(*value);
                changed
            }
            _ => 0,
        },
        NbtTag::LongArray(values) => match value {
            NbtTag::Long(value) => {
                let changed = values.iter().filter(|old| **old != *value).count();
                values.fill(*value);
                changed
            }
            _ => 0,
        },
        _ => 0,
    }
}

fn is_too_deep(tag: &NbtTag, depth: usize) -> bool {
    if depth >= 512 {
        return true;
    }
    match tag {
        NbtTag::Compound(compound) => compound
            .child_tags
            .values()
            .any(|child| is_too_deep(child, depth + 1)),
        NbtTag::List(values) => values.iter().any(|child| is_too_deep(child, depth + 1)),
        _ => false,
    }
}

fn remove_at(nodes: &[Node], current: &mut NbtTag) -> usize {
    let Some((node, remaining)) = nodes.split_first() else {
        return 0;
    };
    if remaining.is_empty() {
        return remove_selected(node, current);
    }

    match node {
        Node::Child(name) => match current {
            NbtTag::Compound(compound) => compound
                .child_tags
                .get_mut(name.as_str())
                .map_or(0, |child| remove_at(remaining, child)),
            _ => 0,
        },
        Node::MatchingChild(name, pattern) => match current {
            NbtTag::Compound(compound) => compound
                .child_tags
                .get_mut(name.as_str())
                .filter(|child| matches_compound(pattern, child))
                .map_or(0, |child| remove_at(remaining, child)),
            _ => 0,
        },
        Node::Index(index) => match current {
            NbtTag::List(values) => resolved_index(values.len(), *index)
                .and_then(|index| values.get_mut(index))
                .map_or(0, |child| remove_at(remaining, child)),
            _ => 0,
        },
        Node::AllElements => match current {
            NbtTag::List(values) => values
                .iter_mut()
                .map(|child| remove_at(remaining, child))
                .sum(),
            _ => 0,
        },
        Node::MatchingElement(pattern) => match current {
            NbtTag::List(values) => values
                .iter_mut()
                .filter(|child| matches_compound(pattern, child))
                .map(|child| remove_at(remaining, child))
                .sum(),
            _ => 0,
        },
        Node::MatchingRoot(pattern) => {
            if matches_compound(pattern, current) {
                remove_at(remaining, current)
            } else {
                0
            }
        }
    }
}

fn remove_selected(node: &Node, current: &mut NbtTag) -> usize {
    match node {
        Node::Child(name) => match current {
            NbtTag::Compound(compound) => {
                usize::from(compound.child_tags.remove(name.as_str()).is_some())
            }
            _ => 0,
        },
        Node::MatchingChild(name, pattern) => match current {
            NbtTag::Compound(compound)
                if compound
                    .get(name)
                    .is_some_and(|value| matches_compound(pattern, value)) =>
            {
                usize::from(compound.child_tags.remove(name.as_str()).is_some())
            }
            _ => 0,
        },
        Node::Index(index) => remove_collection_index(current, *index),
        Node::AllElements => clear_collection(current),
        Node::MatchingElement(pattern) => match current {
            NbtTag::List(values) => {
                let before = values.len();
                values.retain(|value| !matches_compound(pattern, value));
                before - values.len()
            }
            _ => 0,
        },
        Node::MatchingRoot(_) => 0,
    }
}

fn resolved_index(length: usize, index: i32) -> Option<usize> {
    let resolved = if index < 0 {
        i64::try_from(length).ok()?.checked_add(i64::from(index))?
    } else {
        i64::from(index)
    };
    usize::try_from(resolved)
        .ok()
        .filter(|index| *index < length)
}

fn remove_collection_index(tag: &mut NbtTag, index: i32) -> usize {
    macro_rules! remove_index {
        ($values:expr) => {{
            let Some(index) = resolved_index($values.len(), index) else {
                return 0;
            };
            let _ = $values.remove(index);
            1
        }};
    }
    match tag {
        NbtTag::List(values) => remove_index!(values),
        NbtTag::ByteArray(values) => {
            let mut owned = values.to_vec();
            let removed = remove_index!(owned);
            *values = owned.into_boxed_slice();
            removed
        }
        NbtTag::IntArray(values) => remove_index!(values),
        NbtTag::LongArray(values) => remove_index!(values),
        _ => 0,
    }
}

fn clear_collection(tag: &mut NbtTag) -> usize {
    match tag {
        NbtTag::List(values) => {
            let removed = values.len();
            values.clear();
            removed
        }
        NbtTag::ByteArray(values) => {
            let removed = values.len();
            *values = Box::default();
            removed
        }
        NbtTag::IntArray(values) => {
            let removed = values.len();
            values.clear();
            removed
        }
        NbtTag::LongArray(values) => {
            let removed = values.len();
            values.clear();
            removed
        }
        _ => 0,
    }
}

fn parse_node(reader: &mut StringReader<'_>, first: bool) -> Result<Node, NbtPathError> {
    match reader.peek() {
        Some('{') if first => parse_compound(reader).map(Node::MatchingRoot),
        Some('{') => Err(NbtPathError::InvalidNode),
        Some('[') => {
            reader.skip();
            match reader.peek() {
                Some('{') => {
                    let pattern = parse_compound(reader)?;
                    expect(reader, ']')?;
                    Ok(Node::MatchingElement(pattern))
                }
                Some(']') => {
                    reader.skip();
                    Ok(Node::AllElements)
                }
                _ => {
                    let index = reader.read_int().map_err(|_| NbtPathError::InvalidNode)?;
                    expect(reader, ']')?;
                    Ok(Node::Index(index))
                }
            }
        }
        Some('"' | '\'') => {
            let name = reader
                .read_string()
                .map_err(|_| NbtPathError::InvalidNode)?;
            parse_child(reader, name)
        }
        Some(_) => {
            let start = reader.cursor();
            while reader.peek().is_some_and(is_unquoted_name_char) {
                reader.skip();
            }
            if reader.cursor() == start {
                return Err(NbtPathError::InvalidNode);
            }
            parse_child(reader, reader.string()[start..reader.cursor()].to_owned())
        }
        None => Err(NbtPathError::InvalidNode),
    }
}

fn parse_child(reader: &mut StringReader<'_>, name: String) -> Result<Node, NbtPathError> {
    if reader.peek() == Some('{') {
        Ok(Node::MatchingChild(name, parse_compound(reader)?))
    } else {
        Ok(Node::Child(name))
    }
}

fn parse_compound(reader: &mut StringReader<'_>) -> Result<NbtCompound, NbtPathError> {
    match SnbtParser::parse_for_commands(reader).map_err(|_| NbtPathError::InvalidSnbt)? {
        NbtTag::Compound(compound) => Ok(compound),
        _ => Err(NbtPathError::InvalidSnbt),
    }
}

fn expect(reader: &mut StringReader<'_>, expected: char) -> Result<(), NbtPathError> {
    if reader.peek() == Some(expected) {
        reader.skip();
        Ok(())
    } else {
        Err(NbtPathError::MissingClosingBracket)
    }
}

const fn is_unquoted_name_char(c: char) -> bool {
    !matches!(c, ' ' | '"' | '\'' | '[' | ']' | '.' | '{' | '}')
}

fn select(node: &Node, tag: &NbtTag) -> Vec<NbtTag> {
    match node {
        Node::Child(name) => match tag {
            NbtTag::Compound(compound) => compound.get(name).cloned().into_iter().collect(),
            _ => Vec::new(),
        },
        Node::MatchingChild(name, pattern) => match tag {
            NbtTag::Compound(compound) => compound
                .get(name)
                .filter(|value| matches_compound(pattern, value))
                .cloned()
                .into_iter()
                .collect(),
            _ => Vec::new(),
        },
        Node::MatchingRoot(pattern) => matches_compound(pattern, tag)
            .then(|| tag.clone())
            .into_iter()
            .collect(),
        Node::Index(index) => collection_index(tag, *index).into_iter().collect(),
        Node::AllElements => collection_elements(tag),
        Node::MatchingElement(pattern) => collection_elements(tag)
            .into_iter()
            .filter(|value| matches_compound(pattern, value))
            .collect(),
    }
}

fn collection_elements(tag: &NbtTag) -> Vec<NbtTag> {
    match tag {
        NbtTag::List(values) => values.clone(),
        NbtTag::ByteArray(values) => values.iter().copied().map(NbtTag::Byte).collect(),
        NbtTag::IntArray(values) => values.iter().copied().map(NbtTag::Int).collect(),
        NbtTag::LongArray(values) => values.iter().copied().map(NbtTag::Long).collect(),
        _ => Vec::new(),
    }
}

fn collection_index(tag: &NbtTag, index: i32) -> Option<NbtTag> {
    let values = collection_elements(tag);
    let resolved = if index < 0 {
        i64::try_from(values.len())
            .ok()?
            .checked_add(i64::from(index))?
    } else {
        i64::from(index)
    };
    usize::try_from(resolved)
        .ok()
        .and_then(|i| values.get(i).cloned())
}

fn matches_compound(expected: &NbtCompound, actual: &NbtTag) -> bool {
    let NbtTag::Compound(actual) = actual else {
        return false;
    };
    expected.child_tags.iter().all(|(key, expected_value)| {
        actual
            .get(key)
            .is_some_and(|actual_value| matches_nbt(expected_value, actual_value))
    })
}

fn matches_nbt(expected: &NbtTag, actual: &NbtTag) -> bool {
    match (expected, actual) {
        (NbtTag::Compound(expected), actual) => matches_compound(expected, actual),
        (NbtTag::List(expected), NbtTag::List(actual)) => {
            if expected.is_empty() {
                actual.is_empty()
            } else {
                expected.iter().all(|wanted| {
                    actual
                        .iter()
                        .any(|candidate| matches_nbt(wanted, candidate))
                })
            }
        }
        _ => expected == actual,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_root() -> NbtTag {
        let mut first = NbtCompound::new();
        first.put_string("id", "minecraft:pig".to_owned());
        first.put_int("health", 10);
        let mut second = NbtCompound::new();
        second.put_string("id", "minecraft:cow".to_owned());
        second.put_int("health", 8);
        let mut root = NbtCompound::new();
        root.put("mobs", NbtTag::List(vec![first.into(), second.into()]));
        let mut target = NbtCompound::new();
        target.put_string("id", "minecraft:cow".to_owned());
        target.put_int("health", 8);
        root.put_compound("target", target);
        root.put("ints", NbtTag::IntArray(vec![2, 4, 6]));
        root.put_string("key.with.dot", "quoted".to_owned());
        root.into()
    }

    #[test]
    fn parses_and_selects_vanilla_node_families() {
        let root = sample_root();
        assert_eq!(
            NbtPath::parse("mobs[0].health").unwrap().get(&root),
            vec![NbtTag::Int(10)]
        );
        assert_eq!(
            NbtPath::parse("mobs[-1].id").unwrap().get(&root),
            vec![NbtTag::String("minecraft:cow".into())]
        );
        assert_eq!(
            NbtPath::parse("mobs[].health").unwrap().get(&root),
            vec![NbtTag::Int(10), NbtTag::Int(8)]
        );
        assert_eq!(
            NbtPath::parse("mobs[{id:\"minecraft:cow\"}].health")
                .unwrap()
                .get(&root),
            vec![NbtTag::Int(8)]
        );
        assert_eq!(
            NbtPath::parse("\"key.with.dot\"").unwrap().get(&root),
            vec![NbtTag::String("quoted".into())]
        );
        assert_eq!(
            NbtPath::parse("ints[-1]").unwrap().get(&root),
            vec![NbtTag::Int(6)]
        );
    }

    #[test]
    fn root_and_named_compound_matches_are_partial_recursive_matches() {
        let root = sample_root();
        assert_eq!(
            NbtPath::parse("{mobs:[{health:8}]}.mobs[-1].id")
                .unwrap()
                .get(&root),
            vec![NbtTag::String("minecraft:cow".into())]
        );
        assert_eq!(
            NbtPath::parse("target{id:\"minecraft:cow\"}.health")
                .unwrap()
                .get(&root),
            vec![NbtTag::Int(8)]
        );
        assert!(NbtPath::parse("{mobs:[]}").unwrap().get(&root).is_empty());
    }

    #[test]
    fn invalid_paths_fail_instead_of_returning_partial_results() {
        assert!(NbtPath::parse("").is_err());
        assert!(NbtPath::parse("mobs[").is_err());
        assert!(NbtPath::parse("mobs..health").is_err());
        assert!(NbtPath::parse("mobs health").is_err());
        assert!(NbtPath::parse("mobs{broken").is_err());
    }

    #[test]
    fn remove_supports_every_terminal_node_family() {
        let mut root = sample_root();
        assert_eq!(
            NbtPath::parse("target.health").unwrap().remove(&mut root),
            1
        );
        assert!(
            NbtPath::parse("target.health")
                .unwrap()
                .get(&root)
                .is_empty()
        );

        assert_eq!(
            NbtPath::parse("target{id:\"minecraft:cow\"}")
                .unwrap()
                .remove(&mut root),
            1
        );
        assert!(NbtPath::parse("target").unwrap().get(&root).is_empty());

        assert_eq!(NbtPath::parse("mobs[-1]").unwrap().remove(&mut root), 1);
        assert_eq!(NbtPath::parse("mobs[]").unwrap().get(&root).len(), 1);
        assert_eq!(
            NbtPath::parse("mobs[{id:\"minecraft:pig\"}]")
                .unwrap()
                .remove(&mut root),
            1
        );
        assert!(NbtPath::parse("mobs[]").unwrap().get(&root).is_empty());

        assert_eq!(NbtPath::parse("ints[-1]").unwrap().remove(&mut root), 1);
        assert_eq!(NbtPath::parse("ints[]").unwrap().remove(&mut root), 2);
        assert!(NbtPath::parse("ints[]").unwrap().get(&root).is_empty());
    }

    #[test]
    fn remove_traverses_all_and_matching_intermediate_nodes() {
        let mut root = sample_root();
        assert_eq!(
            NbtPath::parse("mobs[].health").unwrap().remove(&mut root),
            2
        );
        assert!(
            NbtPath::parse("mobs[].health")
                .unwrap()
                .get(&root)
                .is_empty()
        );

        let mut root = sample_root();
        assert_eq!(
            NbtPath::parse("mobs[{id:\"minecraft:cow\"}].health")
                .unwrap()
                .remove(&mut root),
            1
        );
        assert_eq!(
            NbtPath::parse("mobs[].health").unwrap().get(&root),
            vec![NbtTag::Int(10)]
        );

        let mut root = sample_root();
        assert_eq!(
            NbtPath::parse("{target:{id:\"minecraft:cow\"}}.target.health")
                .unwrap()
                .remove(&mut root),
            1
        );
    }

    #[test]
    fn terminal_root_match_and_out_of_range_removals_change_nothing() {
        let mut root = sample_root();
        let original = root.clone();
        assert_eq!(NbtPath::parse("{mobs:[]}").unwrap().remove(&mut root), 0);
        assert_eq!(
            NbtPath::parse("{target:{id:\"minecraft:cow\"}}")
                .unwrap()
                .remove(&mut root),
            0
        );
        assert_eq!(NbtPath::parse("mobs[99]").unwrap().remove(&mut root), 0);
        assert_eq!(root, original);
    }

    #[test]
    fn set_creates_vanilla_preferred_intermediate_parents() {
        let mut root = NbtTag::Compound(NbtCompound::new());
        assert_eq!(
            NbtPath::parse("created.child")
                .unwrap()
                .set(&mut root, NbtTag::Int(7)),
            Ok(1)
        );
        assert_eq!(
            NbtPath::parse("created.child").unwrap().get(&root),
            vec![NbtTag::Int(7)]
        );

        assert_eq!(
            NbtPath::parse("items[].value")
                .unwrap()
                .set(&mut root, NbtTag::String("new".into())),
            Ok(1)
        );
        assert_eq!(
            NbtPath::parse("items[].value").unwrap().get(&root),
            vec![NbtTag::String("new".into())]
        );

        assert_eq!(
            NbtPath::parse("filtered[{kind:\"wanted\"}].value")
                .unwrap()
                .set(&mut root, NbtTag::Byte(1)),
            Ok(1)
        );
        assert_eq!(
            NbtPath::parse("filtered[{kind:\"wanted\"}].value")
                .unwrap()
                .get(&root),
            vec![NbtTag::Byte(1)]
        );
    }

    #[test]
    fn set_counts_only_changed_values_across_node_families() {
        let mut root = sample_root();
        assert_eq!(
            NbtPath::parse("mobs[].health")
                .unwrap()
                .set(&mut root, NbtTag::Int(10)),
            Ok(1)
        );
        assert_eq!(
            NbtPath::parse("mobs[].health").unwrap().get(&root),
            vec![NbtTag::Int(10), NbtTag::Int(10)]
        );
        assert_eq!(
            NbtPath::parse("ints[]")
                .unwrap()
                .set(&mut root, NbtTag::Int(4)),
            Ok(2)
        );
        assert_eq!(
            NbtPath::parse("ints[]").unwrap().get(&root),
            vec![NbtTag::Int(4), NbtTag::Int(4), NbtTag::Int(4)]
        );
        assert_eq!(
            NbtPath::parse("mobs[-1].id")
                .unwrap()
                .set(&mut root, NbtTag::String("minecraft:pig".into())),
            Ok(1)
        );
        assert_eq!(
            NbtPath::parse("missing[0].value")
                .unwrap()
                .set(&mut root, NbtTag::Int(1)),
            Err(NbtMutationError::NothingFound)
        );
    }

    #[test]
    fn terminal_matching_nodes_do_not_create_but_empty_match_list_does() {
        let mut root = NbtTag::Compound(NbtCompound::new());
        assert_eq!(
            NbtPath::parse("missing{kind:\"wanted\"}")
                .unwrap()
                .set(&mut root, NbtTag::Int(1)),
            Ok(0)
        );

        let mut compound = NbtCompound::new();
        compound.put("items", NbtTag::List(Vec::new()));
        let mut root = NbtTag::Compound(compound);
        assert_eq!(
            NbtPath::parse("items[{kind:\"wanted\"}]")
                .unwrap()
                .set(&mut root, NbtTag::String("replacement".into())),
            Ok(1)
        );
        assert_eq!(
            NbtPath::parse("items[0]").unwrap().get(&root),
            vec![NbtTag::String("replacement".into())]
        );
        assert_eq!(
            NbtPath::parse("{items:[]}")
                .unwrap()
                .set(&mut root, NbtTag::Int(2)),
            Ok(0)
        );
    }

    #[test]
    fn insert_supports_offsets_creation_and_multiple_destinations() {
        let mut nested = NbtCompound::new();
        nested.put(
            "lists",
            NbtTag::List(vec![
                NbtTag::List(vec![NbtTag::Int(1)]),
                NbtTag::List(vec![NbtTag::Int(4)]),
            ]),
        );
        let mut root = NbtTag::Compound(nested);
        assert_eq!(
            NbtPath::parse("lists[]").unwrap().insert(
                -1,
                &mut root,
                &[NbtTag::Int(2), NbtTag::Int(3)]
            ),
            Ok(2)
        );
        assert_eq!(
            NbtPath::parse("lists[0][]").unwrap().get(&root),
            vec![NbtTag::Int(1), NbtTag::Int(2), NbtTag::Int(3)]
        );
        assert_eq!(
            NbtPath::parse("created").unwrap().insert(
                0,
                &mut root,
                &[NbtTag::String("value".into())]
            ),
            Ok(1)
        );
        assert_eq!(
            NbtPath::parse("created[0]").unwrap().get(&root),
            vec![NbtTag::String("value".into())]
        );
    }

    #[test]
    fn insert_reports_vanilla_list_and_index_errors() {
        let mut root = sample_root();
        assert!(matches!(
            NbtPath::parse("target")
                .unwrap()
                .insert(0, &mut root, &[NbtTag::Int(1)]),
            Err(NbtMutationError::ExpectedList(_))
        ));
        assert_eq!(
            NbtPath::parse("mobs").unwrap().insert(
                99,
                &mut root,
                &[NbtTag::Compound(NbtCompound::new())]
            ),
            Err(NbtMutationError::InvalidIndex(99))
        );
        assert_eq!(
            NbtPath::parse("mobs").unwrap().insert(
                -4,
                &mut root,
                &[NbtTag::Compound(NbtCompound::new())]
            ),
            Err(NbtMutationError::InvalidIndex(-4))
        );
        assert_eq!(
            NbtPath::parse("missing[0]")
                .unwrap()
                .insert(0, &mut root, &[NbtTag::Int(1)]),
            Err(NbtMutationError::NothingFound)
        );
    }

    #[test]
    fn insert_obeys_list_element_type_and_empty_source_rules() {
        let mut compound = NbtCompound::new();
        compound.put("values", NbtTag::List(vec![NbtTag::Int(1)]));
        let mut root = NbtTag::Compound(compound);
        assert_eq!(
            NbtPath::parse("values").unwrap().insert(
                1,
                &mut root,
                &[NbtTag::String("wrong".into()), NbtTag::Int(2)],
            ),
            Ok(1)
        );
        assert_eq!(
            NbtPath::parse("values[]").unwrap().get(&root),
            vec![NbtTag::Int(1), NbtTag::Int(2)]
        );
        assert_eq!(
            NbtPath::parse("values").unwrap().insert(0, &mut root, &[]),
            Ok(0)
        );
    }

    #[test]
    fn merge_folds_sources_creates_destinations_and_counts_changes() {
        let mut first = NbtCompound::new();
        first.put_int("a", 1);
        let mut second = NbtCompound::new();
        second.put_int("b", 2);
        second.put_int("a", 3);
        let sources = [NbtTag::Compound(first), NbtTag::Compound(second)];

        let mut root = NbtTag::Compound(NbtCompound::new());
        let path = NbtPath::parse("created").unwrap();
        assert_eq!(path.merge(&mut root, &sources), Ok(1));
        let NbtTag::Compound(created) = path.get(&root).pop().unwrap() else {
            panic!("created destination must be a compound")
        };
        assert_eq!(created.get_int("a"), Some(3));
        assert_eq!(created.get_int("b"), Some(2));
        assert_eq!(path.merge(&mut root, &sources), Ok(0));

        let mut list_root = NbtCompound::new();
        list_root.put(
            "items",
            NbtTag::List(vec![
                NbtTag::Compound(NbtCompound::new()),
                NbtTag::Compound(NbtCompound::new()),
            ]),
        );
        let mut list_root = NbtTag::Compound(list_root);
        assert_eq!(
            NbtPath::parse("items[]")
                .unwrap()
                .merge(&mut list_root, &sources),
            Ok(2)
        );
    }

    #[test]
    fn merge_rejects_non_compound_sources_and_destinations() {
        let mut root = sample_root();
        assert!(matches!(
            NbtPath::parse("target")
                .unwrap()
                .merge(&mut root, &[NbtTag::Int(1)]),
            Err(NbtMutationError::ExpectedObject(_))
        ));
        assert!(matches!(
            NbtPath::parse("target.id")
                .unwrap()
                .merge(&mut root, &[NbtTag::Compound(NbtCompound::new())]),
            Err(NbtMutationError::ExpectedObject(_))
        ));
        assert_eq!(
            NbtPath::parse("missing[0]")
                .unwrap()
                .merge(&mut root, &[NbtTag::Compound(NbtCompound::new())]),
            Err(NbtMutationError::NothingFound)
        );
    }
}
