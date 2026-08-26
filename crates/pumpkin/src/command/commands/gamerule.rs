use pumpkin_data::game_rules::{GameRule, GameRuleRegistry, GameRuleValue};

use crate::command::args::FindArg;
use crate::command::args::bool::BoolArgConsumer;
use crate::command::args::bounded_num::BoundedNumArgumentConsumer;

use crate::TextComponent;

use crate::command::args::ConsumedArgs;
use crate::command::tree::CommandTree;
use crate::command::tree::builder::{argument, literal};
use crate::command::{CommandExecutor, CommandResult, CommandSender};

const NAMES: [&str; 1] = ["gamerule"];

const DESCRIPTION: &str = "Sets or queries a game rule value.";

const ARG_NAME: &str = "value";

#[derive(Clone, Copy)]
enum RuleTransform {
    Direct,
    InvertedBool,
    FireTick,
}

#[derive(Clone)]
struct RuleSpec {
    name: &'static str,
    rule: GameRule,
    transform: RuleTransform,
    integer_bounds: Option<(i32, i32)>,
}

fn direct(name: &'static str, rule: GameRule) -> RuleSpec {
    RuleSpec {
        name,
        rule,
        transform: RuleTransform::Direct,
        integer_bounds: None,
    }
}

fn integer(name: &'static str, rule: GameRule) -> RuleSpec {
    RuleSpec {
        name,
        rule,
        transform: RuleTransform::Direct,
        integer_bounds: None,
    }
}

fn bounded_integer(name: &'static str, rule: GameRule, min: i32, max: i32) -> RuleSpec {
    RuleSpec {
        name,
        rule,
        transform: RuleTransform::Direct,
        integer_bounds: Some((min, max)),
    }
}

fn inverted(name: &'static str, rule: GameRule) -> RuleSpec {
    RuleSpec {
        name,
        rule,
        transform: RuleTransform::InvertedBool,
        integer_bounds: None,
    }
}

fn vanilla_1_21_4_rules() -> Vec<RuleSpec> {
    vec![
        direct("announceAdvancements", GameRule::ShowAdvancementMessages),
        direct("blockExplosionDropDecay", GameRule::BlockExplosionDropDecay),
        direct("commandBlockOutput", GameRule::CommandBlockOutput),
        integer(
            "commandModificationBlockLimit",
            GameRule::MaxBlockModifications,
        ),
        inverted("disableElytraMovementCheck", GameRule::ElytraMovementCheck),
        inverted("disablePlayerMovementCheck", GameRule::PlayerMovementCheck),
        inverted("disableRaids", GameRule::Raids),
        direct("doDaylightCycle", GameRule::AdvanceTime),
        direct("doEntityDrops", GameRule::EntityDrops),
        RuleSpec {
            name: "doFireTick",
            rule: GameRule::FireSpreadRadiusAroundPlayer,
            transform: RuleTransform::FireTick,
            integer_bounds: None,
        },
        direct("doImmediateRespawn", GameRule::ImmediateRespawn),
        direct("doInsomnia", GameRule::SpawnPhantoms),
        direct("doLimitedCrafting", GameRule::LimitedCrafting),
        direct("doMobLoot", GameRule::MobDrops),
        direct("doMobSpawning", GameRule::SpawnMobs),
        direct("doPatrolSpawning", GameRule::SpawnPatrols),
        direct("doTileDrops", GameRule::BlockDrops),
        direct("doTraderSpawning", GameRule::SpawnWanderingTraders),
        direct("doVinesSpread", GameRule::SpreadVines),
        direct("doWardenSpawning", GameRule::SpawnWardens),
        direct("doWeatherCycle", GameRule::AdvanceWeather),
        direct("drowningDamage", GameRule::DrowningDamage),
        direct(
            "enderPearlsVanishOnDeath",
            GameRule::EnderPearlsVanishOnDeath,
        ),
        direct("fallDamage", GameRule::FallDamage),
        direct("fireDamage", GameRule::FireDamage),
        direct("forgiveDeadPlayers", GameRule::ForgiveDeadPlayers),
        direct("freezeDamage", GameRule::FreezeDamage),
        direct("globalSoundEvents", GameRule::GlobalSoundEvents),
        direct("keepInventory", GameRule::KeepInventory),
        direct("lavaSourceConversion", GameRule::LavaSourceConversion),
        direct("logAdminCommands", GameRule::LogAdminCommands),
        integer("maxCommandChainLength", GameRule::MaxCommandSequenceLength),
        integer("maxCommandForkCount", GameRule::MaxCommandForks),
        integer("maxEntityCramming", GameRule::MaxEntityCramming),
        direct("mobExplosionDropDecay", GameRule::MobExplosionDropDecay),
        direct("mobGriefing", GameRule::MobGriefing),
        direct("naturalRegeneration", GameRule::NaturalHealthRegeneration),
        integer(
            "playersNetherPortalCreativeDelay",
            GameRule::PlayersNetherPortalCreativeDelay,
        ),
        integer(
            "playersNetherPortalDefaultDelay",
            GameRule::PlayersNetherPortalDefaultDelay,
        ),
        integer(
            "playersSleepingPercentage",
            GameRule::PlayersSleepingPercentage,
        ),
        direct(
            "projectilesCanBreakBlocks",
            GameRule::ProjectilesCanBreakBlocks,
        ),
        integer("randomTickSpeed", GameRule::RandomTickSpeed),
        direct("reducedDebugInfo", GameRule::ReducedDebugInfo),
        direct("sendCommandFeedback", GameRule::SendCommandFeedback),
        direct("showDeathMessages", GameRule::ShowDeathMessages),
        integer(
            "snowAccumulationHeight",
            GameRule::MaxSnowAccumulationHeight,
        ),
        bounded_integer("spawnChunkRadius", GameRule::SpawnChunkRadius, 0, 32),
        integer("spawnRadius", GameRule::RespawnRadius),
        direct(
            "spectatorsGenerateChunks",
            GameRule::SpectatorsGenerateChunks,
        ),
        direct("tntExplosionDropDecay", GameRule::TntExplosionDropDecay),
        direct("universalAnger", GameRule::UniversalAnger),
        direct("waterSourceConversion", GameRule::WaterSourceConversion),
    ]
}

