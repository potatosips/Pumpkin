use crate::command::argument_builder::{ArgumentBuilder, argument, command, literal};
use crate::command::argument_types::core::integer::IntegerArgumentType;
use crate::command::argument_types::core::string::StringArgumentType;
use crate::command::argument_types::entity::EntityArgumentType;
use crate::command::argument_types::objective::ObjectiveArgumentType;
use crate::command::context::command_context::CommandContext;
use crate::command::errors::error_types::CommandErrorType;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};
use crate::world::scoreboard::{ScoreboardObjective, ScoreboardScore};
use pumpkin_data::translation;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::RenderType;
use pumpkin_util::PermissionLvl;
use pumpkin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use pumpkin_util::text::TextComponent;

const DESCRIPTION: &str = "Manages scoreboard objectives and players.";
const PERMISSION: &str = "minecraft:command.scoreboard";

const ARG_OBJECTIVE: &str = "objective";
const ARG_CRITERION: &str = "criterion";
const ARG_DISPLAY_NAME: &str = "display_name";
const ARG_TARGETS: &str = "targets";
const ARG_TARGET: &str = "target";
const ARG_SCORE: &str = "score";

const DUPLICATE_OBJECTIVE_ERROR: CommandErrorType<0> = CommandErrorType::new(
    translation::java::COMMANDS_SCOREBOARD_OBJECTIVES_ADD_DUPLICATE,
    translation::java::COMMANDS_SCOREBOARD_OBJECTIVES_ADD_DUPLICATE,
);

const INVALID_ENABLE_ERROR: CommandErrorType<0> = CommandErrorType::new(
    translation::java::COMMANDS_SCOREBOARD_PLAYERS_ENABLE_INVALID,
    translation::java::COMMANDS_SCOREBOARD_PLAYERS_ENABLE_INVALID,
);

const FAILED_ENABLE_ERROR: CommandErrorType<0> = CommandErrorType::new(
    translation::java::COMMANDS_SCOREBOARD_PLAYERS_ENABLE_FAILED,
    translation::java::COMMANDS_SCOREBOARD_PLAYERS_ENABLE_FAILED,
);

struct ObjectivesListExecutor;

impl CommandExecutor for ObjectivesListExecutor {
    fn execute<'a>(&'a self, context: &'a CommandContext) -> CommandExecutorResult<'a> {
        Box::pin(async move {
            let world = context.world();
            let scoreboard = world.scoreboard.lock().await;
            let objectives = scoreboard.get_objectives();
            if objectives.is_empty() {
                context
                    .source
                    .send_feedback(
                        TextComponent::translate_cross(
                            translation::java::COMMANDS_SCOREBOARD_OBJECTIVES_LIST_EMPTY,
                            translation::java::COMMANDS_SCOREBOARD_OBJECTIVES_LIST_EMPTY,
                            [],
                        ),
                        false,
                    )
                    .await;
                return Ok(0);
            }

            let obj_names: Vec<String> = objectives.keys().cloned().collect();
            context
                .source
                .send_feedback(
                    TextComponent::translate_cross(
                        translation::java::COMMANDS_SCOREBOARD_OBJECTIVES_LIST_SUCCESS,
                        translation::java::COMMANDS_SCOREBOARD_OBJECTIVES_LIST_SUCCESS,
                        [
                            TextComponent::text(objectives.len().to_string()),
                            TextComponent::text(obj_names.join(", ")),
                        ],
                    ),
                    false,
                )
                .await;

            Ok(objectives.len() as i32)
        })
    }
}

struct ObjectivesAddExecutor {
    has_display_name: bool,
}

