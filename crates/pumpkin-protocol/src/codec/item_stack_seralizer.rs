use crate::VarInt;
use crate::codec::data_component::{
    deserialize, deserialize_for_version, serialize, serialize_for_version,
};
use crate::ser::{NetworkReadExt, NetworkWriteExt, ReadingError, WritingError};
use pumpkin_data::data_component::DataComponent;
use pumpkin_data::data_component_impl::{CustomNameImpl, DataComponentImpl, ItemNameImpl};
use pumpkin_data::item::Item;
use pumpkin_data::item_id_remap::{remap_item_id_for_version, remap_item_id_from_version};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::version::JavaMinecraftVersion;
use std::borrow::Cow;
use std::io::Cursor;

#[derive(Clone)]
pub struct ItemStackSerializer<'a>(pub Cow<'a, ItemStack>);

fn component_id_for_version(
    id: DataComponent,
    version: Option<&JavaMinecraftVersion>,
) -> Option<u8> {
    if version != Some(&JavaMinecraftVersion::V_1_21_4) {
        return Some(id.to_id());
    }

    Some(match id {
        DataComponent::CustomData => 0,
        DataComponent::MaxStackSize => 1,
        DataComponent::MaxDamage => 2,
        DataComponent::Damage => 3,
        DataComponent::Unbreakable => 4,
        DataComponent::CustomName => 5,
        DataComponent::ItemName => 6,
        DataComponent::ItemModel => 7,
        DataComponent::Lore => 8,
        DataComponent::Rarity => 9,
        DataComponent::Enchantments => 10,
        DataComponent::CanPlaceOn => 11,
        DataComponent::CanBreak => 12,
        DataComponent::AttributeModifiers => 13,
        DataComponent::CustomModelData => 14,
        DataComponent::RepairCost => 17,
        DataComponent::CreativeSlotLock => 18,
        DataComponent::EnchantmentGlintOverride => 19,
        DataComponent::IntangibleProjectile => 20,
        DataComponent::Food => 21,
        DataComponent::Consumable => 22,
        DataComponent::UseRemainder => 23,
        DataComponent::UseCooldown => 24,
        DataComponent::DamageResistant => 25,
        DataComponent::Tool => 26,
        DataComponent::Enchantable => 27,
        DataComponent::Equippable => 28,
        DataComponent::Repairable => 29,
        DataComponent::Glider => 30,
        DataComponent::TooltipStyle => 31,
        DataComponent::DeathProtection => 32,
        DataComponent::StoredEnchantments => 33,
        DataComponent::DyedColor => 34,
        DataComponent::MapColor => 35,
        DataComponent::MapId => 36,
        DataComponent::MapDecorations => 37,
        DataComponent::MapPostProcessing => 38,
        DataComponent::ChargedProjectiles => 39,
        DataComponent::BundleContents => 40,
        DataComponent::PotionContents => 41,
        DataComponent::SuspiciousStewEffects => 42,
        DataComponent::WritableBookContent => 43,
        DataComponent::WrittenBookContent => 44,
        DataComponent::Trim => 45,
        DataComponent::DebugStickState => 46,
        DataComponent::EntityData => 47,
        DataComponent::BucketEntityData => 48,
        DataComponent::BlockEntityData => 49,
        DataComponent::Instrument => 50,
        DataComponent::OminousBottleAmplifier => 51,
        DataComponent::JukeboxPlayable => 52,
        DataComponent::Recipes => 53,
        DataComponent::LodestoneTracker => 54,
        DataComponent::FireworkExplosion => 55,
        DataComponent::Fireworks => 56,
        DataComponent::Profile => 57,
        DataComponent::NoteBlockSound => 58,
        DataComponent::BannerPatterns => 59,
        DataComponent::BaseColor => 60,
        DataComponent::PotDecorations => 61,
        DataComponent::Container => 62,
        DataComponent::BlockState => 63,
        DataComponent::Bees => 64,
        DataComponent::Lock => 65,
        DataComponent::ContainerLoot => 66,
        _ => return None,
    })
}

