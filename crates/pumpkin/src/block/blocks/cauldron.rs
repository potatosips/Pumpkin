use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, BlockFuture, BlockMetadata, GetComparatorOutputArgs, OnEntityCollisionArgs,
    UseWithItemArgs,
};
use pumpkin_data::Block;
use pumpkin_data::BlockId;
use pumpkin_data::block_properties::{BlockProperties, WaterCauldronLikeProperties};
use pumpkin_data::data_component::DataComponent;
use pumpkin_data::data_component_impl::{BannerPatternsImpl, DyedColorImpl, PotionContentsImpl};
use pumpkin_data::fluid::Fluid;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_data::world::WorldEvent;
use pumpkin_world::world::BlockFlags;

pub struct CauldronBlock;

fn is_submerged(
    world: &crate::world::World,
    position: &pumpkin_util::math::position::BlockPos,
) -> bool {
    Fluid::same_fluid_type(world.get_fluid(&position.up()).id, Fluid::WATER.id)
}

fn content_height(level: u8) -> f64 {
    (6.0 + f64::from(level) * 3.0) / 16.0
}

fn is_water_potion(stack: &ItemStack) -> bool {
    stack.item.id == Item::POTION.id
        && stack
            .get_data_component::<PotionContentsImpl>()
            .is_some_and(|contents| contents.potion_id == Some(0))
}

fn water_potion() -> ItemStack {
    let mut stack = ItemStack::new(1, &Item::POTION);
    stack
        .get_data_component_mut::<PotionContentsImpl>()
        .expect("potions always have potion contents")
        .potion_id = Some(0);
    stack
}

fn remove_dyed_color(stack: &mut ItemStack) -> bool {
    if stack.get_data_component::<DyedColorImpl>().is_none() {
        return false;
    }
    if let Some((_, value)) = stack
        .patch
        .iter_mut()
        .find(|(component, _)| *component == DataComponent::DyedColor)
    {
        *value = None;
    } else {
        stack.patch.push((DataComponent::DyedColor, None));
    }
    true
}

fn is_dyeable_item(item: &Item) -> bool {
    matches!(
        item.id,
        id if id == Item::LEATHER_HELMET.id
            || id == Item::LEATHER_CHESTPLATE.id
            || id == Item::LEATHER_LEGGINGS.id
            || id == Item::LEATHER_BOOTS.id
            || id == Item::LEATHER_HORSE_ARMOR.id
            || id == Item::WOLF_ARMOR.id
    )
}

fn washed_shulker_box(stack: &ItemStack) -> Option<ItemStack> {
    if stack.item.id == Item::SHULKER_BOX.id
        || !stack
            .item
            .has_tag(&pumpkin_data::tag::Item::MINECRAFT_SHULKER_BOXES)
    {
        return None;
    }
    let mut washed = stack.copy_with_count(1);
    washed.item = &Item::SHULKER_BOX;
    Some(washed)
}

fn banner_without_last_pattern(stack: &ItemStack) -> Option<ItemStack> {
    if !stack
        .item
        .has_tag(&pumpkin_data::tag::Item::MINECRAFT_BANNERS)
    {
        return None;
    }
    if stack
        .get_data_component::<BannerPatternsImpl>()?
        .layers
        .is_empty()
    {
        return None;
    }
    let mut cleaned = stack.copy_with_count(1);
    cleaned
        .get_data_component_mut::<BannerPatternsImpl>()?
        .layers
        .pop();
    Some(cleaned)
}

async fn exchange_item(
    input: &mut ItemStack,
    output: ItemStack,
    player: &crate::entity::player::Player,
) {
    if player.gamemode.load() == pumpkin_util::GameMode::Creative {
        if !player.inventory.contains_item(output.item) {
            player.inventory.offer_or_drop_stack(output, player).await;
        }
        return;
    }
    input.decrement(1);
    if input.is_empty() {
        *input = output;
    } else {
        player.inventory.offer_or_drop_stack(output, player).await;
    }
}

impl BlockMetadata for CauldronBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::CAULDRON,
            BlockId::WATER_CAULDRON,
            BlockId::LAVA_CAULDRON,
            BlockId::POWDER_SNOW_CAULDRON,
        ]
        .into()
    }
}

