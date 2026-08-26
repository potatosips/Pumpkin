use std::borrow::Cow;

use crate::codec::var_int::VarInt;
use crate::ser::{NetworkReadExt, NetworkWriteExt, ReadingError, WritingError};
use pumpkin_data::Enchantment;
use pumpkin_data::data_component::DataComponent;
use pumpkin_data::data_component_impl::{
    AxolotlVariantImpl, BaseColorImpl, BundleContentsImpl, CatCollarImpl, CatSoundVariantImpl,
    CatVariantImpl, ChickenSoundVariantImpl, ChickenVariantImpl, ConsumableImpl, ConsumeAnimation,
    ConsumeEffect, CowSoundVariantImpl, CowVariantImpl, CustomDataImpl, CustomNameImpl, DamageImpl,
    DamageResistantImpl, DamageResistantType, DataComponentImpl, DyedColorImpl, EnchantableImpl,
    EnchantmentsImpl, EquipmentSlot, EquippableImpl, FireworkExplosionImpl, FireworkExplosionShape,
    FireworksImpl, FoodImpl, FoxVariantImpl, FrogVariantImpl, GliderImpl, HorseVariantImpl, IDSet,
    IDSetContent, IdOr, InstrumentImpl, IntangibleProjectileImpl, ItemModelImpl, ItemNameImpl,
    LlamaVariantImpl, MapIdImpl, MaxDamageImpl, MaxStackSizeImpl, MooshroomVariantImpl,
    NoteBlockSoundImpl, OminousBottleAmplifierImpl, PaintingVariantImpl, ParrotVariantImpl,
    PigSoundVariantImpl, PigVariantImpl, PotionContentsImpl, PotionDurationScaleImpl,
    RabbitVariantImpl, SalmonSizeImpl, SheepColorImpl, ShulkerColorImpl, SoundEvent,
    StatusEffectInstance, StoredEnchantmentsImpl, SuspiciousStewEffect, SuspiciousStewEffectsImpl,
    TooltipStyleImpl, TropicalFishBaseColorImpl, TropicalFishPatternColorImpl,
    TropicalFishPatternImpl, UnbreakableImpl, UseCooldownImpl, VillagerVariantImpl, WeaponImpl,
    WolfCollarImpl, WolfSoundVariantImpl, WolfVariantImpl, WritableBookContentImpl,
    WrittenBookContentImpl, ZombieNautilusVariantImpl, get,
};
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::Sound;
use pumpkin_nbt::{serializer::NbtWriteHelperJava, tag::NbtTag};
use pumpkin_util::version::JavaMinecraftVersion;

const MAX_STATUS_EFFECTS: usize = 128;

#[must_use]
pub fn data_to_proto_sound(id_or: &IdOr<SoundEvent>) -> crate::IdOr<crate::SoundEvent> {
    match id_or {
        IdOr::Id(id) => crate::IdOr::Id(*id as u16),
        IdOr::Value(sound) => crate::IdOr::Value(crate::SoundEvent {
            sound_name: sound.sound_name.clone(),
            range: sound.range,
        }),
    }
}

#[must_use]
pub fn proto_to_data_sound(id_or: &crate::IdOr<crate::SoundEvent>) -> Option<IdOr<SoundEvent>> {
    match id_or {
        crate::IdOr::Id(id) => {
            let name = Sound::NAMES.get(*id as usize)?;
            Some(IdOr::Id(Sound::from_name(name)?))
        }
        crate::IdOr::Value(sound) => Some(IdOr::Value(SoundEvent {
            sound_name: sound.sound_name.clone(),
            range: sound.range,
        })),
    }
}

fn deserialize_idset<T: IDSetContent>(
    seq: &mut impl NetworkReadExt,
) -> Result<IDSet<T>, ReadingError> {
    let id_type = seq.get_var_int()?.0;

    match id_type.cmp(&0) {
        std::cmp::Ordering::Equal => {
            let tag = seq.get_str()?;
            Ok(IDSet::Tag(Cow::Owned(tag.into())))
        }
        std::cmp::Ordering::Greater => {
            let len = id_type - 1;
            let mut content_vec = Vec::with_capacity(len as usize);

            for _ in 0..len {
                let varint_id = seq.get_var_int()?.0;

                let elmt = T::from_id(varint_id as u16).ok_or(ReadingError::Message(
                    "Invalid registry id VarInt in IDSet".into(),
                ))?;
                content_vec.push(elmt);
            }
            Ok(IDSet::IDs(Cow::Owned(content_vec)))
        }
        std::cmp::Ordering::Less => Result::Err(ReadingError::Message(
            "Negative type/len VarInt in IDSet".into(),
        )),
    }
}

fn serialize_idset<C: IDSetContent>(
    idset: &IDSet<C>,
    seq: &mut impl NetworkWriteExt,
) -> Result<(), WritingError> {
    match idset {
        IDSet::Tag(tag) => {
            seq.write_var_int(&VarInt(0))?;
            seq.write_string(tag)
        }
        IDSet::IDs(elements) => {
            seq.write_var_int(&VarInt(elements.len() as i32 + 1))?;
            for elmt in elements.iter() {
                seq.write_var_int(&VarInt(elmt.registry_id() as i32))?;
            }
            Ok(())
        }
    }
}

fn deserialize_status_effects(
    seq: &mut impl NetworkReadExt,
) -> Result<Vec<StatusEffectInstance>, ReadingError> {
    let effects_len = seq.get_var_int()?.0 as usize;
    if effects_len > MAX_STATUS_EFFECTS {
        return Err(ReadingError::Message("Too many status effects".into()));
    }
    let mut custom_effects = Vec::with_capacity(effects_len);
    for _ in 0..effects_len {
        let effect_registry_id = seq.get_var_int()?.0;
        let effect_name = StatusEffect::from_id(effect_registry_id as u16)
            .ok_or(ReadingError::Message("Invalid effect_id!".into()))?
            .minecraft_name;
        let effect_id = Cow::Borrowed(effect_name);

        // Effect parameters
        let amplifier = seq.get_var_int()?.0;
        let duration = seq.get_var_int()?.0;
        let ambient = seq.get_bool()?;
        let show_particles = seq.get_bool()?;
        let show_icon = seq.get_bool()?;

        // Hidden effect (optional, recursive) - we skip it for now
        let has_hidden = seq.get_bool()?;
        if has_hidden {
            // Skip hidden effect parameters recursively
            skip_effect_parameters(seq)?;
        }

        custom_effects.push(StatusEffectInstance {
            effect_id,
            amplifier,
            duration,
            ambient,
            show_particles,
            show_icon,
        });
    }

    Ok(custom_effects)
}

fn serialize_status_effects(
    effects: &Vec<StatusEffectInstance>,
    seq: &mut impl NetworkWriteExt,
) -> Result<(), WritingError> {
    seq.write_var_int(&VarInt(effects.len() as i32))?;

    for effect in effects {
        let effect_id = StatusEffect::from_minecraft_name(&effect.effect_id)
            .ok_or_else(|| {
                WritingError::Message(format!("Invalid status effect: {}", effect.effect_id))
            })?
            .registry_id();
        seq.write_var_int(&VarInt(effect_id as i32))?;
        // Effect parameters
        seq.write_var_int(&VarInt::from(effect.amplifier))?;
        seq.write_var_int(&VarInt::from(effect.duration))?;
        seq.write_bool(effect.ambient)?;
        seq.write_bool(effect.show_particles)?;
        seq.write_bool(effect.show_icon)?;
        // No hidden effect for now
        seq.write_bool(false)?;
    }
    Ok(())
}

fn deserialize_consume_effect(
    seq: &mut impl NetworkReadExt,
) -> Result<ConsumeEffect, ReadingError> {
    let effect_type = seq.get_var_int()?.0;
    match effect_type {
        0 => {
            let probability = seq.get_f32()?;
            Ok(ConsumeEffect::ApplyEffects((
                Cow::Owned(deserialize_status_effects(seq)?),
                probability,
            )))
        }
        1 => {
            let idset = deserialize_idset(seq)?;
            Ok(ConsumeEffect::RemoveEffects(idset))
        }
        2 => Ok(ConsumeEffect::ClearAllEffects),
        3 => {
            let diameter = seq.get_f32()?;
            Ok(ConsumeEffect::TeleportRandomly(diameter))
        }
        4 => {
            // Need to read IdOr<SoundEvent> manually. This depends on how it is serialized.
            // In vanilla, it's either an id (0) or a sound event (1) ... but wait, `crate::IdOr<crate::SoundEvent>` doesn't have a `NetworkReadExt` method.
            // Let's defer this and assume it implements `read` for now or wait, `IdOr` does implement `PacketRead` or something?
            // Actually, we can just use `IdOr::read` if we impl it, but let's change it to:
            let proto_sound_event = crate::IdOr::<crate::SoundEvent>::read(seq, |r| {
                let sound_name = r.get_str()?.into();
                let range = r.get_option(NetworkReadExt::get_f32)?;
                Ok(crate::SoundEvent { sound_name, range })
            })
            .map_err(|e| {
                ReadingError::Message(format!("No sound IdOr<SoundEvent> in ConsumeEffect: {e}"))
            })?;
            Ok(ConsumeEffect::PlaySound(
                proto_to_data_sound(&proto_sound_event).ok_or(ReadingError::Message(
                    "Invalid sound in ConsumeEffect".into(),
                ))?,
            ))
        }
        _ => Err(ReadingError::Message(
            "Invalid effect_type in ConsumeEffect".into(),
        )),
    }
}