struct QueryExecutor(RuleSpec);

impl CommandExecutor for QueryExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        server: &'a crate::server::Server,
        _args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let key = TextComponent::text(self.0.name);
            let level_info = server.level_info.load();
            let game_rule = level_info.game_rules.get(&self.0.rule);
            let (value, game_rule_i32_value) = match (self.0.transform, game_rule) {
                (RuleTransform::Direct, GameRuleValue::Int(value)) => (
                    value.to_string(),
                    (*value).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                ),
                (RuleTransform::Direct, GameRuleValue::Bool(value)) => {
                    (value.to_string(), *value as i32)
                }
                (RuleTransform::InvertedBool, GameRuleValue::Bool(value)) => {
                    ((!*value).to_string(), (!*value) as i32)
                }
                (RuleTransform::FireTick, GameRuleValue::Int(value)) => {
                    let enabled = *value >= 0;
                    (enabled.to_string(), enabled as i32)
                }
                _ => unreachable!("invalid 1.21.4 gamerule transform"),
            };
            let value = TextComponent::text(value);
            drop(level_info);

            sender
                .send_message(TextComponent::translate_cross(
                    "commands.gamerule.query",
                    "commands.gamerule.query",
                    [key, value],
                ))
                .await;

            Ok(game_rule_i32_value)
        })
    }
}

struct SetExecutor(RuleSpec);

impl CommandExecutor for SetExecutor {
    #[expect(unused)]
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        server: &'a crate::server::Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let key = TextComponent::text(self.0.name);
            let current_info = server.level_info.load();

            let mut new_info = (**current_info).clone();

            let mut output_value = String::new();
            let mut result_i32: i32;

            let raw_value = new_info.game_rules.get_mut(&self.0.rule);

            match (self.0.transform, raw_value) {
                (RuleTransform::Direct, GameRuleValue::Int(value)) => {
                    let arg_value = BoundedNumArgumentConsumer::<i32>::find_arg(args, ARG_NAME)??;
                    *value = i64::from(arg_value);
                    output_value = arg_value.to_string();
                    // TODO: Should integer gamerule values be kept as a `i64` or should it be changed to an `i32`?
                    // For now, we can cast it
                    result_i32 = arg_value;
                }
                (RuleTransform::Direct, GameRuleValue::Bool(value)) => {
                    let arg_value = BoolArgConsumer::find_arg(args, ARG_NAME)?;
                    *value = arg_value;
                    output_value = arg_value.to_string();
                    result_i32 = *value as i32;
                }
                (RuleTransform::InvertedBool, GameRuleValue::Bool(value)) => {
                    let arg_value = BoolArgConsumer::find_arg(args, ARG_NAME)?;
                    *value = !arg_value;
                    output_value = arg_value.to_string();
                    result_i32 = arg_value as i32;
                }
                (RuleTransform::FireTick, GameRuleValue::Int(value)) => {
                    let arg_value = BoolArgConsumer::find_arg(args, ARG_NAME)?;
                    *value = if arg_value { 128 } else { -1 };
                    output_value = arg_value.to_string();
                    result_i32 = arg_value as i32;
                }
                _ => unreachable!("invalid 1.21.4 gamerule transform"),
            }

            server.level_info.store(std::sync::Arc::new(new_info));

            let value_component = TextComponent::text(output_value);
            sender
                .send_message(TextComponent::translate_cross(
                    "commands.gamerule.set",
                    "commands.gamerule.set",
                    [key, value_component],
                ))
                .await;

            Ok(result_i32)
        })
    }
}

pub fn init_command_tree() -> CommandTree {
    let mut command_tree = CommandTree::new(NAMES, DESCRIPTION);
    let rule_registry = GameRuleRegistry::default();
    for spec in vanilla_1_21_4_rules() {
        let arg = match (
            spec.transform,
            rule_registry.get(&spec.rule),
            spec.integer_bounds,
        ) {
            (RuleTransform::Direct, GameRuleValue::Int(_), Some((min, max))) => argument(
                ARG_NAME,
                BoundedNumArgumentConsumer::<i32>::new().min(min).max(max),
            ),
            (RuleTransform::Direct, GameRuleValue::Int(_), None) => {
                argument(ARG_NAME, BoundedNumArgumentConsumer::<i32>::new())
            }
            _ => argument(ARG_NAME, BoolArgConsumer),
        };
        command_tree = command_tree.then(
            literal(spec.name)
                .execute(QueryExecutor(spec.clone()))
                .then(arg.execute(SetExecutor(spec))),
        );
    }
    command_tree
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_the_exact_vanilla_1_21_4_rule_names() {
        let rules = vanilla_1_21_4_rules();
        assert_eq!(rules.len(), 52);
        assert!(rules.iter().any(|rule| rule.name == "doEntityDrops"));
        assert!(rules.iter().any(|rule| rule.name == "spawnChunkRadius"));
        assert!(!rules.iter().any(|rule| rule.name == "entity_drops"));
        assert!(!rules.iter().any(|rule| rule.name == "locatorBar"));
        assert_eq!(
            rules
                .iter()
                .find(|rule| rule.name == "spawnChunkRadius")
                .and_then(|rule| rule.integer_bounds),
            Some((0, 32))
        );
    }
}