fn component_id_from_version(id: u8, version: &JavaMinecraftVersion) -> Option<DataComponent> {
    if *version != JavaMinecraftVersion::V_1_21_4 {
        return DataComponent::try_from_id(id);
    }

    Some(match id {
        0 => DataComponent::CustomData,
        1 => DataComponent::MaxStackSize,
        2 => DataComponent::MaxDamage,
        3 => DataComponent::Damage,
        4 => DataComponent::Unbreakable,
        5 => DataComponent::CustomName,
        6 => DataComponent::ItemName,
        7 => DataComponent::ItemModel,
        8 => DataComponent::Lore,
        9 => DataComponent::Rarity,
        10 => DataComponent::Enchantments,
        11 => DataComponent::CanPlaceOn,
        12 => DataComponent::CanBreak,
        13 => DataComponent::AttributeModifiers,
        14 => DataComponent::CustomModelData,
        // 15 and 16 are the removed, zero-payload HideAdditionalTooltip and
        // HideTooltip components. Callers handle them as ignorable legacy IDs.
        15 | 16 => return None,
        17 => DataComponent::RepairCost,
        18 => DataComponent::CreativeSlotLock,
        19 => DataComponent::EnchantmentGlintOverride,
        20 => DataComponent::IntangibleProjectile,
        21 => DataComponent::Food,
        22 => DataComponent::Consumable,
        23 => DataComponent::UseRemainder,
        24 => DataComponent::UseCooldown,
        25 => DataComponent::DamageResistant,
        26 => DataComponent::Tool,
        27 => DataComponent::Enchantable,
        28 => DataComponent::Equippable,
        29 => DataComponent::Repairable,
        30 => DataComponent::Glider,
        31 => DataComponent::TooltipStyle,
        32 => DataComponent::DeathProtection,
        33 => DataComponent::StoredEnchantments,
        34 => DataComponent::DyedColor,
        35 => DataComponent::MapColor,
        36 => DataComponent::MapId,
        37 => DataComponent::MapDecorations,
        38 => DataComponent::MapPostProcessing,
        39 => DataComponent::ChargedProjectiles,
        40 => DataComponent::BundleContents,
        41 => DataComponent::PotionContents,
        42 => DataComponent::SuspiciousStewEffects,
        43 => DataComponent::WritableBookContent,
        44 => DataComponent::WrittenBookContent,
        45 => DataComponent::Trim,
        46 => DataComponent::DebugStickState,
        47 => DataComponent::EntityData,
        48 => DataComponent::BucketEntityData,
        49 => DataComponent::BlockEntityData,
        50 => DataComponent::Instrument,
        51 => DataComponent::OminousBottleAmplifier,
        52 => DataComponent::JukeboxPlayable,
        53 => DataComponent::Recipes,
        54 => DataComponent::LodestoneTracker,
        55 => DataComponent::FireworkExplosion,
        56 => DataComponent::Fireworks,
        57 => DataComponent::Profile,
        58 => DataComponent::NoteBlockSound,
        59 => DataComponent::BannerPatterns,
        60 => DataComponent::BaseColor,
        61 => DataComponent::PotDecorations,
        62 => DataComponent::Container,
        63 => DataComponent::BlockState,
        64 => DataComponent::Bees,
        65 => DataComponent::Lock,
        66 => DataComponent::ContainerLoot,
        _ => return None,
    })
}

fn item_component_counts(stack: &ItemStack, version: Option<&JavaMinecraftVersion>) -> (u8, u8) {
    let mut to_add = 0u8;
    let mut to_remove = 0u8;

    for (_id, data) in &stack.patch {
        if component_id_for_version(*_id, version).is_none() {
            continue;
        }
        if data.is_none() {
            to_remove += 1;
        } else {
            to_add += 1;
        }
    }

    (to_add, to_remove)
}

fn serialize_any_item_stack_with_id(
    stack: &ItemStack,
    item_id: u16,
    is_template: bool,
    version: Option<&JavaMinecraftVersion>,
    write: &mut impl NetworkWriteExt,
) -> Result<(), WritingError> {
    if stack.is_empty() {
        write.put_var_int(&VarInt(0))
    } else {
        let (to_add, to_remove) = item_component_counts(stack, version);
        if is_template {
            write.put_var_int(&VarInt::from(item_id))?;
            write.put_var_int(&VarInt::from(stack.item_count))?;
        } else {
            write.put_var_int(&VarInt::from(stack.item_count))?;
            write.put_var_int(&VarInt::from(item_id))?;
        }
        write.put_var_int(&VarInt::from(to_add))?;
        write.put_var_int(&VarInt::from(to_remove))?;

        for (id, data) in &stack.patch {
            if let Some(data) = data
                && let Some(component_id) = component_id_for_version(*id, version)
            {
                write.put_var_int(&VarInt::from(component_id))?;
                if let Some(version) = version {
                    serialize_for_version(*id, data.as_ref(), version, write)?;
                } else {
                    serialize(*id, data.as_ref(), write)?;
                }
            }
        }

        for (id, data) in &stack.patch {
            if data.is_none()
                && let Some(component_id) = component_id_for_version(*id, version)
            {
                write.put_var_int(&VarInt::from(component_id))?;
            }
        }

        Ok(())
    }
}