fn serialize_consume_effect(
    consume_effect: &ConsumeEffect,
    seq: &mut impl NetworkWriteExt,
) -> Result<(), WritingError> {
    seq.write_var_int(&VarInt(consume_effect.registry_id() as i32))?;
    match consume_effect {
        ConsumeEffect::ApplyEffects((effects, probability)) => {
            serialize_status_effects(&effects.to_vec(), seq)?;
            seq.write_f32(*probability)?;
        }
        ConsumeEffect::RemoveEffects(idset) => serialize_idset(idset, seq)?,
        ConsumeEffect::ClearAllEffects => (),
        ConsumeEffect::TeleportRandomly(diameter) => seq.write_f32(*diameter)?,
        ConsumeEffect::PlaySound(id_or) => {
            crate::IdOr::<crate::SoundEvent>::write(&data_to_proto_sound(id_or), seq, |w, e| {
                w.write_string(&e.sound_name)?;
                w.write_option(&e.range, |w2, r| w2.write_f32(*r))
            })?;
        }
    }
    Ok(())
}

trait DataComponentCodec<Impl: DataComponentImpl> {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError>;
    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Impl, ReadingError>;
}

impl DataComponentCodec<Self> for MaxStackSizeImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_var_int(&VarInt::from(self.size))
    }
    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let size = u8::try_from(seq.get_var_int()?.0)
            .map_err(|_| ReadingError::Message("No MaxStackSize VarInt!".into()))?;
        Ok(Self { size })
    }
}

impl DataComponentCodec<Self> for MaxDamageImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_var_int(&VarInt::from(self.max_damage))
    }
    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let max_damage = seq.get_var_int()?.0;
        Ok(Self { max_damage })
    }
}

impl DataComponentCodec<Self> for FoodImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_var_int(&VarInt::from(self.nutrition))?;
        seq.write_f32(self.saturation)?;
        seq.write_bool(self.can_always_eat)
    }
    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let nutrition = seq.get_var_int()?.0;
        let saturation = seq.get_f32()?;
        let can_always_eat = seq.get_bool()?;
        Ok(Self {
            nutrition,
            saturation,
            can_always_eat,
        })
    }
}

impl DataComponentCodec<Self> for GliderImpl {
    fn serialize(&self, _seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        Ok(())
    }
    fn deserialize(_seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        Ok(Self)
    }
}

impl DataComponentCodec<Self> for IntangibleProjectileImpl {
    fn serialize(&self, _seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        Ok(())
    }
    fn deserialize(_seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        Ok(Self)
    }
}

impl DataComponentCodec<Self> for PotionDurationScaleImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_f32(self.scale)
    }
    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let scale = seq.get_f32()?;
        Ok(Self { scale })
    }
}

impl DataComponentCodec<Self> for TooltipStyleImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_string(&self.id)
    }
    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let id = seq.get_str()?;
        Ok(Self { id: id.to_string() })
    }
}

impl DataComponentCodec<Self> for NoteBlockSoundImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_string(&self.sound)
    }
    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let sound = seq.get_str()?;
        Ok(Self {
            sound: sound.to_string(),
        })
    }
}

impl DataComponentCodec<Self> for BaseColorImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_string(&self.color)
    }
    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let color = seq.get_str()?;
        Ok(Self {
            color: color.to_string(),
        })
    }
}

impl DataComponentCodec<Self> for EnchantableImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_var_int(&VarInt::from(self.value))
    }
    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let value = seq.get_var_int()?.0;
        Ok(Self { value })
    }
}

impl DataComponentCodec<Self> for WeaponImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_var_int(&VarInt::from(self.item_damage_per_attack as i32))
    }
    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let item_damage_per_attack = seq.get_var_int()?.0.max(0) as u32;
        Ok(Self {
            item_damage_per_attack,
        })
    }
}

impl DataComponentCodec<Self> for OminousBottleAmplifierImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_var_int(&VarInt::from(self.amplifier))
    }
    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let amplifier = seq.get_var_int()?.0;
        Ok(Self { amplifier })
    }
}

impl DataComponentCodec<Self> for DamageResistantImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_string(self.res_type.as_str())
    }
    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let tag = seq.get_str()?;
        Ok(Self {
            res_type: DamageResistantType::from_tag(&tag),
        })
    }
}

impl DataComponentCodec<Self> for InstrumentImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_string(&self.instrument)
    }
    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let instrument = seq.get_str()?;
        Ok(Self {
            instrument: Cow::Owned(instrument.into()),
        })
    }
}

impl DataComponentCodec<Self> for DamageImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_var_int(&VarInt::from(self.damage))
    }
    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let damage = seq.get_var_int()?.0;
        Ok(Self { damage })
    }
}

impl DataComponentCodec<Self> for EnchantmentsImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_var_int(&VarInt::from(self.enchantment.len() as i32))?;
        for (enc, level) in self.enchantment.iter() {
            seq.write_var_int(&VarInt::from(enc.id))?;
            seq.write_var_int(&VarInt::from(*level))?;
        }
        Ok(())
    }
    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        const MAX_ENCHANTMENTS: usize = 256;

        let len = seq.get_var_int()?.0 as usize;
        if len > MAX_ENCHANTMENTS {
            return Err(ReadingError::Message("Too many enchantments".into()));
        }
        let mut enc = Vec::with_capacity(len);
        for _ in 0..len {
            let id = seq.get_var_int()?.0 as u8;
            let level = seq.get_var_int()?.0;
            enc.push((
                Enchantment::from_id(id).ok_or(ReadingError::Message(
                    "EnchantmentsImpl Enchantment VarInt Incorrect!".into(),
                ))?,
                level,
            ));
        }
        Ok(Self {
            enchantment: Cow::from(enc),
        })
    }
}

impl DataComponentCodec<Self> for UnbreakableImpl {
    fn serialize(&self, _seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        Ok(())
    }
    fn deserialize(_seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        Ok(Self)
    }
}

impl DataComponentCodec<Self> for ItemModelImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_string(&self.id)
    }

    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let id = seq.get_str()?;
        Ok(Self {
            id: Cow::Owned(id.into()),
        })
    }
}

impl DataComponentCodec<Self> for CustomNameImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        let mut bytes = Vec::new();
        NbtTag::String(self.name.clone().get_text().into_boxed_str())
            .serialize(&mut NbtWriteHelperJava::new(&mut bytes))
            .map_err(|e| WritingError::Message(e.to_string()))?;
        seq.write_slice(&bytes)?;
        Ok(())
    }

    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let name = seq.get_str()?;
        Ok(Self {
            name: pumpkin_util::text::TextComponent::text(String::from(name)),
        })
    }
}

impl DataComponentCodec<Self> for ItemNameImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        let mut name = pumpkin_nbt::compound::NbtCompound::new();
        name.put_string("translate", self.name.to_string());
        let mut bytes = Vec::new();
        NbtTag::Compound(name)
            .serialize(&mut NbtWriteHelperJava::new(&mut bytes))
            .map_err(|error| WritingError::Message(error.to_string()))?;
        seq.write_slice(&bytes)
    }

    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let name = seq.get_str()?;
        Ok(Self {
            name: Cow::Owned(name.into()),
        })
    }
}

impl DataComponentCodec<Self> for DyedColorImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_i32(self.rgb)
    }

    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        Ok(Self {
            rgb: seq.get_i32()?,
        })
    }
}

impl DataComponentCodec<Self> for SuspiciousStewEffectsImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        let effect_count = i32::try_from(self.effects.len())
            .map_err(|_| WritingError::Message("Too many suspicious stew effects".into()))?;
        seq.write_var_int(&VarInt(effect_count))?;
        for effect in self.effects.iter() {
            let id = StatusEffect::from_minecraft_name(&effect.effect)
                .ok_or_else(|| WritingError::Message("Unknown suspicious stew effect".into()))?
                .id;
            seq.write_var_int(&VarInt(i32::from(id)))?;
            seq.write_var_int(&VarInt(effect.duration))?;
        }
        Ok(())
    }

    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        const MAX_EFFECTS: i32 = 128;

        let count = seq.get_var_int()?.0;
        if !(0..=MAX_EFFECTS).contains(&count) {
            return Err(ReadingError::Message(
                "Invalid suspicious stew effect count".into(),
            ));
        }

        let mut effects =
            Vec::with_capacity(usize::try_from(count).map_err(|_| {
                ReadingError::Message("Invalid suspicious stew effect count".into())
            })?);
        for _ in 0..count {
            let id = u16::try_from(seq.get_var_int()?.0)
                .map_err(|_| ReadingError::Message("Invalid suspicious stew effect id".into()))?;
            let effect = StatusEffect::from_id(id)
                .ok_or_else(|| ReadingError::Message("Unknown suspicious stew effect id".into()))?;
            let duration = seq.get_var_int()?.0;
            effects.push(SuspiciousStewEffect {
                effect: Cow::Borrowed(effect.minecraft_name),
                duration,
            });
        }
        Ok(Self {
            effects: Cow::Owned(effects),
        })
    }
}