impl CommandExecutor for ObjectivesAddExecutor {
    fn execute<'a>(&'a self, context: &'a CommandContext) -> CommandExecutorResult<'a> {
        Box::pin(async move {
            let objective_name = StringArgumentType::get(context, ARG_OBJECTIVE)?;
            let criterion = StringArgumentType::get(context, ARG_CRITERION)?;

            let display_name = if self.has_display_name {
                TextComponent::text(StringArgumentType::get(context, ARG_DISPLAY_NAME)?.to_string())
            } else {
                TextComponent::text(objective_name.to_string())
            };

            let world = context.world();
            let mut scoreboard = world.scoreboard.lock().await;

            if scoreboard.get_objectives().contains_key(objective_name) {
                return Err(DUPLICATE_OBJECTIVE_ERROR.create_without_context());
            }

            let new_objective = ScoreboardObjective::new(
                objective_name,
                display_name.clone(),
                RenderType::Integer,
                None,
                criterion,
            );

            scoreboard.add_objective(world, new_objective).await;

            context
                .source
                .send_feedback(
                    TextComponent::translate_cross(
                        translation::java::COMMANDS_SCOREBOARD_OBJECTIVES_ADD_SUCCESS,
                        translation::java::COMMANDS_SCOREBOARD_OBJECTIVES_ADD_SUCCESS,
                        [display_name],
                    ),
                    true,
                )
                .await;

            Ok(1)
        })
    }
}

struct PlayersEnableExecutor;

impl CommandExecutor for PlayersEnableExecutor {
    fn execute<'a>(&'a self, context: &'a CommandContext) -> CommandExecutorResult<'a> {
        Box::pin(async move {
            let targets = EntityArgumentType::get_players(context, ARG_TARGETS).await?;
            let objective_name = ObjectiveArgumentType::get(context, ARG_OBJECTIVE)?;

            let world = context.world();
            let mut scoreboard = world.scoreboard.lock().await;

            let objective = scoreboard
                .get_objectives()
                .get(objective_name)
                .ok_or_else(|| INVALID_ENABLE_ERROR.create_without_context())?;

            if objective.criterion != "trigger" {
                return Err(INVALID_ENABLE_ERROR.create_without_context());
            }

            let objective_display_name = objective.display_name.clone();

            let mut enabled_count = 0;
            for player in &targets {
                let player_name = &player.gameprofile.name;
                let current_score = scoreboard
                    .get_scores()
                    .get(objective_name)
                    .and_then(|m| m.get(player_name));

                let is_already_enabled = current_score.is_some_and(|s| !s.locked);

                if !is_already_enabled {
                    let value = current_score.map_or(0, |s| s.value.0);
                    let display_name = current_score.and_then(|s| s.display_name.clone());
                    let number_format = current_score.and_then(|s| s.number_format.clone());

                    let updated_score = ScoreboardScore {
                        entity_name: player_name.clone(),
                        objective_name: objective_name.to_string(),
                        value: VarInt(value),
                        display_name,
                        number_format,
                        locked: false,
                    };

                    scoreboard.update_score(world, updated_score).await;
                    enabled_count += 1;
                }
            }

            if enabled_count == 0 {
                return Err(FAILED_ENABLE_ERROR.create_without_context());
            }

            let msg = if targets.len() == 1 {
                TextComponent::translate_cross(
                    translation::java::COMMANDS_SCOREBOARD_PLAYERS_ENABLE_SUCCESS_SINGLE,
                    translation::java::COMMANDS_SCOREBOARD_PLAYERS_ENABLE_SUCCESS_SINGLE,
                    [
                        objective_display_name,
                        TextComponent::text(targets[0].gameprofile.name.clone()),
                    ],
                )
            } else {
                TextComponent::translate_cross(
                    translation::java::COMMANDS_SCOREBOARD_PLAYERS_ENABLE_SUCCESS_MULTIPLE,
                    translation::java::COMMANDS_SCOREBOARD_PLAYERS_ENABLE_SUCCESS_MULTIPLE,
                    [
                        objective_display_name,
                        TextComponent::text(targets.len().to_string()),
                    ],
                )
            };

            context.source.send_feedback(msg, true).await;

            Ok(enabled_count)
        })
    }
}

struct ObjectivesRemoveExecutor;