fn serialize_item_stack_with_id(
    stack: &ItemStack,
    item_id: u16,
    version: Option<&JavaMinecraftVersion>,
    write: &mut impl NetworkWriteExt,
) -> Result<(), WritingError> {
    serialize_any_item_stack_with_id(stack, item_id, false, version, write)
}

fn serialize_item_cost_with_id(
    stack: &ItemStack,
    item_id: u16,
    write: &mut impl NetworkWriteExt,
) -> Result<(), WritingError> {
    let component_count = stack
        .patch
        .iter()
        .filter(|(_, data)| data.is_some())
        .count();
    let component_count = i32::try_from(component_count)
        .map_err(|_| WritingError::Message("Too many item cost components".into()))?;

    write.put_var_int(&VarInt::from(item_id))?;
    write.put_var_int(&VarInt::from(stack.item_count))?;
    write.put_var_int(&VarInt(component_count))?;
    for (id, data) in &stack.patch {
        if let Some(data) = data {
            write.put_var_int(&VarInt::from(id.to_id()))?;
            serialize(*id, data.as_ref(), write)?;
        }
    }
    Ok(())
}

fn read_component_id(read: &mut impl NetworkReadExt) -> Result<DataComponent, ReadingError> {
    let id_val = read.get_var_int()?.0;
    let id_u8 = id_val
        .try_into()
        .map_err(|_| ReadingError::Message(format!("Invalid component ID: {id_val}")))?;
    DataComponent::try_from_id(id_u8)
        .ok_or_else(|| ReadingError::Message(format!("Unknown component ID: {id_val}")))
}

fn decode_custom_name(component_data: &[u8]) -> Result<Box<dyn DataComponentImpl>, ReadingError> {
    let mut cursor = Cursor::new(component_data);
    let mut nbt_reader = pumpkin_nbt::deserializer::NbtReadHelperJava::new(&mut cursor);
    let tag = NbtTag::deserialize(&mut nbt_reader)
        .map_err(|err| ReadingError::Message(format!("Failed to decode CustomName NBT: {err}")))?;
    CustomNameImpl::read_data(&tag)
        .map(DataComponentImpl::to_dyn)
        .ok_or_else(|| ReadingError::Message("Invalid CustomName component NBT".into()))
}

fn decode_item_name(component_data: &[u8]) -> Result<Box<dyn DataComponentImpl>, ReadingError> {
    let mut cursor = Cursor::new(component_data);
    let mut nbt_reader = pumpkin_nbt::deserializer::NbtReadHelperJava::new(&mut cursor);
    let tag = NbtTag::deserialize(&mut nbt_reader)
        .map_err(|err| ReadingError::Message(format!("Failed to decode ItemName NBT: {err}")))?;
    let name = match tag {
        NbtTag::String(name) => name.to_string(),
        NbtTag::Compound(compound) => compound
            .get_string("translate")
            .or_else(|| compound.get_string("text"))
            .unwrap_or_default()
            .to_owned(),
        _ => String::new(),
    };
    Ok(ItemNameImpl {
        name: Cow::Owned(name),
    }
    .to_dyn())
}

fn decode_component(
    id: DataComponent,
    component_data: &[u8],
) -> Result<Box<dyn DataComponentImpl>, ReadingError> {
    match id {
        DataComponent::CustomName => decode_custom_name(component_data),
        DataComponent::ItemName => decode_item_name(component_data),
        _ => {
            let mut cursor = Cursor::new(component_data);
            deserialize(id, &mut cursor)
        }
    }
}

