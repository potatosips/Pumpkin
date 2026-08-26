use std::{
    fs::{self, File},
    io::BufWriter,
    path::{Path, PathBuf},
};

use pumpkin_data::game_rules::{GameRule, GameRuleRegistry, GameRuleValue};
use pumpkin_nbt::{compound::NbtCompound, nbt_compress::read_gzip_compound_tag, tag::NbtTag};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::world_info::{WorldGenSettings, WorldInfoError};

#[derive(Clone, Copy)]
enum JavaRuleTransform {
    Direct,
    InvertedBool,
    FireTick,
}

/// The exact Java 1.21.4 gamerule surface and its mapping to Pumpkin's internal
/// (newer cross-edition) registry. Keep this list version-specific.
fn java_1_21_4_game_rules() -> Vec<(&'static str, GameRule, JavaRuleTransform)> {
    use GameRule::*;
    use JavaRuleTransform::{Direct as D, FireTick as F, InvertedBool as I};
    vec![
        ("announceAdvancements", ShowAdvancementMessages, D),
        ("blockExplosionDropDecay", BlockExplosionDropDecay, D),
        ("commandBlockOutput", CommandBlockOutput, D),
        ("commandModificationBlockLimit", MaxBlockModifications, D),
        ("disableElytraMovementCheck", ElytraMovementCheck, I),
        ("disablePlayerMovementCheck", PlayerMovementCheck, I),
        ("disableRaids", Raids, I),
        ("doDaylightCycle", AdvanceTime, D),
        ("doEntityDrops", EntityDrops, D),
        ("doFireTick", FireSpreadRadiusAroundPlayer, F),
        ("doImmediateRespawn", ImmediateRespawn, D),
        ("doInsomnia", SpawnPhantoms, D),
        ("doLimitedCrafting", LimitedCrafting, D),
        ("doMobLoot", MobDrops, D),
        ("doMobSpawning", SpawnMobs, D),
        ("doPatrolSpawning", SpawnPatrols, D),
        ("doTileDrops", BlockDrops, D),
        ("doTraderSpawning", SpawnWanderingTraders, D),
        ("doVinesSpread", SpreadVines, D),
        ("doWardenSpawning", SpawnWardens, D),
        ("doWeatherCycle", AdvanceWeather, D),
        ("drowningDamage", DrowningDamage, D),
        ("enderPearlsVanishOnDeath", EnderPearlsVanishOnDeath, D),
        ("fallDamage", FallDamage, D),
        ("fireDamage", FireDamage, D),
        ("forgiveDeadPlayers", ForgiveDeadPlayers, D),
        ("freezeDamage", FreezeDamage, D),
        ("globalSoundEvents", GlobalSoundEvents, D),
        ("keepInventory", KeepInventory, D),
        ("lavaSourceConversion", LavaSourceConversion, D),
        ("logAdminCommands", LogAdminCommands, D),
        ("maxCommandChainLength", MaxCommandSequenceLength, D),
        ("maxCommandForkCount", MaxCommandForks, D),
        ("maxEntityCramming", MaxEntityCramming, D),
        ("mobExplosionDropDecay", MobExplosionDropDecay, D),
        ("mobGriefing", MobGriefing, D),
        ("naturalRegeneration", NaturalHealthRegeneration, D),
        (
            "playersNetherPortalCreativeDelay",
            PlayersNetherPortalCreativeDelay,
            D,
        ),
        (
            "playersNetherPortalDefaultDelay",
            PlayersNetherPortalDefaultDelay,
            D,
        ),
        ("playersSleepingPercentage", PlayersSleepingPercentage, D),
        ("projectilesCanBreakBlocks", ProjectilesCanBreakBlocks, D),
        ("randomTickSpeed", RandomTickSpeed, D),
        ("reducedDebugInfo", ReducedDebugInfo, D),
        ("sendCommandFeedback", SendCommandFeedback, D),
        ("showDeathMessages", ShowDeathMessages, D),
        ("snowAccumulationHeight", MaxSnowAccumulationHeight, D),
        ("spawnChunkRadius", SpawnChunkRadius, D),
        ("spawnRadius", RespawnRadius, D),
        ("spectatorsGenerateChunks", SpectatorsGenerateChunks, D),
        ("tntExplosionDropDecay", TntExplosionDropDecay, D),
        ("universalAnger", UniversalAnger, D),
        ("waterSourceConversion", WaterSourceConversion, D),
    ]
}