impl CommandExecutor for ObjectivesRemoveExecutor {
    fn execute<'a>(&'a self, context: &'a CommandContext) -> CommandExecutorResult<'a> {
        Box::pin(async move {
            let objective_name = ObjectiveArgumentType::get(context, ARG_OBJECTIVE)?;

            let world = context.world();
            let mut scoreboard = world.scoreboard.lock().await;

            let objective = scoreboard
                .get_objectives()
                .get(objective_name)
                .ok_or_else(|| INVALID_ENABLE_ERROR.create_without_context())?;

            let display_name = objective.display_name.clone();

            scoreboard.remove_objective(world, objective_name).await;

            context
                .source
                .send_feedback(
                    TextComponent::translate_cross(
                        translation::java::COMMANDS_SCOREBOARD_OBJECTIVES_REMOVE_SUCCESS,
                        translation::bedrock::COMMANDS_SCOREBOARD_OBJECTIVES_REMOVE_SUCCESS,
                        [display_name],
                    ),
                    true,
                )
                .await;

            Ok(1)
        })
    }
}

struct PlayersGetExecutor;

impl CommandExecutor for PlayersGetExecutor {
    fn execute<'a>(&'a self, context: &'a CommandContext) -> CommandExecutorResult<'a> {
        Box::pin(async move {
            let target_name = StringArgumentType::get(context, ARG_TARGET)?;
            let objective_name = ObjectiveArgumentType::get(context, ARG_OBJECTIVE)?;

            let world = context.world();
            let scoreboard = world.scoreboard.lock().await;

            let objective = scoreboard
                .get_objectives()
                .get(objective_name)
                .ok_or_else(|| INVALID_ENABLE_ERROR.create_without_context())?;

            if let Some(score) = scoreboard.get_score(target_name, objective_name) {
                let score_val = score.value.0;
                context
                    .source
                    .send_feedback(
                        TextComponent::translate_cross(
                            translation::java::COMMANDS_SCOREBOARD_PLAYERS_GET_SUCCESS,
                            translation::java::COMMANDS_SCOREBOARD_PLAYERS_GET_SUCCESS,
                            [
                                TextComponent::text(target_name.to_string()),
                                TextComponent::text(score_val.to_string()),
                                objective.display_name.clone(),
                            ],
                        ),
                        false,
                    )
                    .await;
                Ok(score_val)
            } else {
                context
                    .source
                    .send_feedback(
                        TextComponent::translate_cross(
                            translation::java::COMMANDS_SCOREBOARD_PLAYERS_GET_NULL,
                            translation::java::COMMANDS_SCOREBOARD_PLAYERS_GET_NULL,
                            [
                                objective.display_name.clone(),
                                TextComponent::text(target_name.to_string()),
                            ],
                        ),
                        false,
                    )
                    .await;
                Ok(0)
            }
        })
    }
}

#[derive(Clone, Copy)]
enum ScoreOperation {
    Set,
    Add,
    Remove,
}

struct PlayersModifyScoreExecutor {
    op: ScoreOperation,
}