fn read_length_prefixed_component(
    read: &mut impl NetworkReadExt,
) -> Result<(DataComponent, Box<dyn DataComponentImpl>), ReadingError> {
    let id = read_component_id(read)?;
    let byte_len = read.get_var_int()?.0;
    let byte_len: usize = byte_len
        .try_into()
        .map_err(|_| ReadingError::Message("Negative component data length".into()))?;
    if byte_len > crate::MAX_PACKET_DATA_SIZE {
        return Err(ReadingError::TooLarge("Component data too large".into()));
    }

    let component_impl = if byte_len <= 256 {
        let mut stack_buf = [0u8; 256];
        let slice = &mut stack_buf[..byte_len];
        read.read_bytes_to_buf(slice)?;
        decode_component(id, slice)?
    } else {
        let mut component_data = vec![0u8; byte_len];
        read.read_bytes_to_buf(&mut component_data)?;
        decode_component(id, &component_data)?
    };

    Ok((id, component_impl))
}

impl ItemStackSerializer<'_> {
    pub fn read(
        read: &mut impl NetworkReadExt,
    ) -> Result<ItemStackSerializer<'static>, ReadingError> {
        const MAX_COMPONENTS: i32 = 256;

        let item_count = read.get_var_int()?;
        if item_count.0 == 0 {
            return Ok(ItemStackSerializer(Cow::Borrowed(ItemStack::EMPTY)));
        }

        let item_id = read.get_var_int()?;
        let num_to_add = read.get_var_int()?.0;
        let num_to_remove = read.get_var_int()?.0;

        if num_to_add < 0 || num_to_remove < 0 {
            return Err(ReadingError::Message("Negative component count".into()));
        }

        let total_components = num_to_add
            .checked_add(num_to_remove)
            .ok_or_else(|| ReadingError::Message("Component count overflow".into()))?;

        if total_components > MAX_COMPONENTS {
            return Err(ReadingError::Message(
                "Too many components in ItemStack patch".into(),
            ));
        }

        let mut patch = Vec::with_capacity((num_to_add + num_to_remove) as usize);

        for _ in 0..num_to_add {
            let id_val = read.get_var_int()?.0;
            let id = DataComponent::try_from_id(id_val as u8)
                .ok_or_else(|| ReadingError::Message(format!("Unknown component ID: {id_val}")))?;

            let _byte_len = read.get_var_int()?;

            let component_impl = deserialize(id, read)?;
            patch.push((id, Some(component_impl)));
        }

        for _ in 0..num_to_remove {
            let id_val = read.get_var_int()?.0;
            let id = DataComponent::try_from_id(id_val as u8)
                .ok_or_else(|| ReadingError::Message("Unknown component ID".into()))?;
            patch.push((id, None));
        }

        let item_id_u16: u16 = item_id
            .0
            .try_into()
            .map_err(|_| ReadingError::Message("Invalid item id!".into()))?;

        Ok(ItemStackSerializer(Cow::Owned(
            ItemStack::new_with_component(
                item_count.0 as u8,
                Item::from_id(item_id_u16).unwrap_or(&Item::AIR),
                patch,
            ),
        )))
    }

    pub fn write(&self, write: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        serialize_item_stack_with_id(self.0.as_ref(), self.0.item.id, None, write)
    }

    pub fn read_length_prefixed_optional(
        read: &mut impl NetworkReadExt,
    ) -> Result<ItemStackSerializer<'static>, ReadingError> {
        const MAX_COMPONENTS: i32 = 256;

        let item_count = read.get_var_int()?;
        if item_count.0 == 0 {
            return Ok(ItemStackSerializer(Cow::Borrowed(ItemStack::EMPTY)));
        }
        let item_count_u8 = item_count
            .0
            .try_into()
            .map_err(|_| ReadingError::Message("Invalid item count!".into()))?;

        let item_id = read.get_var_int()?;
        let num_to_add = read.get_var_int()?.0;
        let num_to_remove = read.get_var_int()?.0;

        if num_to_add < 0 || num_to_remove < 0 {
            return Err(ReadingError::Message("Negative component count".into()));
        }

        let total_components = num_to_add
            .checked_add(num_to_remove)
            .ok_or_else(|| ReadingError::Message("Component count overflow".into()))?;

        if total_components > MAX_COMPONENTS {
            return Err(ReadingError::Message(
                "Too many components in ItemStack patch".into(),
            ));
        }

        let mut patch = Vec::with_capacity(total_components as usize);

        for _ in 0..num_to_add {
            let (id, component_impl) = read_length_prefixed_component(read)?;
            patch.push((id, Some(component_impl)));
        }

        for _ in 0..num_to_remove {
            patch.push((read_component_id(read)?, None));
        }

        let item_id_u16 = item_id
            .0
            .try_into()
            .map_err(|_| ReadingError::Message("Invalid item id!".into()))?;

        Ok(ItemStackSerializer(Cow::Owned(
            ItemStack::new_with_component(
                item_count_u8,
                Item::from_id(item_id_u16).unwrap_or(&Item::AIR),
                patch,
            ),
        )))
    }

    pub fn read_length_prefixed_optional_with_version(
        read: &mut impl NetworkReadExt,
        version: &JavaMinecraftVersion,
    ) -> Result<ItemStackSerializer<'static>, ReadingError> {
        if *version != JavaMinecraftVersion::V_1_21_4 {
            return Self::read_length_prefixed_optional(read);
        }

        const MAX_COMPONENTS: i32 = 256;
        let item_count = read.get_var_int()?.0;
        if item_count == 0 {
            return Ok(ItemStackSerializer(Cow::Borrowed(ItemStack::EMPTY)));
        }
        let item_count = u8::try_from(item_count)
            .map_err(|_| ReadingError::Message("Invalid item count".into()))?;
        let item_id = read.get_var_int()?.0;
        let num_to_add = read.get_var_int()?.0;
        let num_to_remove = read.get_var_int()?.0;
        let total_components = num_to_add
            .checked_add(num_to_remove)
            .filter(|count| (0..=MAX_COMPONENTS).contains(count))
            .ok_or_else(|| ReadingError::Message("Invalid component count".into()))?;

        let mut patch = Vec::with_capacity(total_components as usize);
        for _ in 0..num_to_add {
            let raw_id = u8::try_from(read.get_var_int()?.0)
                .map_err(|_| ReadingError::Message("Invalid component ID".into()))?;
            // Both legacy tooltip-hiding components have an empty payload and
            // are represented by the newer TooltipDisplay component now.
            if matches!(raw_id, 15 | 16) {
                continue;
            }
            let id = component_id_from_version(raw_id, version).ok_or_else(|| {
                ReadingError::Message(format!("Unknown 1.21.4 component ID: {raw_id}"))
            })?;
            let value = deserialize_for_version(id, version, read)?;
            patch.push((id, Some(value)));
        }
        for _ in 0..num_to_remove {
            let raw_id = u8::try_from(read.get_var_int()?.0)
                .map_err(|_| ReadingError::Message("Invalid component ID".into()))?;
            if let Some(id) = component_id_from_version(raw_id, version) {
                patch.push((id, None));
            }
        }

        let item_id =
            u16::try_from(item_id).map_err(|_| ReadingError::Message("Invalid item ID".into()))?;
        Ok(ItemStackSerializer(Cow::Owned(
            ItemStack::new_with_component(
                item_count,
                Item::from_id(item_id).unwrap_or(&Item::AIR),
                patch,
            ),
        )))
    }

    pub fn write_with_version(
        &self,
        write: &mut impl NetworkWriteExt,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let remapped_item_id = remap_item_id_for_version(self.0.item.id, *version);
        serialize_item_stack_with_id(self.0.as_ref(), remapped_item_id, Some(version), write)
    }

    pub fn write_item_cost_with_version(
        &self,
        write: &mut impl NetworkWriteExt,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let remapped_item_id = remap_item_id_for_version(self.0.item.id, *version);
        serialize_item_cost_with_id(self.0.as_ref(), remapped_item_id, write)
    }

    #[must_use]
    pub fn to_stack(self) -> ItemStack {
        self.0.into_owned()
    }

    #[must_use]
    pub fn to_stack_for_version(self, version: &JavaMinecraftVersion) -> ItemStack {
        let mut stack = self.0.into_owned();
        if stack.is_empty() {
            return stack;
        }

        let remapped_item_id = remap_item_id_from_version(stack.item.id, *version);
        stack.item = Item::from_id(remapped_item_id).unwrap_or(&Item::AIR);
        stack
    }
}