impl DataComponentCodec<Self> for CustomDataImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        let mut bytes = Vec::new();
        NbtTag::Compound(self.data.clone())
            .serialize(&mut NbtWriteHelperJava::new(&mut bytes))
            .map_err(|e| WritingError::Message(e.to_string()))?;
        seq.write_slice(&bytes)?;
        Ok(())
    }

    fn deserialize(_seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        Err(ReadingError::Message(
            "CustomData raw component decoding is not supported; use the custom-data item-stack API".into(),
        ))
    }
}

impl DataComponentCodec<Self> for ConsumableImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_f32(self.consume_seconds)?;
        seq.write_var_int(&VarInt(self.animation as i32))?;
        crate::IdOr::<crate::SoundEvent>::write(
            &data_to_proto_sound(&self.sound_event),
            seq,
            |w, e| {
                w.write_string(&e.sound_name)?;
                w.write_option(&e.range, |w2, r| w2.write_f32(*r))
            },
        )?;
        seq.write_bool(self.consume_particles)?;
        seq.write_var_int(&VarInt(self.effects.len() as i32))?;

        for effect in self.effects.iter() {
            serialize_consume_effect(effect, seq)?;
        }

        Ok(())
    }

    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let consume_seconds = seq.get_f32()?;
        let animation_id = seq.get_var_int()?;

        let animation: ConsumeAnimation = animation_id
            .0
            .try_into()
            .map_err(|()| ReadingError::Message("Invalid ConsumableImpl animation id!".into()))?;
        let proto_sound_event = crate::IdOr::<crate::SoundEvent>::read(seq, |r| {
            let sound_name = r.get_str()?.into();
            let range = r.get_option(NetworkReadExt::get_f32)?;
            Ok(crate::SoundEvent { sound_name, range })
        })?;
        let consume_particles = seq.get_bool()?;

        let sound_event = proto_to_data_sound(&proto_sound_event).ok_or(ReadingError::Message(
            "Invalid sound in ConsumableImpl".into(),
        ))?;
        let effects_len = seq.get_var_int()?.0;
        let mut effects_vec = Vec::with_capacity(effects_len as usize);

        for _ in 0..effects_len {
            effects_vec.push(deserialize_consume_effect(seq)?);
        }

        let effects: Cow<'static, [ConsumeEffect]> = Cow::Owned(effects_vec);

        Ok(Self {
            consume_seconds,
            animation,
            sound_event,
            consume_particles,
            effects,
        })
    }
}

impl DataComponentCodec<Self> for EquippableImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_var_int(&VarInt(self.slot.get_slot_index()))?;
        crate::IdOr::<crate::SoundEvent>::write(
            &data_to_proto_sound(&self.equip_sound),
            seq,
            |w, e| {
                w.write_string(&e.sound_name)?;
                w.write_option(&e.range, |w2, r| w2.write_f32(*r))
            },
        )?;

        seq.write_bool(self.asset_id.is_some())?;
        if let Some(asset) = &self.asset_id {
            seq.write_string(asset)?;
        }

        seq.write_bool(self.camera_overlay.is_some())?;
        if let Some(overlay) = &self.camera_overlay {
            seq.write_string(overlay)?;
        }

        seq.write_bool(self.allowed_entities.is_some())?;
        if let Some(allowed) = &self.allowed_entities {
            serialize_idset(allowed, seq)?;
        }

        seq.write_bool(self.dispensable)?;
        seq.write_bool(self.swappable)?;
        seq.write_bool(self.damage_on_hurt)?;
        seq.write_bool(self.equip_on_interact)?;
        seq.write_bool(self.can_be_sheared)?;
        crate::IdOr::<crate::SoundEvent>::write(
            &data_to_proto_sound(&self.shearing_sound),
            seq,
            |w, e| {
                w.write_string(&e.sound_name)?;
                w.write_option(&e.range, |w2, r| w2.write_f32(*r))
            },
        )
    }

    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let slot_index = seq.get_var_int()?.0;
        let slot = EquipmentSlot::from_slot_index(slot_index).ok_or(ReadingError::Message(
            format!("Invalid equipment slot index {slot_index}"),
        ))?;
        let equip_sound = proto_to_data_sound(&crate::IdOr::<crate::SoundEvent>::read(seq, |r| {
            let sound_name = r.get_str()?.into();
            let range = r.get_option(NetworkReadExt::get_f32)?;
            Ok(crate::SoundEvent { sound_name, range })
        })?)
        .ok_or(ReadingError::Message(
            "Invalid sound in EquippableImpl".into(),
        ))?;

        let asset_id = if seq.get_bool()? {
            Some(Cow::Owned(seq.get_str()?.into()))
        } else {
            None
        };

        let camera_overlay = if seq.get_bool()? {
            Some(Cow::Owned(seq.get_str()?.into()))
        } else {
            None
        };

        let has_allowed_entities = seq.get_bool()?;

        let allowed_entities: Option<IDSet<EntityType>> = if has_allowed_entities {
            Some(deserialize_idset(seq)?)
        } else {
            None
        };

        let dispensable = seq.get_bool()?;
        let swappable = seq.get_bool()?;
        let damage_on_hurt = seq.get_bool()?;
        let equip_on_interact = seq.get_bool()?;
        let can_be_sheared = seq.get_bool()?;
        let shearing_sound =
            proto_to_data_sound(&crate::IdOr::<crate::SoundEvent>::read(seq, |r| {
                let sound_name = r.get_str()?.into();
                let range = r.get_option(NetworkReadExt::get_f32)?;
                Ok(crate::SoundEvent { sound_name, range })
            })?)
            .ok_or(ReadingError::Message(
                "Invalid shearing sound in EquippableImpl".into(),
            ))?;

        Ok(Self {
            slot,
            equip_sound,
            asset_id,
            camera_overlay,
            allowed_entities,
            dispensable,
            swappable,
            damage_on_hurt,
            equip_on_interact,
            can_be_sheared,
            shearing_sound,
        })
    }
}

impl DataComponentCodec<Self> for PotionContentsImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        // Potion ID (optional)
        if let Some(potion_id) = self.potion_id {
            seq.write_bool(true)?;
            seq.write_var_int(&VarInt::from(potion_id))?;
        } else {
            seq.write_bool(false)?;
        }

        // Custom color (optional)
        if let Some(color) = self.custom_color {
            seq.write_bool(true)?;
            seq.write_i32(color)?;
        } else {
            seq.write_bool(false)?;
        }

        // Custom effects list
        serialize_status_effects(&self.custom_effects, seq)?;

        // Custom name (optional)
        if let Some(name) = &self.custom_name {
            seq.write_bool(true)?;
            seq.write_string(name.as_str())?;
        } else {
            seq.write_bool(false)?;
        }

        Ok(())
    }

    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        // Potion ID (optional)
        let has_potion = seq.get_bool()?;
        let potion_id = has_potion
            .then(|| seq.get_var_int().map(|value| value.0))
            .transpose()?;

        // Custom color (optional)
        let has_color = seq.get_bool()?;
        let custom_color = has_color.then(|| seq.get_i32()).transpose()?;

        // Custom effects list
        let custom_effects = deserialize_status_effects(seq)?;

        // Custom name (optional)
        let has_name = seq.get_bool()?;
        let custom_name = has_name
            .then(|| seq.get_str().map(String::from))
            .transpose()?;

        Ok(Self {
            potion_id,
            custom_color,
            custom_effects,
            custom_name,
        })
    }
}

/// Helper to skip hidden effect parameters iteratively with a depth cap
fn skip_effect_parameters(seq: &mut impl NetworkReadExt) -> Result<(), ReadingError> {
    const MAX_EFFECT_DEPTH: usize = 32;
    let mut depth = 0;
    loop {
        // amplifier
        seq.get_var_int()?;
        // duration
        seq.get_var_int()?;
        // ambient
        seq.get_bool()?;
        // show_particles
        seq.get_bool()?;
        // show_icon
        seq.get_bool()?;
        // has_hidden
        let has_hidden = seq.get_bool()?;
        if !has_hidden {
            break;
        }
        depth += 1;
        if depth > MAX_EFFECT_DEPTH {
            return Err(ReadingError::TooLarge(
                "Potion effect hidden depth exceeded".into(),
            ));
        }
    }
    Ok(())
}

