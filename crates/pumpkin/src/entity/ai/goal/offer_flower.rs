use std::sync::Arc;

use pumpkin_data::entity::EntityType;
use pumpkin_util::math::boundingbox::BoundingBox;
use rand::RngExt;

use super::{Controls, Goal, GoalFuture, to_goal_ticks};
use crate::entity::{EntityBase, mob::Mob, passive::iron_golem::IronGolemEntity};

const START_CHANCE: i32 = 8_000;
const SEARCH_XZ: f64 = 6.0;
const SEARCH_Y: f64 = 2.0;
const OFFER_SERVER_TICKS: i32 = 400;

pub struct OfferFlowerGoal {
    golem: Arc<IronGolemEntity>,
    villager: Option<Arc<dyn EntityBase>>,
    ticks_left: i32,
}

impl OfferFlowerGoal {
    #[must_use]
    pub const fn new(golem: Arc<IronGolemEntity>) -> Self {
        Self {
            golem,
            villager: None,
            ticks_left: 0,
        }
    }

    fn search_box(&self) -> BoundingBox {
        self.golem
            .get_entity()
            .bounding_box
            .load()
            .expand(SEARCH_XZ, SEARCH_Y, SEARCH_XZ)
    }
}

impl Goal for OfferFlowerGoal {
    fn can_start<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move {
            let world = mob.get_entity().world.load();
            if world.level_time.lock().await.is_night()
                || mob.get_random().random_range(0..START_CHANCE) != 0
            {
                return false;
            }

            let origin = mob.get_entity().pos.load();
            self.villager = world
                .get_entities_at_box(&self.search_box())
                .into_iter()
                .filter(|entity| entity.get_entity().entity_type == &EntityType::VILLAGER)
                .min_by(|a, b| {
                    a.get_entity()
                        .pos
                        .load()
                        .squared_distance_to_vec(&origin)
                        .total_cmp(&b.get_entity().pos.load().squared_distance_to_vec(&origin))
                });
            self.villager.is_some()
        })
    }

    fn should_continue<'a>(&'a self, _mob: &'a dyn Mob) -> GoalFuture<'a, bool> {
        Box::pin(async move { self.ticks_left > 0 && self.villager.is_some() })
    }

    fn start<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            // Pumpkin currently ticks every running goal each server tick;
            // Vanilla's adjustedTickDelay(400) therefore corresponds to 200.
            self.ticks_left = to_goal_ticks(OFFER_SERVER_TICKS);
            self.golem.set_offering_flower(true);
        })
    }

    fn stop<'a>(&'a mut self, _mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            self.golem.set_offering_flower(false);
            self.villager = None;
        })
    }

    fn tick<'a>(&'a mut self, mob: &'a dyn Mob) -> GoalFuture<'a, ()> {
        Box::pin(async move {
            if let Some(villager) = &self.villager {
                mob.get_mob_entity()
                    .look_control
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .look_at_entity_with_range(villager, 30.0, 30.0);
            }
            self.ticks_left -= 1;
        })
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_offer_flower_probability_and_duration_are_preserved() {
        assert_eq!(START_CHANCE, 8_000);
        assert_eq!(to_goal_ticks(OFFER_SERVER_TICKS), 200);
        assert_eq!((SEARCH_XZ, SEARCH_Y), (6.0, 2.0));
    }
}