#[must_use]
pub fn java_game_rules_to_nbt(rules: &GameRuleRegistry) -> NbtCompound {
    let mut result = NbtCompound::new();
    for (name, rule, transform) in java_1_21_4_game_rules() {
        let value = match (transform, rules.get(&rule)) {
            (JavaRuleTransform::Direct, GameRuleValue::Bool(value)) => value.to_string(),
            (JavaRuleTransform::Direct, GameRuleValue::Int(value)) => value.to_string(),
            (JavaRuleTransform::InvertedBool, GameRuleValue::Bool(value)) => (!*value).to_string(),
            (JavaRuleTransform::FireTick, GameRuleValue::Int(value)) => (*value >= 0).to_string(),
            _ => unreachable!("invalid Java 1.21.4 gamerule mapping"),
        };
        result.put_string(name, value);
    }
    result
}

pub fn apply_java_game_rules_from_nbt(rules: &mut GameRuleRegistry, nbt: &NbtCompound) {
    for (name, rule, transform) in java_1_21_4_game_rules() {
        let Some(serialized) = nbt.get_string(name) else {
            continue;
        };
        match (transform, rules.get_mut(&rule)) {
            (JavaRuleTransform::Direct, GameRuleValue::Bool(value)) => {
                if let Ok(parsed) = serialized.parse::<bool>() {
                    *value = parsed;
                }
            }
            (JavaRuleTransform::Direct, GameRuleValue::Int(value)) => {
                if let Ok(parsed) = serialized.parse::<i32>() {
                    *value = i64::from(parsed);
                }
            }
            (JavaRuleTransform::InvertedBool, GameRuleValue::Bool(value)) => {
                if let Ok(parsed) = serialized.parse::<bool>() {
                    *value = !parsed;
                }
            }
            (JavaRuleTransform::FireTick, GameRuleValue::Int(value)) => {
                if let Ok(parsed) = serialized.parse::<bool>() {
                    *value = if parsed { 128 } else { -1 };
                }
            }
            _ => unreachable!("invalid Java 1.21.4 gamerule mapping"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct DataFileRoot<T> {
    #[serde(rename = "data")]
    pub data: T,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct WeatherData {
    #[serde(rename = "rain_time", default)]
    pub rain_time: i32,
    #[serde(rename = "raining", default)]
    pub raining: bool,
    #[serde(rename = "thundering", default)]
    pub thundering: bool,
    #[serde(rename = "thunder_time", default)]
    pub thunder_time: i32,
    #[serde(rename = "clear_weather_time", default)]
    pub clear_weather_time: i32,
    #[serde(rename = "DataVersion", default)]
    pub data_version: i32,
}

impl Default for WeatherData {
    fn default() -> Self {
        Self {
            rain_time: 0,
            raining: false,
            thundering: false,
            thunder_time: 0,
            clear_weather_time: -1,
            data_version: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct WorldGenSettingsData {
    #[serde(flatten)]
    pub settings: WorldGenSettings,
    #[serde(rename = "DataVersion", default)]
    pub data_version: i32,
    #[serde(rename = "bonus_chest", default)]
    pub bonus_chest: bool,
    #[serde(rename = "generate_structures", default = "default_true")]
    pub generate_structures: bool,
}

const fn default_true() -> bool {
    true
}

impl WorldGenSettingsData {
    #[must_use]
    pub const fn new(settings: WorldGenSettings, data_version: i32) -> Self {
        Self {
            settings,
            data_version,
            bonus_chest: false,
            generate_structures: true,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DimensionClock {
    pub total_ticks: i64,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct WorldClocksData {
    pub clocks: std::collections::HashMap<String, DimensionClock>,
    pub data_version: i32,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct WanderingTraderData {
    #[serde(rename = "spawn_delay", default = "default_wandering_trader_delay")]
    pub spawn_delay: i32,
    #[serde(rename = "spawn_chance", default = "default_wandering_trader_chance")]
    pub spawn_chance: i32,
    #[serde(rename = "DataVersion", default)]
    pub data_version: i32,
}

const fn default_wandering_trader_delay() -> i32 {
    24_000
}
const fn default_wandering_trader_chance() -> i32 {
    25
}

impl Default for WanderingTraderData {
    fn default() -> Self {
        Self {
            spawn_delay: default_wandering_trader_delay(),
            spawn_chance: default_wandering_trader_chance(),
            data_version: 0,
        }
    }
}

#[must_use]
pub fn minecraft_data_dir(level_folder: &Path) -> PathBuf {
    level_folder.join("data").join("minecraft")
}

/// Ensures the `<world>/data/minecraft/` directory exists.
pub fn ensure_minecraft_data_dir(level_folder: &Path) -> Result<PathBuf, WorldInfoError> {
    let dir = minecraft_data_dir(level_folder);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn read_weather(level_folder: &Path) -> WeatherData {
    let path = minecraft_data_dir(level_folder).join("weather.dat");
    if !path.exists() {
        return WeatherData::default();
    }
    match File::open(&path) {
        Ok(f) => match read_gzip_compound_tag(f) {
            Ok(compound) => {
                let data_compound = compound.get_compound("data");
                let c = data_compound.as_ref().map_or(&compound, |v| v);
                WeatherData {
                    clear_weather_time: c.get_int("clear_weather_time").unwrap_or(0),
                    rain_time: c.get_int("rain_time").unwrap_or(0),
                    thunder_time: c.get_int("thunder_time").unwrap_or(0),
                    raining: c.get_bool("raining").unwrap_or(false),
                    thundering: c.get_bool("thundering").unwrap_or(false),
                    data_version: c.get_int("DataVersion").unwrap_or(0),
                }
            }
            Err(e) => {
                warn!("Failed to deserialize weather.dat, using defaults: {e}");
                WeatherData::default()
            }
        },
        Err(e) => {
            warn!("Failed to open weather.dat, using defaults: {e}");
            WeatherData::default()
        }
    }
}

pub fn write_weather(level_folder: &Path, data: &WeatherData) -> Result<(), WorldInfoError> {
    let dir = ensure_minecraft_data_dir(level_folder)?;
    let path = dir.join("weather.dat");
    let file = File::create(&path)?;
    let mut data_comp = NbtCompound::new();
    data_comp.put_int("clear_weather_time", data.clear_weather_time);
    data_comp.put_int("rain_time", data.rain_time);
    data_comp.put_int("thunder_time", data.thunder_time);
    data_comp.put_bool("raining", data.raining);
    data_comp.put_bool("thundering", data.thundering);
    let mut root = NbtCompound::new();
    root.put_compound("data", data_comp);
    pumpkin_nbt::nbt_compress::write_gzip_compound_tag(root, BufWriter::new(file))
        .map_err(|e| WorldInfoError::SerializationError(e.to_string()))
}

pub fn read_world_gen_settings(level_folder: &Path) -> Option<WorldGenSettings> {
    let path = minecraft_data_dir(level_folder).join("world_gen_settings.dat");
    if !path.exists() {
        return None;
    }
    match File::open(&path) {
        Ok(f) => match read_gzip_compound_tag(f) {
            Ok(compound) => {
                let seed = compound
                    .get_compound("data")
                    .and_then(|c| c.get_long("seed"));
                if seed.is_none() {
                    warn!("world_gen_settings.dat has no seed");
                }
                seed.map(|seed| WorldGenSettings {
                    seed,
                    dimensions: std::collections::HashMap::new(),
                })
            }
            Err(e) => {
                warn!("Failed to deserialize world_gen_settings.dat: {e}");
                None
            }
        },
        Err(e) => {
            warn!("Failed to open world_gen_settings.dat: {e}");
            None
        }
    }
}

pub fn write_world_gen_settings(
    level_folder: &Path,
    settings: &WorldGenSettings,
    data_version: i32,
) -> Result<(), WorldInfoError> {
    let dir = ensure_minecraft_data_dir(level_folder)?;
    let path = dir.join("world_gen_settings.dat");
    let file = File::create(&path)?;
    let mut inner = NbtCompound::new();
    inner.put_int("DataVersion", data_version);
    inner.put_long("seed", settings.seed);

    let mut root = NbtCompound::new();
    root.put_compound("data", inner);
    pumpkin_nbt::nbt_compress::write_gzip_compound_tag(root, BufWriter::new(file))
        .map_err(|e| WorldInfoError::SerializationError(e.to_string()))
}

#[must_use]
pub fn game_rules_to_nbt(rules: &GameRuleRegistry, data_version: i32) -> NbtCompound {
    let mut inner = NbtCompound::new();
    for rule in GameRule::all() {
        let key = format!("minecraft:{rule}");
        match rules.get(rule) {
            GameRuleValue::Bool(b) => inner.put(&key, NbtTag::Byte(i8::from(*b))),
            GameRuleValue::Int(i) => inner.put(&key, NbtTag::Int(*i as i32)),
        }
    }
    inner.put_int("DataVersion", data_version);

    let mut root = NbtCompound::new();
    root.put_compound("data", inner);
    root
}

pub fn game_rules_from_nbt(root: &NbtCompound) -> GameRuleRegistry {
    let mut registry = GameRuleRegistry::default();

    let Some(inner) = root.get_compound("data") else {
        warn!("game_rules.dat missing 'data' compound, using defaults");
        return registry;
    };

    for rule in GameRule::all() {
        let key = format!("minecraft:{rule}");
        match registry.get_mut(rule) {
            GameRuleValue::Bool(b) => {
                if let Some(v) = inner.get_byte(&key) {
                    *b = v != 0;
                }
            }
            GameRuleValue::Int(i) => {
                if let Some(v) = inner.get_int(&key) {
                    *i = i64::from(v);
                }
            }
        }
    }

    registry
}

pub fn read_game_rules(level_folder: &Path) -> GameRuleRegistry {
    let path = minecraft_data_dir(level_folder).join("game_rules.dat");
    if !path.exists() {
        return GameRuleRegistry::default();
    }

    match File::open(&path) {
        Ok(f) => match read_gzip_compound_tag(f) {
            Ok(compound) => game_rules_from_nbt(&compound),
            Err(e) => {
                warn!("Failed to parse game_rules.dat: {e}");
                GameRuleRegistry::default()
            }
        },
        Err(e) => {
            warn!("Failed to open game_rules.dat: {e}");
            GameRuleRegistry::default()
        }
    }
}

pub fn write_game_rules(
    level_folder: &Path,
    rules: &GameRuleRegistry,
    data_version: i32,
) -> Result<(), WorldInfoError> {
    let dir = ensure_minecraft_data_dir(level_folder)?;
    let path = dir.join("game_rules.dat");

    let compound = game_rules_to_nbt(rules, data_version);
    let file = File::create(&path)?;

    pumpkin_nbt::nbt_compress::write_gzip_compound_tag(compound, file)
        .map_err(|e| WorldInfoError::SerializationError(e.to_string()))
}

pub fn read_world_clocks(level_folder: &Path) -> WorldClocksData {
    let path = minecraft_data_dir(level_folder).join("world_clocks.dat");
    if !path.exists() {
        return WorldClocksData::default();
    }

    match File::open(&path) {
        Ok(f) => match read_gzip_compound_tag(f) {
            Ok(compound) => world_clocks_from_nbt(&compound),
            Err(e) => {
                warn!("Failed to parse world_clocks.dat: {e}");
                WorldClocksData::default()
            }
        },
        Err(e) => {
            warn!("Failed to open world_clocks.dat: {e}");
            WorldClocksData::default()
        }
    }
}

fn world_clocks_from_nbt(root: &NbtCompound) -> WorldClocksData {
    let mut result = WorldClocksData::default();

    let Some(inner) = root.get_compound("data") else {
        return result;
    };

    result.data_version = inner.get_int("DataVersion").unwrap_or(0);

    for (key, tag) in &inner.child_tags {
        if key.as_ref() == "DataVersion" {
            continue;
        }
        if let NbtTag::Compound(dim_compound) = tag {
            let total_ticks = dim_compound.get_long("total_ticks").unwrap_or(0);
            result
                .clocks
                .insert(key.to_string(), DimensionClock { total_ticks });
        }
    }

    result
}

pub fn write_world_clocks(
    level_folder: &Path,
    clocks: &WorldClocksData,
) -> Result<(), WorldInfoError> {
    let dir = ensure_minecraft_data_dir(level_folder)?;
    let path = dir.join("world_clocks.dat");

    let mut inner = NbtCompound::new();
    for (dim_name, clock) in &clocks.clocks {
        let mut dim_compound = NbtCompound::new();
        dim_compound.put_long("total_ticks", clock.total_ticks);
        inner.put_compound(dim_name, dim_compound);
    }
    inner.put_int("DataVersion", clocks.data_version);

    let mut root = NbtCompound::new();
    root.put_compound("data", inner);

    let file = File::create(&path)?;

    pumpkin_nbt::nbt_compress::write_gzip_compound_tag(root, file)
        .map_err(|e| WorldInfoError::SerializationError(e.to_string()))
}

pub fn read_wandering_trader(level_folder: &Path) -> WanderingTraderData {
    let path = minecraft_data_dir(level_folder).join("wandering_trader.dat");
    if !path.exists() {
        return WanderingTraderData::default();
    }
    match File::open(&path) {
        Ok(f) => match read_gzip_compound_tag(f) {
            Ok(compound) => {
                let data_compound = compound.get_compound("data");
                let c = data_compound.as_ref().map_or(&compound, |v| v);
                WanderingTraderData {
                    spawn_delay: c.get_int("WanderingTraderSpawnDelay").unwrap_or(24_000),
                    spawn_chance: c.get_int("WanderingTraderSpawnChance").unwrap_or(25),
                    data_version: c.get_int("DataVersion").unwrap_or(0),
                }
            }
            Err(e) => {
                warn!("Failed to deserialize wandering_trader.dat, using defaults: {e}");
                WanderingTraderData::default()
            }
        },
        Err(e) => {
            warn!("Failed to open wandering_trader.dat: {e}");
            WanderingTraderData::default()
        }
    }
}

pub fn write_wandering_trader(
    level_folder: &Path,
    data: &WanderingTraderData,
) -> Result<(), WorldInfoError> {
    let dir = ensure_minecraft_data_dir(level_folder)?;
    let path = dir.join("wandering_trader.dat");
    let file = File::create(&path)?;
    let mut data_comp = NbtCompound::new();
    data_comp.put_int("WanderingTraderSpawnDelay", data.spawn_delay);
    data_comp.put_int("WanderingTraderSpawnChance", data.spawn_chance);
    let mut root = NbtCompound::new();
    root.put_compound("data", data_comp);
    pumpkin_nbt::nbt_compress::write_gzip_compound_tag(root, BufWriter::new(file))
        .map_err(|e| WorldInfoError::SerializationError(e.to_string()))
}

pub fn write_custom_boss_events_stub(
    level_folder: &Path,
    data_version: i32,
) -> Result<(), WorldInfoError> {
    let dir = ensure_minecraft_data_dir(level_folder)?;
    let path = dir.join("custom_boss_events.dat");
    // Only create if absent; actual boss-bar persistence lives elsewhere.
    if path.exists() {
        return Ok(());
    }

    let mut inner = NbtCompound::new();
    inner.put_int("DataVersion", data_version);
    let mut root = NbtCompound::new();
    root.put_compound("data", inner);

    let file = File::create(&path)?;

    pumpkin_nbt::nbt_compress::write_gzip_compound_tag(root, file)
        .map_err(|e| WorldInfoError::SerializationError(e.to_string()))
}

pub fn write_scheduled_events_stub(
    level_folder: &Path,
    data_version: i32,
) -> Result<(), WorldInfoError> {
    let dir = ensure_minecraft_data_dir(level_folder)?;
    let path = dir.join("scheduled_events.dat");
    if path.exists() {
        return Ok(());
    }

    let mut inner = NbtCompound::new();
    inner.put("events", NbtTag::List(vec![]));
    inner.put_int("DataVersion", data_version);
    let mut root = NbtCompound::new();
    root.put_compound("data", inner);

    let file = File::create(&path)?;

    pumpkin_nbt::nbt_compress::write_gzip_compound_tag(root, file)
        .map_err(|e| WorldInfoError::SerializationError(e.to_string()))
}

#[cfg(test)]
mod java_game_rule_tests {
    use super::*;

    #[test]
    fn java_1_21_4_game_rules_use_exact_string_surface() {
        let nbt = java_game_rules_to_nbt(&GameRuleRegistry::default());
        assert_eq!(nbt.child_tags.len(), 52);
        assert_eq!(nbt.get_string("fallDamage"), Some("true"));
        assert_eq!(nbt.get_string("spawnChunkRadius"), Some("2"));
        assert!(nbt.get_string("fall_damage").is_none());
        assert!(nbt.get_string("locatorBar").is_none());
    }

    #[test]
    fn java_1_21_4_game_rules_round_trip_transforms() {
        let mut source = GameRuleRegistry::default();
        source.raids = false;
        source.fire_spread_radius_around_player = -1;
        source.random_tick_speed = -42;

        let nbt = java_game_rules_to_nbt(&source);
        assert_eq!(nbt.get_string("disableRaids"), Some("true"));
        assert_eq!(nbt.get_string("doFireTick"), Some("false"));
        assert_eq!(nbt.get_string("randomTickSpeed"), Some("-42"));

        let mut decoded = GameRuleRegistry::default();
        apply_java_game_rules_from_nbt(&mut decoded, &nbt);
        assert_eq!(decoded, source);
    }
}
