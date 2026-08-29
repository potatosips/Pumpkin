use std::pin::Pin;
use std::sync::Arc;

use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::data_component_impl::CustomNameImpl;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;

pub struct NameTagItem;

impl ItemMetadata for NameTagItem {
    fn ids() -> Box<[u16]> {
        [Item::NAME_TAG.id].into()
    }
}

impl ItemBehaviour for NameTagItem {
    fn use_on_entity<'a>(
        &'a self,
        item: &'a mut ItemStack,
        player: &'a Player,
        entity: Arc<dyn EntityBase>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let Some(name) = item.get_data_component::<CustomNameImpl>() else {
                return;
            };
            let base_entity = entity.get_entity();
            if !can_apply_name_tag(
                base_entity.entity_type.saveable,
                entity.get_living_entity().is_some(),
                base_entity.is_alive(),
            ) {
                return;
            }

            base_entity.set_custom_name(name.name.clone());
            if let Some(mob) = entity.get_mob() {
                mob.get_mob_entity().set_persistence_required(true);
            }
            item.decrement_unless_creative(player.gamemode.load(), 1);
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

const fn can_apply_name_tag(saveable: bool, living: bool, alive: bool) -> bool {
    saveable && living && alive
}

#[cfg(test)]
mod tests {
    use super::can_apply_name_tag;

    #[test]
    fn name_tags_only_apply_to_live_saveable_living_entities() {
        assert!(can_apply_name_tag(true, true, true));
        assert!(!can_apply_name_tag(false, true, true));
        assert!(!can_apply_name_tag(true, false, true));
        assert!(!can_apply_name_tag(true, true, false));
    }
}