impl DataComponentCodec<Self> for FireworkExplosionImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        // Shape (VarInt enum)
        seq.write_var_int(&VarInt::from(self.shape.to_id()))?;
        // Colors list
        seq.write_var_int(&VarInt::from(self.colors.len() as i32))?;
        for color in &self.colors {
            seq.write_i32(*color)?;
        }
        // Fade colors list
        seq.write_var_int(&VarInt::from(self.fade_colors.len() as i32))?;
        for color in &self.fade_colors {
            seq.write_i32(*color)?;
        }
        // hasTrail
        seq.write_bool(self.has_trail)?;
        // hasTwinkle
        seq.write_bool(self.has_twinkle)?;
        Ok(())
    }

    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        // Needs a length cap during deserialization to prevent OOM from malicious packets
        // Vanilla doesn't have any limits (Integer.MAX_VALUE is technically a limit but not enforced in practice)
        const MAX_COLORS: usize = 256;
        const MAX_FADE_COLORS: usize = 256;

        // Shape (VarInt enum)
        let shape_id = seq.get_var_int()?.0;
        let shape = FireworkExplosionShape::from_id(shape_id).ok_or(ReadingError::Message(
            "Invalid FireworkExplosionShape id!".into(),
        ))?;

        // Colors list
        let colors_len = seq.get_var_int()?.0 as usize;
        if colors_len > MAX_COLORS {
            return Err(ReadingError::Message(format!(
                "FireworkExplosionImpl colors_len {colors_len} exceeds maximum of {MAX_COLORS}"
            )));
        }
        let mut colors = Vec::with_capacity(colors_len);
        for _ in 0..colors_len {
            let color = seq.get_i32()?;
            colors.push(color);
        }

        // Fade colors list
        let fade_colors_len = seq.get_var_int()?.0 as usize;
        if fade_colors_len > MAX_FADE_COLORS {
            return Err(ReadingError::Message(format!(
                "FireworkExplosionImpl fade_colors_len {fade_colors_len} exceeds maximum of {MAX_FADE_COLORS}"
            )));
        }
        let mut fade_colors = Vec::with_capacity(fade_colors_len);
        for _ in 0..fade_colors_len {
            let color = seq.get_i32()?;
            fade_colors.push(color);
        }

        // hasTrail
        let has_trail = seq.get_bool()?;

        // hasTwinkle
        let has_twinkle = seq.get_bool()?;

        Ok(Self::new(
            shape,
            colors,
            fade_colors,
            has_trail,
            has_twinkle,
        ))
    }
}

impl DataComponentCodec<Self> for FireworksImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        // Flight duration (VarInt)
        seq.write_var_int(&VarInt::from(self.flight_duration))?;
        // Explosions list
        seq.write_var_int(&VarInt::from(self.explosions.len() as i32))?;
        for explosion in &self.explosions {
            explosion.serialize(seq)?;
        }
        Ok(())
    }

    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        // Needs a length cap during deserialization to prevent OOM from malicious packets
        // Vanilla doesn't have any limits
        const MAX_EXPLOSIONS: usize = 256;
        // Vanilla restricts to 0-255 (UNSIGNED_BYTE in data component codec) (do not trust client NBT to limit it)
        const MAX_FLIGHT_DURATION: i32 = 255;

        // Flight duration
        let flight_duration = seq.get_var_int()?.0;
        if !(0..=MAX_FLIGHT_DURATION).contains(&flight_duration) {
            return Err(ReadingError::Message(format!(
                "FireworksImpl flight_duration {flight_duration} is out of bounds (0-{MAX_FLIGHT_DURATION})"
            )));
        }

        // Explosions list
        let explosions_len = seq.get_var_int()?.0 as usize;
        if explosions_len > MAX_EXPLOSIONS {
            return Err(ReadingError::Message(format!(
                "FireworksImpl explosions_len {explosions_len} exceeds maximum of {MAX_EXPLOSIONS}"
            )));
        }
        let mut explosions = Vec::with_capacity(explosions_len);
        for _ in 0..explosions_len {
            // Recursively deserialize each explosion
            let explosion = FireworkExplosionImpl::deserialize(seq)?;
            explosions.push(explosion);
        }

        Ok(Self::new(flight_duration, explosions))
    }
}

impl DataComponentCodec<Self> for StoredEnchantmentsImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_var_int(&VarInt::from(self.enchantment.len() as i32))?;
        for (enc, level) in self.enchantment.iter() {
            seq.write_var_int(&VarInt::from(enc.id))?;
            seq.write_var_int(&VarInt::from(*level))?;
        }
        Ok(())
    }

    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        const MAX_ENCHANTMENTS: usize = 256;

        let len = seq.get_var_int()?.0 as usize;

        if len > MAX_ENCHANTMENTS {
            return Err(ReadingError::Message("Too many enchantments".into()));
        }

        let mut stored_enchantments = Vec::with_capacity(len);
        for _ in 0..len {
            let id = seq.get_var_int()?.0 as u8;
            let level = seq.get_var_int()?.0;
            stored_enchantments.push((
                Enchantment::from_id(id).ok_or(ReadingError::Message(
                    "StoredEnchantmentsImpl Enchantment VarInt Incorrect!".into(),
                ))?,
                level,
            ));
        }
        Ok(Self {
            enchantment: Cow::from(stored_enchantments),
        })
    }
}

fn write_anonymous_nbt(tag: &NbtTag, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
    let mut bytes = Vec::new();
    tag.clone()
        .serialize(&mut NbtWriteHelperJava::new(&mut bytes))
        .map_err(|error| WritingError::Message(error.to_string()))?;
    seq.write_slice(&bytes)
}

struct NetworkNbtSource<'a, R>(&'a mut R);

impl<'nbt, R: NetworkReadExt> pumpkin_nbt::deserializer::NbtDataSource<'nbt>
    for NetworkNbtSource<'_, R>
{
    fn read_u8(&mut self) -> pumpkin_nbt::deserializer::Result<u8> {
        self.0.get_u8().map_err(|error| {
            pumpkin_nbt::Error::Incomplete(std::io::Error::other(error.to_string()))
        })
    }

    fn read_bytes(&mut self, buf: &mut [u8]) -> pumpkin_nbt::deserializer::Result<()> {
        self.0.read_bytes_to_buf(buf).map_err(|error| {
            pumpkin_nbt::Error::Incomplete(std::io::Error::other(error.to_string()))
        })
    }

    fn seek_relative(&mut self, offset: i64) -> pumpkin_nbt::deserializer::Result<()> {
        let count = usize::try_from(offset).map_err(|_| {
            pumpkin_nbt::Error::Incomplete(std::io::Error::other(
                "negative seek on network NBT source",
            ))
        })?;
        let mut discard = vec![0; count];
        self.read_bytes(&mut discard)
    }

    fn read_string(&mut self, len: usize) -> pumpkin_nbt::deserializer::Result<Cow<'nbt, str>> {
        let mut bytes = vec![0; len];
        self.read_bytes(&mut bytes)?;
        cesu8::from_java_cesu8(&bytes)
            .map(|text| Cow::Owned(text.into_owned()))
            .map_err(|_| pumpkin_nbt::Error::Cesu8DecodingError)
    }

    fn read_byte_array(
        &mut self,
        len: usize,
    ) -> pumpkin_nbt::deserializer::Result<Cow<'nbt, [i8]>> {
        let mut bytes = vec![0; len];
        self.read_bytes(&mut bytes)?;
        Ok(Cow::Owned(
            bytes.into_iter().map(|byte| byte as i8).collect(),
        ))
    }
}

fn read_anonymous_nbt(seq: &mut impl NetworkReadExt) -> Result<NbtTag, ReadingError> {
    let mut reader = pumpkin_nbt::deserializer::NbtReadHelperJava::new(NetworkNbtSource(seq));
    NbtTag::deserialize(&mut reader)
        .map_err(|error| ReadingError::Message(format!("Invalid book text NBT: {error}")))
}

impl DataComponentCodec<Self> for WritableBookContentImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_var_int(&VarInt(self.pages.len() as i32))?;
        for (index, page) in self.pages.iter().enumerate() {
            seq.write_string(page)?;
            let filtered = self.filtered_pages.get(index).and_then(Option::as_ref);
            seq.write_option(&filtered, |writer, text| writer.write_string(text))?;
        }
        Ok(())
    }

    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let count = seq.get_var_int()?.0;
        if !(0..=100).contains(&count) {
            return Err(ReadingError::Message(
                "Invalid writable book page count".into(),
            ));
        }
        let mut pages = Vec::with_capacity(count as usize);
        let mut filtered_pages = Vec::with_capacity(count as usize);
        for _ in 0..count {
            pages.push(seq.get_str_bounded(1024)?.to_string());
            filtered_pages.push(
                seq.get_option(|reader| reader.get_str_bounded(1024))?
                    .map(Into::into),
            );
        }
        Ok(Self {
            pages,
            filtered_pages,
        })
    }
}

impl DataComponentCodec<Self> for WrittenBookContentImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_string(&self.title)?;
        seq.write_option(&self.filtered_title, |writer, title| {
            writer.write_string(title)
        })?;
        seq.write_string(&self.author)?;
        seq.write_var_int(&VarInt(self.generation))?;
        seq.write_var_int(&VarInt(self.pages.len() as i32))?;
        for (index, page) in self.pages.iter().enumerate() {
            write_anonymous_nbt(page, seq)?;
            let filtered = self.filtered_pages.get(index).and_then(Option::as_ref);
            if let Some(filtered) = filtered {
                write_anonymous_nbt(filtered, seq)?;
            } else {
                write_anonymous_nbt(&NbtTag::End, seq)?;
            }
        }
        seq.write_bool(self.resolved)
    }

    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let title = seq.get_str_bounded(32)?.to_string();
        let filtered_title = seq
            .get_option(|reader| reader.get_str_bounded(32))?
            .map(Into::into);
        let author = seq.get_str_bounded(16)?.to_string();
        let generation = seq.get_var_int()?.0;
        let count = seq.get_var_int()?.0;
        if !(0..=100).contains(&count) {
            return Err(ReadingError::Message(
                "Invalid written book page count".into(),
            ));
        }
        let mut pages = Vec::with_capacity(count as usize);
        let mut filtered_pages = Vec::with_capacity(count as usize);
        for _ in 0..count {
            pages.push(read_anonymous_nbt(seq)?);
            let filtered = read_anonymous_nbt(seq)?;
            filtered_pages.push((filtered != NbtTag::End).then_some(filtered));
        }
        Ok(Self {
            title,
            filtered_title,
            author,
            generation,
            pages,
            filtered_pages,
            resolved: seq.get_bool()?,
        })
    }
}