impl CommandExecutor for PlayersModifyScoreExecutor {
    fn execute<'a>(&'a self, context: &'a CommandContext) -> CommandExecutorResult<'a> {
        Box::pin(async move {
            let targets = EntityArgumentType::get_players(context, ARG_TARGETS).await?;
            let objective_name = ObjectiveArgumentType::get(context, ARG_OBJECTIVE)?;
            let score_delta = IntegerArgumentType::get(context, ARG_SCORE)?;

            let world = context.world();
            let mut scoreboard = world.scoreboard.lock().await;

            let objective = scoreboard
                .get_objectives()
                .get(objective_name)
                .ok_or_else(|| INVALID_ENABLE_ERROR.create_without_context())?;

            let objective_display_name = objective.display_name.clone();

            for player in &targets {
                let player_name = &player.gameprofile.name;
                let current_score = scoreboard
                    .get_scores()
                    .get(objective_name)
                    .and_then(|m| m.get(player_name));

                let old_val = current_score.map_or(0, |s| s.value.0);
                let new_val = match self.op {
                    ScoreOperation::Set => score_delta,
                    ScoreOperation::Add => old_val.saturating_add(score_delta),
                    ScoreOperation::Remove => old_val.saturating_sub(score_delta),
                };

                let updated_score = ScoreboardScore {
                    entity_name: player_name.clone(),
                    objective_name: objective_name.to_string(),
                    value: VarInt(new_val),
                    display_name: current_score.and_then(|s| s.display_name.clone()),
                    number_format: current_score.and_then(|s| s.number_format.clone()),
                    locked: current_score.is_some_and(|s| s.locked),
                };

                scoreboard.update_score(world, updated_score).await;
            }

            let msg = match (self.op, targets.len()) {
                (ScoreOperation::Set, 1) => TextComponent::translate_cross(
                    translation::java::COMMANDS_SCOREBOARD_PLAYERS_SET_SUCCESS_SINGLE,
                    translation::java::COMMANDS_SCOREBOARD_PLAYERS_SET_SUCCESS_SINGLE,
                    [
                        objective_display_name,
                        TextComponent::text(targets[0].gameprofile.name.clone()),
                        TextComponent::text(score_delta.to_string()),
                    ],
                ),
                (ScoreOperation::Set, count) => TextComponent::translate_cross(
                    translation::java::COMMANDS_SCOREBOARD_PLAYERS_SET_SUCCESS_MULTIPLE,
                    translation::java::COMMANDS_SCOREBOARD_PLAYERS_SET_SUCCESS_MULTIPLE,
                    [
                        objective_display_name,
                        TextComponent::text(count.to_string()),
                        TextComponent::text(score_delta.to_string()),
                    ],
                ),
                (ScoreOperation::Add, 1) => TextComponent::translate_cross(
                    translation::java::COMMANDS_SCOREBOARD_PLAYERS_ADD_SUCCESS_SINGLE,
                    translation::java::COMMANDS_SCOREBOARD_PLAYERS_ADD_SUCCESS_SINGLE,
                    [
                        TextComponent::text(score_delta.to_string()),
                        objective_display_name,
                        TextComponent::text(targets[0].gameprofile.name.clone()),
                    ],
                ),
                (ScoreOperation::Add, count) => TextComponent::translate_cross(
                    translation::java::COMMANDS_SCOREBOARD_PLAYERS_ADD_SUCCESS_MULTIPLE,
                    translation::java::COMMANDS_SCOREBOARD_PLAYERS_ADD_SUCCESS_MULTIPLE,
                    [
                        TextComponent::text(score_delta.to_string()),
                        objective_display_name,
                        TextComponent::text(count.to_string()),
                    ],
                ),
                (ScoreOperation::Remove, 1) => TextComponent::translate_cross(
                    translation::java::COMMANDS_SCOREBOARD_PLAYERS_REMOVE_SUCCESS_SINGLE,
                    translation::java::COMMANDS_SCOREBOARD_PLAYERS_REMOVE_SUCCESS_SINGLE,
                    [
                        TextComponent::text(score_delta.to_string()),
                        objective_display_name,
                        TextComponent::text(targets[0].gameprofile.name.clone()),
                    ],
                ),
                (ScoreOperation::Remove, count) => TextComponent::translate_cross(
                    translation::java::COMMANDS_SCOREBOARD_PLAYERS_REMOVE_SUCCESS_MULTIPLE,
                    translation::java::COMMANDS_SCOREBOARD_PLAYERS_REMOVE_SUCCESS_MULTIPLE,
                    [
                        TextComponent::text(score_delta.to_string()),
                        objective_display_name,
                        TextComponent::text(count.to_string()),
                    ],
                ),
            };

            context.source.send_feedback(msg, true).await;
            Ok(targets.len() as i32)
        })
    }
}

struct PlayersResetExecutor {
    has_objective: bool,
}

