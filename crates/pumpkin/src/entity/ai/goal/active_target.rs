use super::{Controls, Goal, to_goal_ticks};
use crate::entity::ai::goal::GoalFuture;
use crate::entity::ai::goal::track_target::TrackTargetGoal;
use crate::entity::ai::target_predicate::TargetPredicate;
use crate::entity::mob::Mob;
use crate::entity::{EntityBase, mob::MobEntity};
use crate::world::World;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_util::random::RandomImpl;
use std::future::Future;
use std::sync::Arc;

const DEFAULT_RECIPROCAL_CHANCE: i32 = 10;

pub struct ActiveTargetGoal {
    track_target_goal: TrackTargetGoal,
    target: Option<Arc<dyn EntityBase>>,
    reciprocal_chance: i32,
    target_types: Vec<&'static EntityType>,
    target_predicate: TargetPredicate,
    only_if_untamed: bool,
    follow_distance_scale: f64,
}

impl ActiveTargetGoal {
    pub fn new<F, Fut>(
        mob: &MobEntity,
        target_type: &'static EntityType,
        reciprocal_chance: i32,
        check_visibility: bool,
        check_can_navigate: bool,
        predicate: Option<F>,
    ) -> Self
    where
        F: Fn(i32, Arc<World>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = bool> + Send + 'static,
    {
        let track_target_goal = TrackTargetGoal::new(check_visibility, check_can_navigate);
        let mut target_predicate = TargetPredicate::create_attackable();
        target_predicate.base_max_distance = mob
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE);

        if let Some(predicate) = predicate {
            target_predicate.set_predicate(predicate);
        }

        Self {
            track_target_goal,
            target: None,
            reciprocal_chance: to_goal_ticks(reciprocal_chance),
            target_types: vec![target_type],
            target_predicate,
            only_if_untamed: false,
            follow_distance_scale: 1.0,
        }
    }

    #[must_use]
    pub fn with_default(
        mob: &MobEntity,
        target_type: &'static EntityType,
        check_visibility: bool,
    ) -> Box<Self> {
        let track_target_goal = TrackTargetGoal::with_default(check_visibility);
        let mut target_predicate = TargetPredicate::create_attackable();
        target_predicate.base_max_distance = mob
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE);

        Box::new(Self {
            track_target_goal,
            target: None,
            reciprocal_chance: to_goal_ticks(DEFAULT_RECIPROCAL_CHANCE),
            target_types: vec![target_type],
            target_predicate,
            only_if_untamed: false,
            follow_distance_scale: 1.0,
        })
    }

    #[must_use]
    pub fn with_default_untamed(
        mob: &MobEntity,
        target_type: &'static EntityType,
        check_visibility: bool,
    ) -> Box<Self> {
        let mut goal = Self::with_default(mob, target_type, check_visibility);
        goal.only_if_untamed = true;
        goal
    }

    pub fn set_target(&mut self, target: Option<Arc<dyn EntityBase>>) {
        self.target = target;
    }

    #[must_use]
    pub fn new_many(
        mob: &MobEntity,
        target_types: &[&'static EntityType],
        reciprocal_chance: i32,
        check_visibility: bool,
        check_can_navigate: bool,
    ) -> Self {
        let track_target_goal = TrackTargetGoal::new(check_visibility, check_can_navigate);
        let mut target_predicate = TargetPredicate::create_attackable();
        target_predicate.base_max_distance = mob
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE);
        Self {
            track_target_goal,
            target: None,
            reciprocal_chance: to_goal_ticks(reciprocal_chance),
            target_types: target_types.to_vec(),
            target_predicate,
            only_if_untamed: false,
            follow_distance_scale: 1.0,
        }
    }

    #[must_use]
    pub fn with_follow_distance_scale(mut self, scale: f64) -> Self {
        self.follow_distance_scale = scale;
        self.track_target_goal = self.track_target_goal.set_follow_distance_scale(scale);
        self
    }

    async fn find_closest_target(&mut self, mob: &dyn Mob) {
        let mob_entity = mob.get_mob_entity();
        let follow_range = mob_entity
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE)
            * self.follow_distance_scale;

        // Vanilla updates the target conditions with the current follow distance on every search
        self.target_predicate.base_max_distance = follow_range;

        let world = mob_entity.living_entity.entity.world.load();

        // Vanilla searches using getEyeY(), so we offset the position by the eye height
        let mut search_pos = mob_entity.living_entity.entity.pos.load();
        search_pos.y += mob_entity
            .living_entity
            .entity
            .entity_dimension
            .load()
            .eye_height as f64;

        if self.target_types.contains(&&EntityType::PLAYER) {
            let mut candidates = world.get_nearby_players(search_pos, follow_range);
            candidates.sort_by(|a, b| {
                a.get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&search_pos)
                    .total_cmp(
                        &b.get_entity()
                            .pos
                            .load()
                            .squared_distance_to_vec(&search_pos),
                    )
            });
            for player in candidates {
                let potential_entity: Arc<dyn EntityBase> = player;
                let living = potential_entity
                    .get_living_entity()
                    .expect("players are living entities");
                if self
                    .track_target_goal
                    .can_track(mob, Some(living), &self.target_predicate)
                    .await
                {
                    self.target = Some(potential_entity);
                    return;
                }
            }
        } else {
            let search_box = mob_entity.living_entity.entity.bounding_box.load().expand(
                follow_range,
                4.0,
                follow_range,
            );
            let mut candidates: Vec<_> = world
                .get_entities_at_box(&search_box)
                .into_iter()
                .filter(|entity| self.target_types.contains(&entity.get_entity().entity_type))
                .collect();
            candidates.sort_by(|a, b| {
                a.get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&search_pos)
                    .total_cmp(
                        &b.get_entity()
                            .pos
                            .load()
                            .squared_distance_to_vec(&search_pos),
                    )
            });
            for potential_entity in candidates {
                let Some(living) = potential_entity.get_living_entity() else {
                    continue;
                };
                if self
                    .track_target_goal
                    .can_track(mob, Some(living), &self.target_predicate)
                    .await
                {
                    self.target = Some(potential_entity);
                    return;
                }
            }
        }
        self.target = None;
    }
}

impl Goal for ActiveTargetGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async {
            if self.only_if_untamed && mob.is_tame() {
                return false;
            }
            if self.reciprocal_chance > 0
                && mob
                    .get_entity_random()
                    .next_bounded_i32(self.reciprocal_chance)
                    != 0
            {
                return false;
            }
            self.find_closest_target(mob).await;
            self.target.is_some()
        })
    }

    fn should_continue<'a>(&'a self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async {
            if self.only_if_untamed && mob.is_tame() {
                return false;
            }
            self.track_target_goal.should_continue(mob).await
        })
    }

    fn start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async {
            mob.set_mob_target(self.target.clone()).await;
            self.track_target_goal.start(mob).await;
        })
    }

    fn stop<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async {
            self.track_target_goal.stop(mob).await;
        })
    }

    fn controls(&self) -> Controls {
        self.track_target_goal.controls()
    }
}
