use crate::data_component_impl::DataComponentImpl;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct WritableBookContentImpl {
    pub pages: Vec<String>,
    pub filtered_pages: Vec<Option<String>>,
}
impl WritableBookContentImpl {
    pub fn read_data(tag: &NbtTag) -> Option<Self> {
        let mut pages = Vec::new();
        let mut filtered_pages = Vec::new();
        if let NbtTag::Compound(c) = tag
            && let Some(NbtTag::List(l)) = c.get("pages")
        {
            for item in l {
                if let NbtTag::String(s) = item {
                    pages.push(s.to_string());
                    filtered_pages.push(None);
                } else if let NbtTag::Compound(comp) = item {
                    if let Some(s) = comp.get_string("raw") {
                        pages.push(s.to_string());
                        filtered_pages.push(comp.get_string("filtered").map(str::to_string));
                    }
                }
            }
        }
        Some(Self {
            pages,
            filtered_pages,
        })
    }
}
impl DataComponentImpl for WritableBookContentImpl {
    fn write_data(&self) -> NbtTag {
        let mut compound = NbtCompound::new();
        let pages_tags: Vec<NbtTag> = self
            .pages
            .iter()
            .enumerate()
            .map(|(index, page)| {
                if let Some(Some(filtered)) = self.filtered_pages.get(index) {
                    let mut page_tag = NbtCompound::new();
                    page_tag.put_string("raw", page.clone());
                    page_tag.put_string("filtered", filtered.clone());
                    NbtTag::Compound(page_tag)
                } else {
                    NbtTag::String(page.clone().into_boxed_str())
                }
            })
            .collect();
        compound.put("pages", NbtTag::List(pages_tags));
        NbtTag::Compound(compound)
    }
    default_impl!(WritableBookContent);
}

#[derive(Clone, Debug, PartialEq)]
pub struct WrittenBookContentImpl {
    pub title: String,
    pub filtered_title: Option<String>,
    pub author: String,
    pub generation: i32,
    /// Each page is a complete text-component NBT value. Keeping the original
    /// tag preserves styles, translations, events, and nested components.
    pub pages: Vec<NbtTag>,
    pub filtered_pages: Vec<Option<NbtTag>>,
    pub resolved: bool,
}
impl WrittenBookContentImpl {
    pub fn read_data(tag: &NbtTag) -> Option<Self> {
        let mut pages = Vec::new();
        let mut filtered_pages = Vec::new();
        let mut title = String::new();
        let mut filtered_title = None;
        let mut author = String::new();
        let mut generation = 0;
        let mut resolved = false;
        if let NbtTag::Compound(c) = tag {
            if let Some(s) = c.get_string("title") {
                title = s.to_string();
            } else if let Some(title_tag) = c.get_compound("title") {
                title = title_tag.get_string("raw").unwrap_or_default().to_string();
                filtered_title = title_tag.get_string("filtered").map(str::to_string);
            }
            if let Some(s) = c.get_string("author") {
                author = s.to_string();
            }
            generation = c.get_int("generation").unwrap_or(0);
            resolved = c.get_bool("resolved").unwrap_or(false);
            if let Some(NbtTag::List(l)) = c.get("pages") {
                for item in l {
                    if let NbtTag::Compound(filterable) = item
                        && let Some(raw) = filterable.get("raw")
                    {
                        pages.push(raw.clone());
                        filtered_pages.push(filterable.get("filtered").cloned());
                    } else {
                        // An unfiltered Filterable<Component> stores the component
                        // directly, and that component is commonly a compound.
                        pages.push(item.clone());
                        filtered_pages.push(None);
                    }
                }
            }
        }
        Some(Self {
            title,
            filtered_title,
            author,
            generation,
            pages,
            filtered_pages,
            resolved,
        })
    }
}
impl DataComponentImpl for WrittenBookContentImpl {
    fn write_data(&self) -> NbtTag {
        let mut compound = NbtCompound::new();
        if let Some(filtered_title) = &self.filtered_title {
            let mut title = NbtCompound::new();
            title.put_string("raw", self.title.clone());
            title.put_string("filtered", filtered_title.clone());
            compound.put("title", NbtTag::Compound(title));
        } else {
            compound.put_string("title", self.title.clone());
        }
        compound.put_string("author", self.author.clone());
        compound.put_int("generation", self.generation);
        compound.put_bool("resolved", self.resolved);
        let pages_tags: Vec<NbtTag> = self
            .pages
            .iter()
            .enumerate()
            .map(|(index, page)| {
                if let Some(Some(filtered)) = self.filtered_pages.get(index) {
                    let mut page_tag = NbtCompound::new();
                    page_tag.put("raw", page.clone());
                    page_tag.put("filtered", filtered.clone());
                    NbtTag::Compound(page_tag)
                } else {
                    page.clone()
                }
            })
            .collect();
        compound.put("pages", NbtTag::List(pages_tags));
        NbtTag::Compound(compound)
    }
    default_impl!(WrittenBookContent);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_book_storage_preserves_unfiltered_compound_components() {
        let mut page = NbtCompound::new();
        page.put_string("text", "styled".into());
        page.put_string("color", "gold".into());
        let expected = WrittenBookContentImpl {
            title: "Rich Book".into(),
            filtered_title: None,
            author: "Author".into(),
            generation: 0,
            pages: vec![NbtTag::Compound(page)],
            filtered_pages: vec![None],
            resolved: true,
        };

        let decoded = WrittenBookContentImpl::read_data(&expected.write_data()).unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn written_book_storage_preserves_filtered_component_pairs() {
        let expected = WrittenBookContentImpl {
            title: "Filtered Book".into(),
            filtered_title: Some("Safe Title".into()),
            author: "Author".into(),
            generation: 1,
            pages: vec![NbtTag::String("raw page".into())],
            filtered_pages: vec![Some(NbtTag::String("safe page".into()))],
            resolved: false,
        };

        let decoded = WrittenBookContentImpl::read_data(&expected.write_data()).unwrap();
        assert_eq!(decoded, expected);
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct DebugStickStateImpl;
impl DataComponentImpl for DebugStickStateImpl {
    default_impl!(DebugStickState);
}