impl CommandExecutor for PlayersResetExecutor {
    fn execute<'a>(&'a self, context: &'a CommandContext) -> CommandExecutorResult<'a> {
        Box::pin(async move {
            let targets = EntityArgumentType::get_players(context, ARG_TARGETS).await?;
            let world = context.world();
            let mut scoreboard = world.scoreboard.lock().await;

            if self.has_objective {
                let objective_name = ObjectiveArgumentType::get(context, ARG_OBJECTIVE)?;
                let objective = scoreboard
                    .get_objectives()
                    .get(objective_name)
                    .ok_or_else(|| INVALID_ENABLE_ERROR.create_without_context())?;
                let obj_display = objective.display_name.clone();

                for player in &targets {
                    scoreboard
                        .remove_score(world, &player.gameprofile.name, objective_name)
                        .await;
                }

                let msg = if targets.len() == 1 {
                    TextComponent::translate_cross(
                        translation::java::COMMANDS_SCOREBOARD_PLAYERS_RESET_SPECIFIC_SINGLE,
                        translation::java::COMMANDS_SCOREBOARD_PLAYERS_RESET_SPECIFIC_SINGLE,
                        [
                            obj_display,
                            TextComponent::text(targets[0].gameprofile.name.clone()),
                        ],
                    )
                } else {
                    TextComponent::translate_cross(
                        translation::java::COMMANDS_SCOREBOARD_PLAYERS_RESET_SPECIFIC_MULTIPLE,
                        translation::java::COMMANDS_SCOREBOARD_PLAYERS_RESET_SPECIFIC_MULTIPLE,
                        [obj_display, TextComponent::text(targets.len().to_string())],
                    )
                };
                context.source.send_feedback(msg, true).await;
            } else {
                for player in &targets {
                    scoreboard
                        .reset_scores_for_entity(world, &player.gameprofile.name)
                        .await;
                }

                let msg = if targets.len() == 1 {
                    TextComponent::translate_cross(
                        translation::java::COMMANDS_SCOREBOARD_PLAYERS_RESET_ALL_SINGLE,
                        translation::java::COMMANDS_SCOREBOARD_PLAYERS_RESET_ALL_SINGLE,
                        [TextComponent::text(targets[0].gameprofile.name.clone())],
                    )
                } else {
                    TextComponent::translate_cross(
                        translation::java::COMMANDS_SCOREBOARD_PLAYERS_RESET_ALL_MULTIPLE,
                        translation::java::COMMANDS_SCOREBOARD_PLAYERS_RESET_ALL_MULTIPLE,
                        [TextComponent::text(targets.len().to_string())],
                    )
                };
                context.source.send_feedback(msg, true).await;
            }

            Ok(targets.len() as i32)
        })
    }
}

struct PlayersListExecutor {
    has_target: bool,
}