pub fn deserialize(
    id: DataComponent,
    seq: &mut impl NetworkReadExt,
) -> Result<Box<dyn DataComponentImpl>, ReadingError> {
    match id {
        DataComponent::MaxStackSize => Ok(MaxStackSizeImpl::deserialize(seq)?.to_dyn()),
        DataComponent::CustomData => Err(ReadingError::Message(
            "CustomData raw component decoding is not supported; use the custom-data item-stack API".into(),
        )),
        DataComponent::Enchantments => Ok(EnchantmentsImpl::deserialize(seq)?.to_dyn()),
        DataComponent::Damage => Ok(DamageImpl::deserialize(seq)?.to_dyn()),
        DataComponent::Unbreakable => Ok(UnbreakableImpl::deserialize(seq)?.to_dyn()),
        DataComponent::PotionContents => Ok(PotionContentsImpl::deserialize(seq)?.to_dyn()),
        DataComponent::DyedColor => Ok(DyedColorImpl::deserialize(seq)?.to_dyn()),
        DataComponent::SuspiciousStewEffects => {
            Ok(SuspiciousStewEffectsImpl::deserialize(seq)?.to_dyn())
        }
        DataComponent::FireworkExplosion => Ok(FireworkExplosionImpl::deserialize(seq)?.to_dyn()),
        DataComponent::Fireworks => Ok(FireworksImpl::deserialize(seq)?.to_dyn()),
        DataComponent::ItemModel => Ok(ItemModelImpl::deserialize(seq)?.to_dyn()),
        DataComponent::ItemName => Ok(ItemNameImpl::deserialize(seq)?.to_dyn()),
        DataComponent::CustomName => Ok(CustomNameImpl::deserialize(seq)?.to_dyn()),
        DataComponent::Consumable => Ok(ConsumableImpl::deserialize(seq)?.to_dyn()),
        DataComponent::Equippable => Ok(EquippableImpl::deserialize(seq)?.to_dyn()),
        DataComponent::StoredEnchantments => Ok(StoredEnchantmentsImpl::deserialize(seq)?.to_dyn()),
        DataComponent::UseCooldown => Ok(UseCooldownImpl::deserialize(seq)?.to_dyn()),
        DataComponent::MapId => Ok(MapIdImpl::deserialize(seq)?.to_dyn()),
        DataComponent::BundleContents => Ok(BundleContentsImpl::deserialize(seq)?.to_dyn()),
        DataComponent::WritableBookContent => {
            Ok(WritableBookContentImpl::deserialize(seq)?.to_dyn())
        }
        DataComponent::WrittenBookContent => {
            Ok(WrittenBookContentImpl::deserialize(seq)?.to_dyn())
        }
        DataComponent::MaxDamage => Ok(MaxDamageImpl::deserialize(seq)?.to_dyn()),
        DataComponent::Food => Ok(FoodImpl::deserialize(seq)?.to_dyn()),
        DataComponent::Glider => Ok(GliderImpl::deserialize(seq)?.to_dyn()),
        DataComponent::IntangibleProjectile => {
            Ok(IntangibleProjectileImpl::deserialize(seq)?.to_dyn())
        }
        DataComponent::PotionDurationScale => {
            Ok(PotionDurationScaleImpl::deserialize(seq)?.to_dyn())
        }
        DataComponent::TooltipStyle => Ok(TooltipStyleImpl::deserialize(seq)?.to_dyn()),
        DataComponent::NoteBlockSound => Ok(NoteBlockSoundImpl::deserialize(seq)?.to_dyn()),
        DataComponent::BaseColor => Ok(BaseColorImpl::deserialize(seq)?.to_dyn()),
        DataComponent::Enchantable => Ok(EnchantableImpl::deserialize(seq)?.to_dyn()),
        DataComponent::Weapon => Ok(WeaponImpl::deserialize(seq)?.to_dyn()),
        DataComponent::OminousBottleAmplifier => {
            Ok(OminousBottleAmplifierImpl::deserialize(seq)?.to_dyn())
        }
        DataComponent::DamageResistant => Ok(DamageResistantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::Instrument => Ok(InstrumentImpl::deserialize(seq)?.to_dyn()),
        DataComponent::VillagerVariant => Ok(VillagerVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::WolfVariant => Ok(WolfVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::WolfSoundVariant => Ok(WolfSoundVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::WolfCollar => Ok(WolfCollarImpl::deserialize(seq)?.to_dyn()),
        DataComponent::FoxVariant => Ok(FoxVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::SalmonSize => Ok(SalmonSizeImpl::deserialize(seq)?.to_dyn()),
        DataComponent::ParrotVariant => Ok(ParrotVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::TropicalFishPattern => {
            Ok(TropicalFishPatternImpl::deserialize(seq)?.to_dyn())
        }
        DataComponent::TropicalFishBaseColor => {
            Ok(TropicalFishBaseColorImpl::deserialize(seq)?.to_dyn())
        }
        DataComponent::TropicalFishPatternColor => {
            Ok(TropicalFishPatternColorImpl::deserialize(seq)?.to_dyn())
        }
        DataComponent::MooshroomVariant => Ok(MooshroomVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::RabbitVariant => Ok(RabbitVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::PigVariant => Ok(PigVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::PigSoundVariant => Ok(PigSoundVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::CowVariant => Ok(CowVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::CowSoundVariant => Ok(CowSoundVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::ChickenVariant => Ok(ChickenVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::ChickenSoundVariant => {
            Ok(ChickenSoundVariantImpl::deserialize(seq)?.to_dyn())
        }
        DataComponent::ZombieNautilusVariant => {
            Ok(ZombieNautilusVariantImpl::deserialize(seq)?.to_dyn())
        }
        DataComponent::FrogVariant => Ok(FrogVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::HorseVariant => Ok(HorseVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::PaintingVariant => Ok(PaintingVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::LlamaVariant => Ok(LlamaVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::AxolotlVariant => Ok(AxolotlVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::CatVariant => Ok(CatVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::CatSoundVariant => Ok(CatSoundVariantImpl::deserialize(seq)?.to_dyn()),
        DataComponent::CatCollar => Ok(CatCollarImpl::deserialize(seq)?.to_dyn()),
        DataComponent::SheepColor => Ok(SheepColorImpl::deserialize(seq)?.to_dyn()),
        DataComponent::ShulkerColor => Ok(ShulkerColorImpl::deserialize(seq)?.to_dyn()),
        _ => Err(ReadingError::Message(format!("component_id_{} (TODO)", id.to_id()))),
    }
}
pub fn serialize(
    id: DataComponent,
    value: &dyn DataComponentImpl,
    seq: &mut impl NetworkWriteExt,
) -> Result<(), WritingError> {
    match id {
        DataComponent::MaxStackSize => get::<MaxStackSizeImpl>(value).serialize(seq),
        DataComponent::CustomData => get::<CustomDataImpl>(value).serialize(seq),
        DataComponent::Enchantments => get::<EnchantmentsImpl>(value).serialize(seq),
        DataComponent::Damage => get::<DamageImpl>(value).serialize(seq),
        DataComponent::Unbreakable => get::<UnbreakableImpl>(value).serialize(seq),
        DataComponent::PotionContents => get::<PotionContentsImpl>(value).serialize(seq),
        DataComponent::DyedColor => get::<DyedColorImpl>(value).serialize(seq),
        DataComponent::SuspiciousStewEffects => {
            get::<SuspiciousStewEffectsImpl>(value).serialize(seq)
        }
        DataComponent::FireworkExplosion => get::<FireworkExplosionImpl>(value).serialize(seq),
        DataComponent::Fireworks => get::<FireworksImpl>(value).serialize(seq),
        DataComponent::ItemModel => get::<ItemModelImpl>(value).serialize(seq),
        DataComponent::ItemName => get::<ItemNameImpl>(value).serialize(seq),
        DataComponent::CustomName => get::<CustomNameImpl>(value).serialize(seq),
        DataComponent::Consumable => get::<ConsumableImpl>(value).serialize(seq),
        DataComponent::Equippable => get::<EquippableImpl>(value).serialize(seq),
        DataComponent::StoredEnchantments => get::<StoredEnchantmentsImpl>(value).serialize(seq),
        DataComponent::UseCooldown => get::<UseCooldownImpl>(value).serialize(seq),
        DataComponent::MapId => get::<MapIdImpl>(value).serialize(seq),
        DataComponent::BundleContents => get::<BundleContentsImpl>(value).serialize(seq),
        DataComponent::WritableBookContent => get::<WritableBookContentImpl>(value).serialize(seq),
        DataComponent::WrittenBookContent => get::<WrittenBookContentImpl>(value).serialize(seq),
        DataComponent::MaxDamage => get::<MaxDamageImpl>(value).serialize(seq),
        DataComponent::Food => get::<FoodImpl>(value).serialize(seq),
        DataComponent::Glider => get::<GliderImpl>(value).serialize(seq),
        DataComponent::IntangibleProjectile => {
            get::<IntangibleProjectileImpl>(value).serialize(seq)
        }
        DataComponent::PotionDurationScale => get::<PotionDurationScaleImpl>(value).serialize(seq),
        DataComponent::TooltipStyle => get::<TooltipStyleImpl>(value).serialize(seq),
        DataComponent::NoteBlockSound => get::<NoteBlockSoundImpl>(value).serialize(seq),
        DataComponent::BaseColor => get::<BaseColorImpl>(value).serialize(seq),
        DataComponent::Enchantable => get::<EnchantableImpl>(value).serialize(seq),
        DataComponent::Weapon => get::<WeaponImpl>(value).serialize(seq),
        DataComponent::OminousBottleAmplifier => {
            get::<OminousBottleAmplifierImpl>(value).serialize(seq)
        }
        DataComponent::DamageResistant => get::<DamageResistantImpl>(value).serialize(seq),
        DataComponent::Instrument => get::<InstrumentImpl>(value).serialize(seq),
        DataComponent::VillagerVariant => get::<VillagerVariantImpl>(value).serialize(seq),
        DataComponent::WolfVariant => get::<WolfVariantImpl>(value).serialize(seq),
        DataComponent::WolfSoundVariant => get::<WolfSoundVariantImpl>(value).serialize(seq),
        DataComponent::WolfCollar => get::<WolfCollarImpl>(value).serialize(seq),
        DataComponent::FoxVariant => get::<FoxVariantImpl>(value).serialize(seq),
        DataComponent::SalmonSize => get::<SalmonSizeImpl>(value).serialize(seq),
        DataComponent::ParrotVariant => get::<ParrotVariantImpl>(value).serialize(seq),
        DataComponent::TropicalFishPattern => get::<TropicalFishPatternImpl>(value).serialize(seq),
        DataComponent::TropicalFishBaseColor => {
            get::<TropicalFishBaseColorImpl>(value).serialize(seq)
        }
        DataComponent::TropicalFishPatternColor => {
            get::<TropicalFishPatternColorImpl>(value).serialize(seq)
        }
        DataComponent::MooshroomVariant => get::<MooshroomVariantImpl>(value).serialize(seq),
        DataComponent::RabbitVariant => get::<RabbitVariantImpl>(value).serialize(seq),
        DataComponent::PigVariant => get::<PigVariantImpl>(value).serialize(seq),
        DataComponent::PigSoundVariant => get::<PigSoundVariantImpl>(value).serialize(seq),
        DataComponent::CowVariant => get::<CowVariantImpl>(value).serialize(seq),
        DataComponent::CowSoundVariant => get::<CowSoundVariantImpl>(value).serialize(seq),
        DataComponent::ChickenVariant => get::<ChickenVariantImpl>(value).serialize(seq),
        DataComponent::ChickenSoundVariant => get::<ChickenSoundVariantImpl>(value).serialize(seq),
        DataComponent::ZombieNautilusVariant => {
            get::<ZombieNautilusVariantImpl>(value).serialize(seq)
        }
        DataComponent::FrogVariant => get::<FrogVariantImpl>(value).serialize(seq),
        DataComponent::HorseVariant => get::<HorseVariantImpl>(value).serialize(seq),
        DataComponent::PaintingVariant => get::<PaintingVariantImpl>(value).serialize(seq),
        DataComponent::LlamaVariant => get::<LlamaVariantImpl>(value).serialize(seq),
        DataComponent::AxolotlVariant => get::<AxolotlVariantImpl>(value).serialize(seq),
        DataComponent::CatVariant => get::<CatVariantImpl>(value).serialize(seq),
        DataComponent::CatSoundVariant => get::<CatSoundVariantImpl>(value).serialize(seq),
        DataComponent::CatCollar => get::<CatCollarImpl>(value).serialize(seq),
        DataComponent::SheepColor => get::<SheepColorImpl>(value).serialize(seq),
        DataComponent::ShulkerColor => get::<ShulkerColorImpl>(value).serialize(seq),
        _ => Err(WritingError::Message(format!(
            "{} not yet implemented",
            id.to_name()
        ))),
    }
}

/// Serializes a component using the payload shape understood by the target
/// Java protocol version.
pub fn serialize_for_version(
    id: DataComponent,
    value: &dyn DataComponentImpl,
    version: &JavaMinecraftVersion,
    seq: &mut impl NetworkWriteExt,
) -> Result<(), WritingError> {
    if *version != JavaMinecraftVersion::V_1_21_4 {
        return serialize(id, value, seq);
    }

    match id {
        // These tooltip flags moved into TooltipDisplay after 1.21.4. Current
        // component values no longer retain the old flag, whose Vanilla
        // default was true (show the tooltip).
        DataComponent::Unbreakable => seq.write_bool(true),
        DataComponent::Enchantments => {
            get::<EnchantmentsImpl>(value).serialize(seq)?;
            seq.write_bool(true)
        }
        DataComponent::StoredEnchantments => {
            get::<StoredEnchantmentsImpl>(value).serialize(seq)?;
            seq.write_bool(true)
        }
        DataComponent::DyedColor => {
            get::<DyedColorImpl>(value).serialize(seq)?;
            seq.write_bool(true)
        }
        // 1.21.4 ends this codec after damage_on_hurt. equip_on_interact,
        // can_be_sheared, and shearing_sound were added later.
        DataComponent::Equippable => {
            let equippable = get::<EquippableImpl>(value);
            seq.write_var_int(&VarInt(equippable.slot.get_slot_index()))?;
            crate::IdOr::<crate::SoundEvent>::write(
                &data_to_proto_sound(&equippable.equip_sound),
                seq,
                |writer, sound| {
                    writer.write_string(&sound.sound_name)?;
                    writer.write_option(&sound.range, |writer, range| writer.write_f32(*range))
                },
            )?;

            seq.write_bool(equippable.asset_id.is_some())?;
            if let Some(asset) = &equippable.asset_id {
                seq.write_string(asset)?;
            }
            seq.write_bool(equippable.camera_overlay.is_some())?;
            if let Some(overlay) = &equippable.camera_overlay {
                seq.write_string(overlay)?;
            }
            seq.write_bool(equippable.allowed_entities.is_some())?;
            if let Some(allowed) = &equippable.allowed_entities {
                serialize_idset(allowed, seq)?;
            }
            seq.write_bool(equippable.dispensable)?;
            seq.write_bool(equippable.swappable)?;
            seq.write_bool(equippable.damage_on_hurt)
        }
        _ => serialize(id, value, seq),
    }
}

/// Deserializes component payloads whose shape differs in Minecraft 1.21.4.
pub fn deserialize_for_version(
    id: DataComponent,
    version: &JavaMinecraftVersion,
    seq: &mut impl NetworkReadExt,
) -> Result<Box<dyn DataComponentImpl>, ReadingError> {
    if *version != JavaMinecraftVersion::V_1_21_4 {
        return deserialize(id, seq);
    }

    match id {
        DataComponent::Unbreakable => {
            let _show_in_tooltip = seq.get_bool()?;
            Ok(UnbreakableImpl.to_dyn())
        }
        DataComponent::Enchantments => {
            let value = EnchantmentsImpl::deserialize(seq)?;
            let _show_in_tooltip = seq.get_bool()?;
            Ok(value.to_dyn())
        }
        DataComponent::StoredEnchantments => {
            let value = StoredEnchantmentsImpl::deserialize(seq)?;
            let _show_in_tooltip = seq.get_bool()?;
            Ok(value.to_dyn())
        }
        DataComponent::DyedColor => {
            let value = DyedColorImpl::deserialize(seq)?;
            let _show_in_tooltip = seq.get_bool()?;
            Ok(value.to_dyn())
        }
        DataComponent::Equippable => {
            let slot_index = seq.get_var_int()?.0;
            let slot = EquipmentSlot::from_slot_index(slot_index).ok_or_else(|| {
                ReadingError::Message(format!("Invalid equipment slot index {slot_index}"))
            })?;
            let equip_sound =
                proto_to_data_sound(&crate::IdOr::<crate::SoundEvent>::read(seq, |reader| {
                    let sound_name = reader.get_str()?.into();
                    let range = reader.get_option(NetworkReadExt::get_f32)?;
                    Ok(crate::SoundEvent { sound_name, range })
                })?)
                .ok_or_else(|| ReadingError::Message("Invalid equippable sound".into()))?;
            let asset_id = seq
                .get_option(NetworkReadExt::get_str)?
                .map(|value| Cow::Owned(value.into()));
            let camera_overlay = seq
                .get_option(NetworkReadExt::get_str)?
                .map(|value| Cow::Owned(value.into()));
            let allowed_entities = if seq.get_bool()? {
                Some(deserialize_idset(seq)?)
            } else {
                None
            };
            let dispensable = seq.get_bool()?;
            let swappable = seq.get_bool()?;
            let damage_on_hurt = seq.get_bool()?;

            Ok(EquippableImpl {
                slot,
                equip_sound,
                asset_id,
                camera_overlay,
                allowed_entities,
                dispensable,
                swappable,
                damage_on_hurt,
                equip_on_interact: false,
                can_be_sheared: false,
                shearing_sound: IdOr::Id(Sound::ItemShearsSnip),
            }
            .to_dyn())
        }
        _ => deserialize(id, seq),
    }
}

impl DataComponentCodec<Self> for MapIdImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_var_int(&VarInt::from(self.id))
    }

    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let id = seq.get_var_int()?.0;
        Ok(Self { id })
    }
}

impl DataComponentCodec<Self> for UseCooldownImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_f32(self.seconds)?;
        seq.write_bool(self.cooldown_group.is_some())?;
        if let Some(group) = &self.cooldown_group {
            seq.write_string(group)?;
        }
        Ok(())
    }

    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        let seconds = seq.get_f32()?;
        let cooldown_group = if seq.get_bool()? {
            Some(seq.get_str()?.into())
        } else {
            None
        };
        Ok(Self {
            seconds,
            cooldown_group,
        })
    }
}

fn deserialize_item_stack_template(
    seq: &mut impl NetworkReadExt,
) -> Result<pumpkin_data::item_stack::ItemStack, ReadingError> {
    const MAX_COMPONENTS: i32 = 256;

    let item_id = seq.get_var_int()?.0 as u16;

    let count = seq.get_var_int()?.0 as u8;

    let num_to_add = seq.get_var_int()?.0;
    let num_to_remove = seq.get_var_int()?.0;

    if num_to_add < 0 || num_to_remove < 0 {
        return Err(ReadingError::Message("Negative component count".into()));
    }

    let total_components = num_to_add
        .checked_add(num_to_remove)
        .ok_or_else(|| ReadingError::Message("Component count overflow".into()))?;

    if total_components > MAX_COMPONENTS {
        return Err(ReadingError::Message(
            "Too many components in ItemStackTemplate patch".into(),
        ));
    }

    let mut patch = Vec::with_capacity((num_to_add + num_to_remove) as usize);

    for _ in 0..num_to_add {
        let id_val = seq.get_var_int()?.0;
        let id = DataComponent::try_from_id(id_val as u8)
            .ok_or_else(|| ReadingError::Message(format!("Unknown component ID: {id_val}")))?;

        let _byte_len = seq.get_var_int()?;

        let component_impl = deserialize(id, seq)?;
        patch.push((id, Some(component_impl)));
    }

    for _ in 0..num_to_remove {
        let id_val = seq.get_var_int()?.0;
        let id = DataComponent::try_from_id(id_val as u8)
            .ok_or_else(|| ReadingError::Message("Unknown component ID".into()))?;
        patch.push((id, None));
    }

    Ok(pumpkin_data::item_stack::ItemStack::new_with_component(
        count,
        pumpkin_data::item::Item::from_id(item_id).unwrap_or(&pumpkin_data::item::Item::AIR),
        patch,
    ))
}

fn serialize_item_stack_template(
    stack: &pumpkin_data::item_stack::ItemStack,
    seq: &mut impl NetworkWriteExt,
) -> Result<(), WritingError> {
    seq.write_var_int(&VarInt::from(stack.item.id))?;
    seq.write_var_int(&VarInt::from(stack.item_count))?;

    let mut to_add = 0u8;
    let mut to_remove = 0u8;
    for (_id, data) in &stack.patch {
        if data.is_none() {
            to_remove += 1;
        } else {
            to_add += 1;
        }
    }

    seq.write_var_int(&VarInt::from(to_add))?;
    seq.write_var_int(&VarInt::from(to_remove))?;

    for (id, data) in &stack.patch {
        if let Some(data) = data {
            seq.write_var_int(&VarInt::from(id.to_id()))?;
            serialize(*id, data.as_ref(), seq)?;
        }
    }

    for (id, data) in &stack.patch {
        if data.is_none() {
            seq.write_var_int(&VarInt::from(id.to_id()))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod version_tests {
    use pumpkin_data::data_component::DataComponent;
    use pumpkin_data::data_component_impl::{
        DyedColorImpl, EnchantmentsImpl, EquipmentSlot, EquippableImpl, IdOr, UnbreakableImpl,
        WritableBookContentImpl, WrittenBookContentImpl, get,
    };
    use pumpkin_data::sound::Sound;
    use pumpkin_nbt::{NbtCompound, tag::NbtTag};
    use pumpkin_util::version::JavaMinecraftVersion;

    use super::{deserialize_for_version, serialize, serialize_for_version};

    #[test]
    fn vanilla_1_21_4_restores_component_tooltip_flags() {
        let version = JavaMinecraftVersion::V_1_21_4;

        let mut unbreakable = Vec::new();
        serialize_for_version(
            DataComponent::Unbreakable,
            &UnbreakableImpl,
            &version,
            &mut unbreakable,
        )
        .unwrap();
        assert_eq!(unbreakable, [1]);

        let mut enchantments = Vec::new();
        serialize_for_version(
            DataComponent::Enchantments,
            &EnchantmentsImpl::default(),
            &version,
            &mut enchantments,
        )
        .unwrap();
        assert_eq!(enchantments, [0, 1]);

        let mut dyed_color = Vec::new();
        serialize_for_version(
            DataComponent::DyedColor,
            &DyedColorImpl { rgb: 0x12_34_56 },
            &version,
            &mut dyed_color,
        )
        .unwrap();
        assert_eq!(dyed_color, [0, 0x12, 0x34, 0x56, 1]);
    }

    #[test]
    fn vanilla_1_21_4_equippable_omits_newer_tail_fields() {
        let value = EquippableImpl {
            slot: &EquipmentSlot::HEAD,
            equip_sound: IdOr::Id(Sound::ItemArmorEquipGeneric),
            asset_id: None,
            camera_overlay: None,
            allowed_entities: None,
            dispensable: true,
            swappable: false,
            damage_on_hurt: true,
            equip_on_interact: true,
            can_be_sheared: true,
            shearing_sound: IdOr::Id(Sound::ItemShearsSnip),
        };

        let mut vanilla_1_21_4 = Vec::new();
        serialize_for_version(
            DataComponent::Equippable,
            &value,
            &JavaMinecraftVersion::V_1_21_4,
            &mut vanilla_1_21_4,
        )
        .unwrap();

        let mut current = Vec::new();
        serialize(DataComponent::Equippable, &value, &mut current).unwrap();

        assert!(current.starts_with(&vanilla_1_21_4));
        assert!(current.len() > vanilla_1_21_4.len());
        assert!(vanilla_1_21_4.ends_with(&[0, 0, 0, 1, 0, 1]));
    }

    #[test]
    fn vanilla_1_21_4_writable_book_content_round_trips_filtered_pages() {
        let version = JavaMinecraftVersion::V_1_21_4;
        let expected = WritableBookContentImpl {
            pages: vec!["plain page".into(), "second page".into()],
            filtered_pages: vec![None, Some("filtered page".into())],
        };
        let mut encoded = Vec::new();
        serialize_for_version(
            DataComponent::WritableBookContent,
            &expected,
            &version,
            &mut encoded,
        )
        .unwrap();

        let mut input = encoded.as_slice();
        let decoded =
            deserialize_for_version(DataComponent::WritableBookContent, &version, &mut input)
                .unwrap();

        assert!(input.is_empty());
        assert_eq!(get::<WritableBookContentImpl>(decoded.as_ref()), &expected);
    }

    #[test]
    fn vanilla_1_21_4_written_book_content_round_trips_network_nbt() {
        let version = JavaMinecraftVersion::V_1_21_4;
        let mut rich_page = NbtCompound::new();
        rich_page.put_string("text", "click me".into());
        rich_page.put_string("color", "gold".into());
        let mut click_event = NbtCompound::new();
        click_event.put_string("action", "run_command".into());
        click_event.put_string("command", "/say parity".into());
        rich_page.put_compound("click_event", click_event);

        let expected = WrittenBookContentImpl {
            title: "Parity Book".into(),
            filtered_title: Some("Filtered Title".into()),
            author: "BookBot".into(),
            generation: 2,
            pages: vec![
                NbtTag::Compound(rich_page),
                NbtTag::String("emoji 🎃".into()),
            ],
            filtered_pages: vec![None, Some(NbtTag::String("filtered page".into()))],
            resolved: true,
        };
        let mut encoded = Vec::new();
        serialize_for_version(
            DataComponent::WrittenBookContent,
            &expected,
            &version,
            &mut encoded,
        )
        .unwrap();

        let mut input = encoded.as_slice();
        let decoded =
            deserialize_for_version(DataComponent::WrittenBookContent, &version, &mut input)
                .unwrap();

        assert!(input.is_empty());
        assert_eq!(get::<WrittenBookContentImpl>(decoded.as_ref()), &expected);
    }

    #[test]
    fn vanilla_1_21_4_newly_supported_codecs_round_trip() {
        use pumpkin_data::data_component_impl::{
            BaseColorImpl, DamageResistantImpl, DamageResistantType, EnchantableImpl, FoodImpl,
            GliderImpl, InstrumentImpl, IntangibleProjectileImpl, MaxDamageImpl,
            NoteBlockSoundImpl, OminousBottleAmplifierImpl, PotionDurationScaleImpl,
            SheepColorImpl, TooltipStyleImpl, VillagerVariantImpl, WeaponImpl,
        };

        let version = JavaMinecraftVersion::V_1_21_4;

        // MaxDamage
        let max_dmg = MaxDamageImpl { max_damage: 1561 };
        let mut buf = Vec::new();
        serialize_for_version(DataComponent::MaxDamage, &max_dmg, &version, &mut buf).unwrap();
        let decoded =
            deserialize_for_version(DataComponent::MaxDamage, &version, &mut buf.as_slice())
                .unwrap();
        assert_eq!(get::<MaxDamageImpl>(decoded.as_ref()), &max_dmg);

        // Food
        let food = FoodImpl {
            nutrition: 8,
            saturation: 12.8,
            can_always_eat: true,
        };
        let mut buf = Vec::new();
        serialize_for_version(DataComponent::Food, &food, &version, &mut buf).unwrap();
        let decoded =
            deserialize_for_version(DataComponent::Food, &version, &mut buf.as_slice()).unwrap();
        assert_eq!(get::<FoodImpl>(decoded.as_ref()), &food);

        // Glider
        let glider = GliderImpl;
        let mut buf = Vec::new();
        serialize_for_version(DataComponent::Glider, &glider, &version, &mut buf).unwrap();
        let decoded =
            deserialize_for_version(DataComponent::Glider, &version, &mut buf.as_slice()).unwrap();
        assert_eq!(get::<GliderImpl>(decoded.as_ref()), &glider);

        // IntangibleProjectile
        let intangible = IntangibleProjectileImpl;
        let mut buf = Vec::new();
        serialize_for_version(
            DataComponent::IntangibleProjectile,
            &intangible,
            &version,
            &mut buf,
        )
        .unwrap();
        let decoded = deserialize_for_version(
            DataComponent::IntangibleProjectile,
            &version,
            &mut buf.as_slice(),
        )
        .unwrap();
        assert_eq!(
            get::<IntangibleProjectileImpl>(decoded.as_ref()),
            &intangible
        );

        // PotionDurationScale
        let scale = PotionDurationScaleImpl { scale: 0.125 };
        let mut buf = Vec::new();
        serialize_for_version(
            DataComponent::PotionDurationScale,
            &scale,
            &version,
            &mut buf,
        )
        .unwrap();
        let decoded = deserialize_for_version(
            DataComponent::PotionDurationScale,
            &version,
            &mut buf.as_slice(),
        )
        .unwrap();
        assert_eq!(get::<PotionDurationScaleImpl>(decoded.as_ref()), &scale);

        // TooltipStyle
        let style = TooltipStyleImpl {
            id: "custom_style".into(),
        };
        let mut buf = Vec::new();
        serialize_for_version(DataComponent::TooltipStyle, &style, &version, &mut buf).unwrap();
        let decoded =
            deserialize_for_version(DataComponent::TooltipStyle, &version, &mut buf.as_slice())
                .unwrap();
        assert_eq!(get::<TooltipStyleImpl>(decoded.as_ref()), &style);

        // NoteBlockSound
        let note = NoteBlockSoundImpl {
            sound: "minecraft:block.note_block.harp".into(),
        };
        let mut buf = Vec::new();
        serialize_for_version(DataComponent::NoteBlockSound, &note, &version, &mut buf).unwrap();
        let decoded =
            deserialize_for_version(DataComponent::NoteBlockSound, &version, &mut buf.as_slice())
                .unwrap();
        assert_eq!(get::<NoteBlockSoundImpl>(decoded.as_ref()), &note);

        // BaseColor
        let color = BaseColorImpl {
            color: "red".into(),
        };
        let mut buf = Vec::new();
        serialize_for_version(DataComponent::BaseColor, &color, &version, &mut buf).unwrap();
        let decoded =
            deserialize_for_version(DataComponent::BaseColor, &version, &mut buf.as_slice())
                .unwrap();
        assert_eq!(get::<BaseColorImpl>(decoded.as_ref()), &color);

        // Enchantable
        let enc = EnchantableImpl { value: 15 };
        let mut buf = Vec::new();
        serialize_for_version(DataComponent::Enchantable, &enc, &version, &mut buf).unwrap();
        let decoded =
            deserialize_for_version(DataComponent::Enchantable, &version, &mut buf.as_slice())
                .unwrap();
        assert_eq!(get::<EnchantableImpl>(decoded.as_ref()), &enc);

        // Weapon
        let weapon = WeaponImpl {
            item_damage_per_attack: 2,
        };
        let mut buf = Vec::new();
        serialize_for_version(DataComponent::Weapon, &weapon, &version, &mut buf).unwrap();
        let decoded =
            deserialize_for_version(DataComponent::Weapon, &version, &mut buf.as_slice()).unwrap();
        assert_eq!(get::<WeaponImpl>(decoded.as_ref()), &weapon);

        // OminousBottleAmplifier
        let ominous = OminousBottleAmplifierImpl { amplifier: 4 };
        let mut buf = Vec::new();
        serialize_for_version(
            DataComponent::OminousBottleAmplifier,
            &ominous,
            &version,
            &mut buf,
        )
        .unwrap();
        let decoded = deserialize_for_version(
            DataComponent::OminousBottleAmplifier,
            &version,
            &mut buf.as_slice(),
        )
        .unwrap();
        assert_eq!(
            get::<OminousBottleAmplifierImpl>(decoded.as_ref()),
            &ominous
        );

        // DamageResistant
        let dmg_res = DamageResistantImpl {
            res_type: DamageResistantType::Fire,
        };
        let mut buf = Vec::new();
        serialize_for_version(DataComponent::DamageResistant, &dmg_res, &version, &mut buf)
            .unwrap();
        let decoded = deserialize_for_version(
            DataComponent::DamageResistant,
            &version,
            &mut buf.as_slice(),
        )
        .unwrap();
        assert_eq!(get::<DamageResistantImpl>(decoded.as_ref()), &dmg_res);

        // Instrument
        let inst = InstrumentImpl {
            instrument: std::borrow::Cow::Borrowed("ponder_goat_horn"),
        };
        let mut buf = Vec::new();
        serialize_for_version(DataComponent::Instrument, &inst, &version, &mut buf).unwrap();
        let decoded =
            deserialize_for_version(DataComponent::Instrument, &version, &mut buf.as_slice())
                .unwrap();
        assert_eq!(get::<InstrumentImpl>(decoded.as_ref()), &inst);

        // VillagerVariant
        let villager = VillagerVariantImpl {
            value: std::borrow::Cow::Borrowed("plains"),
        };
        let mut buf = Vec::new();
        serialize_for_version(
            DataComponent::VillagerVariant,
            &villager,
            &version,
            &mut buf,
        )
        .unwrap();
        let decoded = deserialize_for_version(
            DataComponent::VillagerVariant,
            &version,
            &mut buf.as_slice(),
        )
        .unwrap();
        assert_eq!(get::<VillagerVariantImpl>(decoded.as_ref()), &villager);

        // SheepColor
        let sheep = SheepColorImpl {
            value: std::borrow::Cow::Borrowed("pink"),
        };
        let mut buf = Vec::new();
        serialize_for_version(DataComponent::SheepColor, &sheep, &version, &mut buf).unwrap();
        let decoded =
            deserialize_for_version(DataComponent::SheepColor, &version, &mut buf.as_slice())
                .unwrap();
        assert_eq!(get::<SheepColorImpl>(decoded.as_ref()), &sheep);
    }
}

impl DataComponentCodec<Self> for BundleContentsImpl {
    fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
        seq.write_var_int(&VarInt::from(self.items.len() as i32))?;
        for item in &self.items {
            serialize_item_stack_template(item, seq)?;
        }
        Ok(())
    }

    fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
        const MAX_BUNDLE_ITEMS: usize = 64;

        let len = seq.get_var_int()?.0 as usize;

        if len > MAX_BUNDLE_ITEMS {
            return Err(ReadingError::Message(
                "Too many items in BundleContents".into(),
            ));
        }

        let mut items = Vec::with_capacity(len);
        for _ in 0..len {
            items.push(deserialize_item_stack_template(seq)?);
        }
        Ok(Self { items })
    }
}

macro_rules! codec_string_variant {
    ($struct_name:ident) => {
        impl DataComponentCodec<Self> for $struct_name {
            fn serialize(&self, seq: &mut impl NetworkWriteExt) -> Result<(), WritingError> {
                seq.write_string(&self.value)
            }
            fn deserialize(seq: &mut impl NetworkReadExt) -> Result<Self, ReadingError> {
                let value = seq.get_str()?;
                Ok(Self {
                    value: Cow::Owned(value.into()),
                })
            }
        }
    };
}

codec_string_variant!(VillagerVariantImpl);
codec_string_variant!(WolfVariantImpl);
codec_string_variant!(WolfSoundVariantImpl);
codec_string_variant!(WolfCollarImpl);
codec_string_variant!(FoxVariantImpl);
codec_string_variant!(SalmonSizeImpl);
codec_string_variant!(ParrotVariantImpl);
codec_string_variant!(TropicalFishPatternImpl);
codec_string_variant!(TropicalFishBaseColorImpl);
codec_string_variant!(TropicalFishPatternColorImpl);
codec_string_variant!(MooshroomVariantImpl);
codec_string_variant!(RabbitVariantImpl);
codec_string_variant!(PigVariantImpl);
codec_string_variant!(PigSoundVariantImpl);
codec_string_variant!(CowVariantImpl);
codec_string_variant!(CowSoundVariantImpl);
codec_string_variant!(ChickenVariantImpl);
codec_string_variant!(ChickenSoundVariantImpl);
codec_string_variant!(ZombieNautilusVariantImpl);
codec_string_variant!(FrogVariantImpl);
codec_string_variant!(HorseVariantImpl);
codec_string_variant!(PaintingVariantImpl);
codec_string_variant!(LlamaVariantImpl);
codec_string_variant!(AxolotlVariantImpl);
codec_string_variant!(CatVariantImpl);
codec_string_variant!(CatSoundVariantImpl);
codec_string_variant!(CatCollarImpl);
codec_string_variant!(SheepColorImpl);
codec_string_variant!(ShulkerColorImpl);