pub(crate) async fn fire_cauldron_change(
    world: &std::sync::Arc<crate::world::World>,
    pos: pumpkin_util::math::position::BlockPos,
    old_level: i32,
    new_level: i32,
    reason: crate::plugin::block::cauldron_level_change::CauldronChangeReason,
    entity: Option<std::sync::Arc<dyn crate::entity::EntityBase>>,
) -> bool {
    let mut event = crate::plugin::block::cauldron_level_change::CauldronLevelChangeEvent {
        block_pos: pos,
        world: world.clone(),
        old_level,
        new_level,
        reason,
        entity,
        cancelled: false,
    };
    if let Some(server) = world.server.upgrade() {
        server.plugin_manager.fire(&server, &mut event).await;
    }
    !event.cancelled
}

pub(crate) async fn fill_from_dripstone(
    world: &std::sync::Arc<crate::world::World>,
    pos: pumpkin_util::math::position::BlockPos,
    fluid: &'static Fluid,
) {
    let state = world.get_block_state(&pos);
    let block = Block::from_state_id(state.id);
    let (old_level, new_level, new_state, event) =
        if Fluid::same_fluid_type(fluid.id, Fluid::WATER.id) {
            if block == &Block::CAULDRON {
                let mut props = WaterCauldronLikeProperties::default(&Block::WATER_CAULDRON);
                props.level = 1;
                (
                    0,
                    1,
                    props.to_state_id(&Block::WATER_CAULDRON),
                    WorldEvent::SoundDripWaterIntoCauldron,
                )
            } else if block == &Block::WATER_CAULDRON {
                let mut props = WaterCauldronLikeProperties::from_state_id(state.id, block);
                if props.level >= 3 {
                    return;
                }
                let old = props.level;
                props.level += 1;
                (
                    old,
                    props.level,
                    props.to_state_id(block),
                    WorldEvent::SoundDripWaterIntoCauldron,
                )
            } else {
                return;
            }
        } else if Fluid::same_fluid_type(fluid.id, Fluid::LAVA.id) && block == &Block::CAULDRON {
            (
                0,
                3,
                Block::LAVA_CAULDRON.default_state.id,
                WorldEvent::SoundDripLavaIntoCauldron,
            )
        } else {
            return;
        };

    if fire_cauldron_change(
        world,
        pos,
        old_level.into(),
        new_level.into(),
        crate::plugin::block::cauldron_level_change::CauldronChangeReason::NaturalFill,
        None,
    )
    .await
    {
        world
            .set_block_state(&pos, new_state, BlockFlags::NOTIFY_ALL)
            .await;
        world.sync_world_event(event, pos, 0);
    }
}

pub(crate) async fn fill_from_precipitation(
    world: &std::sync::Arc<crate::world::World>,
    pos: pumpkin_util::math::position::BlockPos,
    snow: bool,
) {
    let state = world.get_block_state(&pos);
    let block = Block::from_state_id(state.id);
    let target = if snow {
        &Block::POWDER_SNOW_CAULDRON
    } else {
        &Block::WATER_CAULDRON
    };
    let (old_level, mut props) = if block == &Block::CAULDRON {
        (0, WaterCauldronLikeProperties::default(target))
    } else if block == target {
        let props = WaterCauldronLikeProperties::from_state_id(state.id, block);
        if props.level >= 3 {
            return;
        }
        (props.level, props)
    } else {
        return;
    };
    props.level = old_level + 1;
    if fire_cauldron_change(
        world,
        pos,
        old_level.into(),
        props.level.into(),
        crate::plugin::block::cauldron_level_change::CauldronChangeReason::NaturalFill,
        None,
    )
    .await
    {
        world
            .set_block_state(&pos, props.to_state_id(target), BlockFlags::NOTIFY_ALL)
            .await;
    }
}