impl From<ItemStack> for ItemStackSerializer<'_> {
    fn from(item: ItemStack) -> Self {
        ItemStackSerializer(Cow::Owned(item))
    }
}

impl From<Option<ItemStack>> for ItemStackSerializer<'_> {
    fn from(item: Option<ItemStack>) -> Self {
        item.map_or_else(
            || ItemStackSerializer(Cow::Borrowed(ItemStack::EMPTY)),
            ItemStackSerializer::from,
        )
    }
}

#[derive(Debug, Clone)]
pub struct ItemComponentHash {
    pub added: Vec<(VarInt, i32)>,
    pub removed: Vec<VarInt>,
}

impl ItemComponentHash {
    pub fn read(read: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        const MAX_COMPONENTS: i32 = 256;

        let added_length = read.get_var_int()?;
        if added_length.0 < 0 || added_length.0 > MAX_COMPONENTS {
            return Err(ReadingError::Message("added_length out of bounds".into()));
        }
        let mut added = Vec::with_capacity(added_length.0 as usize);
        for _ in 0..added_length.0 {
            let component_id = read.get_var_int()?;
            let component_value = read.get_i32()?;
            added.push((component_id, component_value));
        }

        let removed_length = read.get_var_int()?;
        if removed_length.0 < 0 || removed_length.0 > MAX_COMPONENTS {
            return Err(ReadingError::Message("removed_length out of bounds".into()));
        }
        let mut removed = Vec::with_capacity(removed_length.0 as usize);
        for _ in 0..removed_length.0 {
            let component_id = read.get_var_int()?;
            removed.push(component_id);
        }

        Ok(Self { added, removed })
    }

