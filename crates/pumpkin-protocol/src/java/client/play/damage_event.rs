use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use pumpkin_data::packet::clientbound::PLAY_DAMAGE_EVENT;
use pumpkin_macros::java_packet;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::version::JavaMinecraftVersion;

/// Notifies the client that an entity has taken damage.
///
/// This packet is used to trigger damage animations (like the red tint on mobs),
/// directional knockback visuals, and sound effects. It provides the client
/// with specific details about the damage source to ensure the visual feedback
/// matches the cause.
#[java_packet(PLAY_DAMAGE_EVENT)]
pub struct CDamageEvent {
    /// The Entity ID of the entity taking damage.
    pub entity_id: VarInt,
    /// The ID of the damage type (references the `minecraft:damage_type` registry).
    /// Examples: `magic`, `fall`, `on_fire`, or `arrow`.
    pub source_type_id: VarInt,
    /// The Entity ID of the actual cause of the damage (e.g., the player who shot the arrow).
    /// Set to 0 if there is no specific entity cause.
    pub source_cause_id: VarInt,
    /// The Entity ID of the direct damager (e.g., the arrow entity itself).
    /// Set to 0 if this is the same as the cause or if not applicable.
    pub source_direct_id: VarInt,
    /// The coordinates of the damage source. Used by the client to calculate
    /// the direction of the "damage tilt" camera effect.
    pub source_position: Option<Vector3<f64>>,
}

impl CDamageEvent {
    #[must_use]
    pub fn new(
        entity_id: VarInt,
        source_type_id: VarInt,
        source_cause_id: Option<VarInt>,
        source_direct_id: Option<VarInt>,
        source_position: Option<Vector3<f64>>,
    ) -> Self {
        Self {
            entity_id,
            source_type_id,
            source_cause_id: source_cause_id.map_or(VarInt(0), |id| VarInt(id.0 + 1)),
            source_direct_id: source_direct_id.map_or(VarInt(0), |id| VarInt(id.0 + 1)),
            source_position,
        }
    }
}

impl ClientPacket for CDamageEvent {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_var_int(&self.entity_id)?;
        let source_type_id = damage_type_id_for_version(self.source_type_id.0, _version);
        write.write_var_int(&VarInt(source_type_id))?;
        write.write_var_int(&self.source_cause_id)?;
        write.write_var_int(&self.source_direct_id)?;
        if let Some(pos) = &self.source_position {
            write.write_bool(true)?;
            write.write_f64(pos.x)?;
            write.write_f64(pos.y)?;
            write.write_f64(pos.z)?;
        } else {
            write.write_bool(false)?;
        }
        Ok(())
    }
}

fn damage_type_id_for_version(id: i32, version: &JavaMinecraftVersion) -> i32 {
    if *version != JavaMinecraftVersion::V_1_21_4 {
        return id;
    }

    match id {
        // Spear does not exist in 1.21.4; player attack is the closest Vanilla source.
        37 => 34,
        // Sulfur-cube heat does not exist in 1.21.4; use hot floor.
        42 => 20,
        // 26.2 inserted spear at 37 and sulfur_cube_hot at 42.
        38..=41 => id - 1,
        43..=50 => id - 2,
        _ => id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remaps_post_1_21_4_damage_registry_insertions() {
        let version = JavaMinecraftVersion::V_1_21_4;
        assert_eq!(damage_type_id_for_version(49, &version), 47); // wither
        assert_eq!(damage_type_id_for_version(50, &version), 48); // wither skull
        assert_eq!(damage_type_id_for_version(43, &version), 41); // sweet berry bush
        assert_eq!(damage_type_id_for_version(36, &version), 36); // sonic boom
        assert_eq!(damage_type_id_for_version(37, &version), 34); // spear fallback
        assert_eq!(damage_type_id_for_version(42, &version), 20); // sulfur heat fallback
    }
}