impl BlockBehaviour for CauldronBlock {
    #[allow(clippy::too_many_lines)]
    fn use_with_item<'a>(
        &'a self,
        args: UseWithItemArgs<'a>,
    ) -> BlockFuture<'a, BlockActionResult> {
        Box::pin(async move {
            let item_id = args.item_stack.item.id;
            let block_id = args.block.id;
            // Filling empty cauldron with buckets
            if block_id == BlockId::CAULDRON {
                if item_id == Item::WATER_BUCKET.id {
                    if !fire_cauldron_change(
                        args.world,
                        *args.position,
                        0,
                        3,
                        crate::plugin::block::cauldron_level_change::CauldronChangeReason::BucketEmpty,
                        Some(args.player.clone()),
                    )
                    .await
                    {
                        return BlockActionResult::Pass;
                    }
                    let state_id = Block::WATER_CAULDRON
                        .from_properties(&[("level", "3")])
                        .to_state_id(&Block::WATER_CAULDRON);
                    args.world
                        .set_block_state(args.position, state_id, BlockFlags::NOTIFY_ALL)
                        .await;
                    args.world.play_sound(
                        Sound::ItemBucketEmpty,
                        SoundCategory::Blocks,
                        &args.position.to_f64(),
                    );
                    exchange_item(
                        args.item_stack,
                        ItemStack::new(1, &Item::BUCKET),
                        args.player.as_ref(),
                    )
                    .await;
                    return BlockActionResult::Success;
                } else if item_id == Item::LAVA_BUCKET.id {
                    if is_submerged(args.world, args.position) {
                        return BlockActionResult::Success;
                    }
                    if !fire_cauldron_change(
                        args.world,
                        *args.position,
                        0,
                        3,
                        crate::plugin::block::cauldron_level_change::CauldronChangeReason::BucketEmpty,
                        Some(args.player.clone()),
                    )
                    .await
                    {
                        return BlockActionResult::Pass;
                    }
                    args.world
                        .set_block_state(
                            args.position,
                            Block::LAVA_CAULDRON.default_state.id,
                            BlockFlags::NOTIFY_ALL,
                        )
                        .await;
                    args.world.play_sound(
                        Sound::ItemBucketEmptyLava,
                        SoundCategory::Blocks,
                        &args.position.to_f64(),
                    );
                    exchange_item(
                        args.item_stack,
                        ItemStack::new(1, &Item::BUCKET),
                        args.player.as_ref(),
                    )
                    .await;
                    return BlockActionResult::Success;
                } else if item_id == Item::POWDER_SNOW_BUCKET.id {
                    if is_submerged(args.world, args.position) {
                        return BlockActionResult::Success;
                    }
                    if !fire_cauldron_change(
                        args.world,
                        *args.position,
                        0,
                        3,
                        crate::plugin::block::cauldron_level_change::CauldronChangeReason::BucketEmpty,
                        Some(args.player.clone()),
                    )
                    .await
                    {
                        return BlockActionResult::Pass;
                    }
                    let state_id = Block::POWDER_SNOW_CAULDRON
                        .from_properties(&[("level", "3")])
                        .to_state_id(&Block::POWDER_SNOW_CAULDRON);
                    args.world
                        .set_block_state(args.position, state_id, BlockFlags::NOTIFY_ALL)
                        .await;
                    args.world.play_sound(
                        Sound::ItemBucketEmptyPowderSnow,
                        SoundCategory::Blocks,
                        &args.position.to_f64(),
                    );
                    exchange_item(
                        args.item_stack,
                        ItemStack::new(1, &Item::BUCKET),
                        args.player.as_ref(),
                    )
                    .await;
                    return BlockActionResult::Success;
                } else if is_water_potion(args.item_stack) {
                    if !fire_cauldron_change(
                        args.world,
                        *args.position,
                        0,
                        1,
                        crate::plugin::block::cauldron_level_change::CauldronChangeReason::BottleEmpty,
                        Some(args.player.clone()),
                    )
                    .await
                    {
                        return BlockActionResult::Pass;
                    }
                    let state_id = Block::WATER_CAULDRON
                        .from_properties(&[("level", "1")])
                        .to_state_id(&Block::WATER_CAULDRON);
                    args.world
                        .set_block_state(args.position, state_id, BlockFlags::NOTIFY_ALL)
                        .await;
                    args.world.play_sound(
                        Sound::ItemBottleEmpty,
                        SoundCategory::Blocks,
                        &args.position.to_f64(),
                    );
                    exchange_item(
                        args.item_stack,
                        ItemStack::new(1, &Item::GLASS_BOTTLE),
                        args.player.as_ref(),
                    )
                    .await;
                    return BlockActionResult::Success;
                }
            }

            // Filling a glass bottle lowers a water cauldron by exactly one level.
            if block_id == BlockId::WATER_CAULDRON && item_id == Item::GLASS_BOTTLE.id {
                let state_id = args.world.get_block_state_id(args.position);
                let props = WaterCauldronLikeProperties::from_state_id(state_id, args.block);
                let new_level = i32::from(props.level) - 1;
                if !fire_cauldron_change(
                    args.world,
                    *args.position,
                    i32::from(props.level),
                    new_level,
                    crate::plugin::block::cauldron_level_change::CauldronChangeReason::BottleFill,
                    Some(args.player.clone()),
                )
                .await
                {
                    return BlockActionResult::Pass;
                }
                let new_state_id = if new_level == 0 {
                    Block::CAULDRON.default_state.id
                } else {
                    let level = new_level.to_string();
                    Block::WATER_CAULDRON
                        .from_properties(&[("level", level.as_str())])
                        .to_state_id(&Block::WATER_CAULDRON)
                };
                args.world
                    .set_block_state(args.position, new_state_id, BlockFlags::NOTIFY_ALL)
                    .await;
                args.world.play_sound(
                    Sound::ItemBottleFill,
                    SoundCategory::Blocks,
                    &args.position.to_f64(),
                );
                exchange_item(args.item_stack, water_potion(), args.player.as_ref()).await;
                return BlockActionResult::Success;
            }

            if block_id == BlockId::WATER_CAULDRON {
                let state_id = args.world.get_block_state_id(args.position);
                let props = WaterCauldronLikeProperties::from_state_id(state_id, args.block);

                let washed_item = washed_shulker_box(args.item_stack);
                let cleaned_banner = banner_without_last_pattern(args.item_stack);
                let is_dyed = is_dyeable_item(args.item_stack.item)
                    && args
                        .item_stack
                        .get_data_component::<DyedColorImpl>()
                        .is_some();
                if washed_item.is_some() || cleaned_banner.is_some() || is_dyed {
                    let new_level = i32::from(props.level) - 1;
                    if !fire_cauldron_change(
                        args.world,
                        *args.position,
                        i32::from(props.level),
                        new_level,
                        crate::plugin::block::cauldron_level_change::CauldronChangeReason::Unknown,
                        Some(args.player.clone()),
                    )
                    .await
                    {
                        return BlockActionResult::Pass;
                    }

                    if let Some(washed) = washed_item.or(cleaned_banner) {
                        // Vanilla replaces one colored box with an undyed box, preserving
                        // block-entity data and every unrelated data component.
                        if args.item_stack.item_count == 1 {
                            *args.item_stack = washed;
                        } else {
                            args.item_stack.decrement(1);
                            args.player
                                .inventory
                                .offer_or_drop_stack(washed, args.player.as_ref())
                                .await;
                        }
                    } else {
                        remove_dyed_color(args.item_stack);
                    }

                    let new_state_id = if new_level == 0 {
                        Block::CAULDRON.default_state.id
                    } else {
                        let level = new_level.to_string();
                        Block::WATER_CAULDRON
                            .from_properties(&[("level", level.as_str())])
                            .to_state_id(&Block::WATER_CAULDRON)
                    };
                    args.world
                        .set_block_state(args.position, new_state_id, BlockFlags::NOTIFY_ALL)
                        .await;
                    return BlockActionResult::Success;
                }
            }

            // Collecting fluid from full cauldrons into empty bucket
            if item_id == Item::BUCKET.id {
                let state_id = args.world.get_block_state_id(args.position);
                let (filled_item, sound) = if block_id == BlockId::WATER_CAULDRON {
                    let props = WaterCauldronLikeProperties::from_state_id(state_id, args.block);
                    if props.level == 3 {
                        (Some(&Item::WATER_BUCKET), Sound::ItemBucketFill)
                    } else {
                        (None, Sound::ItemBucketFill)
                    }
                } else if block_id == BlockId::LAVA_CAULDRON {
                    (Some(&Item::LAVA_BUCKET), Sound::ItemBucketFillLava)
                } else if block_id == BlockId::POWDER_SNOW_CAULDRON {
                    let props = WaterCauldronLikeProperties::from_state_id(state_id, args.block);
                    if props.level == 3 {
                        (
                            Some(&Item::POWDER_SNOW_BUCKET),
                            Sound::ItemBucketFillPowderSnow,
                        )
                    } else {
                        (None, Sound::ItemBucketFillPowderSnow)
                    }
                } else {
                    (None, Sound::ItemBucketFill)
                };

                if let Some(result_item) = filled_item {
                    if !fire_cauldron_change(
                        args.world,
                        *args.position,
                        3,
                        0,
                        crate::plugin::block::cauldron_level_change::CauldronChangeReason::BucketFill,
                        Some(args.player.clone()),
                    )
                    .await
                    {
                        return BlockActionResult::Pass;
                    }
                    args.world
                        .set_block_state(
                            args.position,
                            Block::CAULDRON.default_state.id,
                            BlockFlags::NOTIFY_ALL,
                        )
                        .await;
                    args.world
                        .play_sound(sound, SoundCategory::Blocks, &args.position.to_f64());
                    exchange_item(
                        args.item_stack,
                        ItemStack::new(1, result_item),
                        args.player.as_ref(),
                    )
                    .await;
                    return BlockActionResult::Success;
                }
            }

            // Adding water bottle to non-full water cauldron
            if block_id == BlockId::WATER_CAULDRON && is_water_potion(args.item_stack) {
                let state_id = args.world.get_block_state_id(args.position);
                let props = WaterCauldronLikeProperties::from_state_id(state_id, args.block);
                if props.level < 3 {
                    if !fire_cauldron_change(
                        args.world,
                        *args.position,
                        i32::from(props.level),
                        i32::from(props.level) + 1,
                        crate::plugin::block::cauldron_level_change::CauldronChangeReason::BottleEmpty,
                        Some(args.player.clone()),
                    )
                    .await
                    {
                        return BlockActionResult::Pass;
                    }
                    let next_level_str = match props.level {
                        1 => "2",
                        _ => "3",
                    };
                    let new_state_id = Block::WATER_CAULDRON
                        .from_properties(&[("level", next_level_str)])
                        .to_state_id(&Block::WATER_CAULDRON);
                    args.world
                        .set_block_state(args.position, new_state_id, BlockFlags::NOTIFY_ALL)
                        .await;
                    args.world.play_sound(
                        Sound::ItemBottleEmpty,
                        SoundCategory::Blocks,
                        &args.position.to_f64(),
                    );
                    exchange_item(
                        args.item_stack,
                        ItemStack::new(1, &Item::GLASS_BOTTLE),
                        args.player.as_ref(),
                    )
                    .await;
                    return BlockActionResult::Success;
                }
            }

            BlockActionResult::PassToDefaultBlockAction
        })
    }

    fn on_entity_collision<'a>(&'a self, args: OnEntityCollisionArgs<'a>) -> BlockFuture<'a, ()> {
        Box::pin(async move {
            if !matches!(
                args.block.id,
                BlockId::WATER_CAULDRON | BlockId::POWDER_SNOW_CAULDRON
            ) || args
                .entity
                .get_entity()
                .fire_ticks
                .load(std::sync::atomic::Ordering::Relaxed)
                <= 0
            {
                return;
            }

            let props = WaterCauldronLikeProperties::from_state_id(args.state.id, args.block);
            let surface_y = f64::from(args.position.0.y) + content_height(props.level);
            if args.entity.get_entity().bounding_box.load().min.y > surface_y {
                return;
            }

            let new_level = i32::from(props.level) - 1;
            let event_entity = args
                .world
                .get_entity_by_id(args.entity.get_entity().entity_id);
            if !fire_cauldron_change(
                args.world,
                *args.position,
                i32::from(props.level),
                new_level,
                crate::plugin::block::cauldron_level_change::CauldronChangeReason::Extinguish,
                event_entity,
            )
            .await
            {
                return;
            }

            args.entity.get_entity().extinguish();
            args.entity.get_entity().set_on_fire(false).await;
            // Powder snow first melts to water at the same level, then Vanilla's
            // shared lowerFillLevel path consumes one level for extinguishing.
            let new_state_id = if new_level == 0 {
                Block::CAULDRON.default_state.id
            } else {
                let level = new_level.to_string();
                Block::WATER_CAULDRON
                    .from_properties(&[("level", level.as_str())])
                    .to_state_id(&Block::WATER_CAULDRON)
            };
            args.world
                .set_block_state(args.position, new_state_id, BlockFlags::NOTIFY_ALL)
                .await;
        })
    }

    fn get_comparator_output<'a>(
        &'a self,
        args: GetComparatorOutputArgs<'a>,
    ) -> BlockFuture<'a, Option<u8>> {
        Box::pin(async move {
            match args.block.id {
                BlockId::WATER_CAULDRON | BlockId::POWDER_SNOW_CAULDRON => {
                    let state_id = args.world.get_block_state_id(args.position);
                    let props = WaterCauldronLikeProperties::from_state_id(state_id, args.block);
                    Some(props.level)
                }
                BlockId::LAVA_CAULDRON => Some(3),
                _ => Some(0),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        banner_without_last_pattern, content_height, is_water_potion, remove_dyed_color,
        washed_shulker_box,
    };
    use pumpkin_data::data_component::DataComponent;
    use pumpkin_data::data_component_impl::{
        BannerPatternLayer, BannerPatternsImpl, DataComponentImpl, DyedColorImpl,
        PotionContentsImpl,
    };
    use pumpkin_data::item::Item;
    use pumpkin_data::item_stack::ItemStack;

    #[test]
    fn only_water_potions_fill_cauldron() {
        let mut water = ItemStack::new(1, &Item::POTION);
        water
            .get_data_component_mut::<PotionContentsImpl>()
            .unwrap()
            .potion_id = Some(0);
        let mut healing = ItemStack::new(1, &Item::POTION);
        healing
            .get_data_component_mut::<PotionContentsImpl>()
            .unwrap()
            .potion_id = Some(10);

        assert!(is_water_potion(&water));
        assert!(!is_water_potion(&healing));
        assert!(!is_water_potion(&ItemStack::new(1, &Item::POTION)));
        assert!(!is_water_potion(&ItemStack::new(1, &Item::GLASS_BOTTLE)));
    }

    #[test]
    fn dyed_color_is_removed_without_touching_the_item() {
        let mut armor = ItemStack::new(1, &Item::LEATHER_CHESTPLATE);
        armor.patch.push((
            DataComponent::DyedColor,
            Some(DyedColorImpl { rgb: 0x123456 }.to_dyn()),
        ));
        assert!(remove_dyed_color(&mut armor));
        assert_eq!(armor.item.id, Item::LEATHER_CHESTPLATE.id);
        assert!(armor.get_data_component::<DyedColorImpl>().is_none());
        assert!(!remove_dyed_color(&mut armor));
    }

    #[test]
    fn colored_shulker_washes_to_undyed_and_keeps_components() {
        let mut colored = ItemStack::new(3, &Item::RED_SHULKER_BOX);
        colored.patch.push((
            DataComponent::DyedColor,
            Some(DyedColorImpl { rgb: 42 }.to_dyn()),
        ));
        let washed = washed_shulker_box(&colored).unwrap();
        assert_eq!(washed.item.id, Item::SHULKER_BOX.id);
        assert_eq!(washed.item_count, 1);
        assert_eq!(washed.patch.len(), colored.patch.len());
        assert_eq!(
            washed
                .get_data_component::<DyedColorImpl>()
                .map(|component| component.rgb),
            Some(42)
        );
        assert!(washed_shulker_box(&ItemStack::new(1, &Item::SHULKER_BOX)).is_none());
    }

    #[test]
    fn banner_washing_removes_only_the_top_pattern() {
        let mut banner = ItemStack::new(4, &Item::WHITE_BANNER);
        banner.patch.push((
            DataComponent::BannerPatterns,
            Some(
                BannerPatternsImpl {
                    layers: vec![
                        BannerPatternLayer {
                            pattern: "minecraft:stripe_bottom".to_string(),
                            color: "red".to_string(),
                        },
                        BannerPatternLayer {
                            pattern: "minecraft:circle".to_string(),
                            color: "blue".to_string(),
                        },
                    ],
                }
                .to_dyn(),
            ),
        ));
        let cleaned = banner_without_last_pattern(&banner).unwrap();
        let layers = &cleaned
            .get_data_component::<BannerPatternsImpl>()
            .unwrap()
            .layers;
        assert_eq!(cleaned.item_count, 1);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].pattern, "minecraft:stripe_bottom");
        assert_eq!(layers[0].color, "red");
    }

    #[test]
    fn layered_cauldron_surface_matches_vanilla() {
        assert_eq!(content_height(1), 9.0 / 16.0);
        assert_eq!(content_height(2), 12.0 / 16.0);
        assert_eq!(content_height(3), 15.0 / 16.0);
    }
}