    pub fn write(&self, write: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        write.put_var_int(&VarInt::from(self.added.len() as i32))?;
        for (id, val) in &self.added {
            write.put_var_int(id)?;
            write.put_i32(*val)?;
        }
        write.put_var_int(&VarInt::from(self.removed.len() as i32))?;
        for id in &self.removed {
            write.put_var_int(id)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ItemStackHash {
    item_id: VarInt,
    count: VarInt,
    components: ItemComponentHash,
}

#[derive(Debug, Clone)]
pub struct OptionalItemStackHash(pub Option<ItemStackHash>);

impl OptionalItemStackHash {
    pub fn read(read: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let is_some = read.get_bool()?;
        if is_some {
            let item_id = read.get_var_int()?;
            let count = read.get_var_int()?;
            let components = ItemComponentHash::read(read)?;

            Ok(Self(Some(ItemStackHash {
                item_id,
                count,
                components,
            })))
        } else {
            Ok(Self(None))
        }
    }

    pub fn write(&self, write: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        if let Some(hash) = &self.0 {
            write.put_bool(true)?;
            write.put_var_int(&hash.item_id)?;
            write.put_var_int(&hash.count)?;
            hash.components.write(write)?;
        } else {
            write.put_bool(false)?;
        }
        Ok(())
    }

    #[must_use]
    pub fn hash_equals(&self, other: &ItemStack) -> bool {
        if let Some(hash) = &self.0 {
            if hash.item_id != other.item.id.into() || hash.count != other.item_count.into() {
                return false;
            }
            let calc = || {
                let mut to_add = 0u8;
                let mut to_remove = 0u8;
                for (_id, data) in &other.patch {
                    if data.is_none() {
                        to_remove += 1;
                    } else {
                        to_add += 1;
                    }
                }
                (to_add, to_remove)
            };
            let (to_add, to_remove) = calc();
            if to_add as usize != hash.components.added.len()
                || to_remove as usize != hash.components.removed.len()
            {
                return false;
            }
            for (other_id, data) in &other.patch {
                if let Some(data) = data {
                    let checksum = data.get_hash();
                    for (id, hash) in &hash.components.added {
                        if id == &VarInt::from(other_id.to_id()) {
                            if hash == &checksum {
                                break;
                            }
                            return false;
                        }
                    }
                } else if !hash
                    .components
                    .removed
                    .contains(&VarInt::from(other_id.to_id()))
                {
                    return false;
                }
            }
            true
        } else {
            other.is_empty()
        }
    }
}

pub struct ItemStackTemplateSerializer<'a>(pub Cow<'a, ItemStack>);

impl ItemStackTemplateSerializer<'_> {
    pub fn write_with_version(
        &self,
        write: &mut impl NetworkWriteExt,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let remapped_item_id = remap_item_id_for_version(self.0.item.id, *version);
        serialize_any_item_stack_with_id(
            self.0.as_ref(),
            remapped_item_id,
            *version >= JavaMinecraftVersion::V_26_1,
            Some(version),
            write,
        )
    }

    pub fn write(&self, write: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        serialize_any_item_stack_with_id(self.0.as_ref(), self.0.item.id, true, None, write)
    }
}

#[cfg(test)]
mod tests {
    use pumpkin_data::data_component::DataComponent;
    use pumpkin_data::item::Item;
    use pumpkin_data::item_id_remap::{remap_item_id_for_version, remap_item_id_from_version};
    use pumpkin_util::version::JavaMinecraftVersion;

