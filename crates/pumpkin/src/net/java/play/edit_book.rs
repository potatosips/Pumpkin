#[allow(clippy::wildcard_imports)]
use super::*;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_world::inventory::Inventory;

fn sign_book_stack(mut source: ItemStack, content: WrittenBookContentImpl) -> ItemStack {
    // Vanilla's ItemStack::transmuteCopy keeps the component patch while
    // changing the item prototype. Remove the editable content and replace it
    // with the signed content without discarding custom names or other patches.
    source.item = &Item::WRITTEN_BOOK;
    source.patch.retain(|(component, _)| {
        *component != DataComponent::WritableBookContent
            && *component != DataComponent::WrittenBookContent
    });
    source
        .patch
        .push((DataComponent::WrittenBookContent, Some(content.to_dyn())));
    source
}

impl JavaClient {
    pub async fn handle_edit_book(&self, player: &Player, packet: SEditBook<'_>) {
        let Ok(slot) = usize::try_from(packet.slot.0) else {
            return;
        };
        if !PlayerInventory::is_valid_hotbar_index(slot) && slot != PlayerInventory::OFF_HAND_SLOT {
            return;
        }

        let source_stack = player.inventory().get_stack(slot).await;
        if source_stack.item.id != Item::WRITABLE_BOOK.id {
            return;
        }

        let pages: Vec<String> = packet
            .pages
            .iter()
            .map(|page| (*page).to_string())
            .collect();

        if let Some(title) = packet.title {
            let content = WrittenBookContentImpl {
                title: title.to_string(),
                filtered_title: None,
                author: player.gameprofile.name.clone(),
                generation: 0,
                pages: pages
                    .into_iter()
                    .map(|page| NbtTag::String(page.into_boxed_str()))
                    .collect(),
                filtered_pages: Vec::new(),
                // Typed packet pages are already literal components and need no
                // command-source resolution. Vanilla marks them resolved here.
                resolved: true,
            };
            let written_book = sign_book_stack(source_stack, content);
            player.inventory().set_stack(slot, written_book).await;
        } else {
            let mut writable_book = source_stack;
            let content = WritableBookContentImpl {
                pages,
                filtered_pages: Vec::new(),
            };
            writable_book
                .patch
                .retain(|(component, _)| *component != DataComponent::WritableBookContent);
            writable_book
                .patch
                .push((DataComponent::WritableBookContent, Some(content.to_dyn())));
            player.inventory().set_stack(slot, writable_book).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use pumpkin_data::data_component_impl::{DamageImpl, DataComponentImpl};

    use super::*;

    #[test]
    fn signing_preserves_unrelated_component_patch_and_marks_content_resolved() {
        let mut source = ItemStack::new(1, &Item::WRITABLE_BOOK);
        source.patch.push((
            DataComponent::Damage,
            Some(DamageImpl { damage: 7 }.to_dyn()),
        ));
        let content = WrittenBookContentImpl {
            title: "Parity".into(),
            filtered_title: None,
            author: "Author".into(),
            generation: 0,
            pages: vec![NbtTag::String("page".into())],
            filtered_pages: vec![None],
            resolved: true,
        };

        let signed = sign_book_stack(source, content);

        assert_eq!(signed.item.id, Item::WRITTEN_BOOK.id);
        assert_eq!(
            signed
                .get_data_component::<DamageImpl>()
                .map(|value| value.damage),
            Some(7)
        );
        assert!(
            signed
                .get_data_component::<WrittenBookContentImpl>()
                .is_some_and(|value| value.resolved)
        );
        assert!(
            signed
                .patch
                .iter()
                .all(|(id, _)| *id != DataComponent::WritableBookContent)
        );
    }
}