impl CommandExecutor for PlayersListExecutor {
    fn execute<'a>(&'a self, context: &'a CommandContext) -> CommandExecutorResult<'a> {
        Box::pin(async move {
            let world = context.world();
            let scoreboard = world.scoreboard.lock().await;

            if self.has_target {
                let target_name = StringArgumentType::get(context, ARG_TARGET)?;
                let scores = scoreboard.get_scores_for_entity(target_name);
                if scores.is_empty() {
                    context
                        .source
                        .send_feedback(
                            TextComponent::translate_cross(
                                translation::java::COMMANDS_SCOREBOARD_PLAYERS_LIST_ENTITY_EMPTY,
                                translation::java::COMMANDS_SCOREBOARD_PLAYERS_LIST_ENTITY_EMPTY,
                                [TextComponent::text(target_name.to_string())],
                            ),
                            false,
                        )
                        .await;
                    return Ok(0);
                }

                let list_str = scores
                    .iter()
                    .map(|(obj, score)| format!("{}: {}", obj, score.value.0))
                    .collect::<Vec<_>>()
                    .join(", ");

                context
                    .source
                    .send_feedback(
                        TextComponent::translate_cross(
                            translation::java::COMMANDS_SCOREBOARD_PLAYERS_LIST_ENTITY_SUCCESS,
                            translation::java::COMMANDS_SCOREBOARD_PLAYERS_LIST_ENTITY_SUCCESS,
                            [
                                TextComponent::text(scores.len().to_string()),
                                TextComponent::text(target_name.to_string()),
                                TextComponent::text(list_str),
                            ],
                        ),
                        false,
                    )
                    .await;
                Ok(scores.len() as i32)
            } else {
                let mut tracked_entities = std::collections::HashSet::new();
                for obj_scores in scoreboard.get_scores().values() {
                    for entity_name in obj_scores.keys() {
                        tracked_entities.insert(entity_name.clone());
                    }
                }

                if tracked_entities.is_empty() {
                    context
                        .source
                        .send_feedback(
                            TextComponent::translate_cross(
                                translation::java::COMMANDS_SCOREBOARD_PLAYERS_LIST_EMPTY,
                                translation::java::COMMANDS_SCOREBOARD_PLAYERS_LIST_EMPTY,
                                [],
                            ),
                            false,
                        )
                        .await;
                    return Ok(0);
                }

                let entities_vec: Vec<String> = tracked_entities.into_iter().collect();
                context
                    .source
                    .send_feedback(
                        TextComponent::translate_cross(
                            translation::java::COMMANDS_SCOREBOARD_PLAYERS_LIST_SUCCESS,
                            translation::java::COMMANDS_SCOREBOARD_PLAYERS_LIST_SUCCESS,
                            [
                                TextComponent::text(entities_vec.len().to_string()),
                                TextComponent::text(entities_vec.join(", ")),
                            ],
                        ),
                        false,
                    )
                    .await;
                Ok(entities_vec.len() as i32)
            }
        })
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Two),
    ));

    dispatcher.register(
        command("scoreboard", DESCRIPTION)
            .requires(PERMISSION)
            .then(
                literal("objectives")
                    .then(literal("list").executes(ObjectivesListExecutor))
                    .then(
                        literal("add").then(
                            argument(ARG_OBJECTIVE, StringArgumentType::SingleWord).then(
                                argument(ARG_CRITERION, StringArgumentType::SingleWord)
                                    .executes(ObjectivesAddExecutor {
                                        has_display_name: false,
                                    })
                                    .then(
                                        argument(
                                            ARG_DISPLAY_NAME,
                                            StringArgumentType::GreedyPhrase,
                                        )
                                        .executes(
                                            ObjectivesAddExecutor {
                                                has_display_name: true,
                                            },
                                        ),
                                    ),
                            ),
                        ),
                    )
                    .then(
                        literal("remove").then(
                            argument(ARG_OBJECTIVE, ObjectiveArgumentType)
                                .executes(ObjectivesRemoveExecutor),
                        ),
                    ),
            )
            .then(
                literal("players")
                    .then(
                        literal("list")
                            .executes(PlayersListExecutor { has_target: false })
                            .then(
                                argument(ARG_TARGET, StringArgumentType::SingleWord)
                                    .executes(PlayersListExecutor { has_target: true }),
                            ),
                    )
                    .then(
                        literal("get").then(
                            argument(ARG_TARGET, StringArgumentType::SingleWord).then(
                                argument(ARG_OBJECTIVE, ObjectiveArgumentType)
                                    .executes(PlayersGetExecutor),
                            ),
                        ),
                    )
                    .then(literal("set").then(
                        argument(ARG_TARGETS, EntityArgumentType::Players).then(
                            argument(ARG_OBJECTIVE, ObjectiveArgumentType).then(
                                argument(ARG_SCORE, IntegerArgumentType::any()).executes(
                                    PlayersModifyScoreExecutor {
                                        op: ScoreOperation::Set,
                                    },
                                ),
                            ),
                        ),
                    ))
                    .then(literal("add").then(
                        argument(ARG_TARGETS, EntityArgumentType::Players).then(
                            argument(ARG_OBJECTIVE, ObjectiveArgumentType).then(
                                argument(ARG_SCORE, IntegerArgumentType::any()).executes(
                                    PlayersModifyScoreExecutor {
                                        op: ScoreOperation::Add,
                                    },
                                ),
                            ),
                        ),
                    ))
                    .then(literal("remove").then(
                        argument(ARG_TARGETS, EntityArgumentType::Players).then(
                            argument(ARG_OBJECTIVE, ObjectiveArgumentType).then(
                                argument(ARG_SCORE, IntegerArgumentType::any()).executes(
                                    PlayersModifyScoreExecutor {
                                        op: ScoreOperation::Remove,
                                    },
                                ),
                            ),
                        ),
                    ))
                    .then(
                        literal("reset").then(
                            argument(ARG_TARGETS, EntityArgumentType::Players)
                                .executes(PlayersResetExecutor {
                                    has_objective: false,
                                })
                                .then(argument(ARG_OBJECTIVE, ObjectiveArgumentType).executes(
                                    PlayersResetExecutor {
                                        has_objective: true,
                                    },
                                )),
                        ),
                    )
                    .then(
                        literal("enable").then(
                            argument(ARG_TARGETS, EntityArgumentType::Players).then(
                                argument(ARG_OBJECTIVE, ObjectiveArgumentType)
                                    .executes(PlayersEnableExecutor),
                            ),
                        ),
                    ),
            ),
    );
}