    use crate::VarInt;
    use crate::ser::NetworkWriteExt;

    use super::{ItemStackSerializer, component_id_for_version};

    #[test]
    fn vanilla_1_21_4_item_component_ids_are_remapped_by_registry_name() {
        let version = JavaMinecraftVersion::V_1_21_4;

        assert_eq!(
            component_id_for_version(DataComponent::PotionContents, Some(&version)),
            Some(41)
        );
        assert_eq!(
            component_id_for_version(DataComponent::OminousBottleAmplifier, Some(&version)),
            Some(51)
        );
        assert_eq!(
            component_id_for_version(DataComponent::ContainerLoot, Some(&version)),
            Some(66)
        );
        assert_eq!(
            component_id_for_version(DataComponent::AttackRange, Some(&version)),
            None
        );
    }

    #[test]
    fn vanilla_1_21_4_creative_stack_decodes_unprefixed_legacy_payloads() {
        let version = JavaMinecraftVersion::V_1_21_4;
        let mut bytes = Vec::new();
        let old_item_id = remap_item_id_for_version(Item::DIAMOND_SWORD.id, version);
        assert_eq!(
            remap_item_id_from_version(old_item_id, version),
            Item::DIAMOND_SWORD.id
        );
        bytes.write_var_int(&VarInt(1)).unwrap();
        bytes.write_var_int(&VarInt::from(old_item_id)).unwrap();
        bytes.write_var_int(&VarInt(4)).unwrap();
        bytes.write_var_int(&VarInt(0)).unwrap();

        // unbreakable(show_in_tooltip=true)
        bytes.write_var_int(&VarInt(4)).unwrap();
        bytes.write_bool(true).unwrap();
        // enchantments([], show_in_tooltip=true)
        bytes.write_var_int(&VarInt(10)).unwrap();
        bytes.write_var_int(&VarInt(0)).unwrap();
        bytes.write_bool(true).unwrap();
        // dyed_color(0x123456, show_in_tooltip=true)
        bytes.write_var_int(&VarInt(34)).unwrap();
        bytes.write_i32(0x12_34_56).unwrap();
        bytes.write_bool(true).unwrap();
        // equippable(head, numeric sound holder 0, no optionals, true/false/true)
        bytes.write_var_int(&VarInt(28)).unwrap();
        bytes.write_var_int(&VarInt(5)).unwrap();
        bytes.write_var_int(&VarInt(1)).unwrap();
        bytes.extend_from_slice(&[0, 0, 0, 1, 0, 1]);

        let mut input = bytes.as_slice();
        let serializer =
            ItemStackSerializer::read_length_prefixed_optional_with_version(&mut input, &version)
                .unwrap();

        assert!(input.is_empty());
        assert_eq!(serializer.0.item.id, old_item_id);
        let stack = serializer.to_stack_for_version(&version);
        assert_eq!(stack.item.id, Item::DIAMOND_SWORD.id);
        assert_eq!(stack.patch.len(), 4);
        assert!(stack.patch[0].0 == DataComponent::Unbreakable);
        assert!(stack.patch[1].0 == DataComponent::Enchantments);
        assert!(stack.patch[2].0 == DataComponent::DyedColor);
        assert!(stack.patch[3].0 == DataComponent::Equippable);
    }
}

impl From<ItemStack> for ItemStackTemplateSerializer<'_> {
    fn from(item: ItemStack) -> Self {
        ItemStackTemplateSerializer(Cow::Owned(item))
    }
}
