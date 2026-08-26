use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, Ordering},
};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::data_component::DataComponent;
use pumpkin_data::data_component_impl::{DataComponentImpl, EquipmentSlot, PotionContentsImpl};
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_protocol::java::client::play::Metadata;

use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, NBTStorage,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, revenge::RevengeGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal, witch_attack::WitchAttackGoal,
    },
    attributes::{Modifier, ModifierOperation},
    mob::{Mob, MobEntity},
};

const DRINKING_SPEED_MODIFIER_ID: &str = "minecraft:drinking";

pub struct WitchEntity {
    pub mob_entity: MobEntity,
    drinking_time: AtomicI32,
    drinking_potion: AtomicI32,
}

impl WitchEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let witch = Self {
            mob_entity,
            drinking_time: AtomicI32::new(0),
            drinking_potion: AtomicI32::new(-1),
        };
        let mob_arc = Arc::new(witch);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(1, Box::new(SwimGoal::default()));
            goal_selector.add_goal(2, Box::new(WitchAttackGoal::new()));
            goal_selector.add_goal(3, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                4,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(4, Box::new(RandomLookAroundGoal::default()));

            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
        };

        mob_arc
    }

    fn potion_stack(potion_id: i32) -> ItemStack {
        let mut stack = ItemStack::new(1, &Item::POTION);
        stack.patch.push((
            DataComponent::PotionContents,
            Some(
                PotionContentsImpl {
                    potion_id: Some(potion_id),
                    custom_color: None,
                    custom_effects: Vec::new(),
                    custom_name: None,
                }
                .to_dyn(),
            ),
        ));
        stack
    }

    async fn start_drinking(&self, potion_id: i32) {
        let stack = Self::potion_stack(potion_id);
        self.drinking_potion.store(potion_id, Ordering::Relaxed);
        self.drinking_time.store(32, Ordering::Relaxed);
        self.mob_entity
            .living_entity
            .set_active_hand(pumpkin_util::Hand::Right, stack.clone(), 32)
            .await;
        self.mob_entity
            .living_entity
            .entity_equipment
            .lock()
            .await
            .equipment
            .insert(EquipmentSlot::MAIN_HAND, stack.clone());
        self.mob_entity
            .living_entity
            .send_equipment_changes(&[(EquipmentSlot::MAIN_HAND, stack)]);
        self.mob_entity
            .living_entity
            .update_attribute(&Attributes::MOVEMENT_SPEED, |attribute| {
                attribute.add_or_replace_modifier(Modifier {
                    id: DRINKING_SPEED_MODIFIER_ID.to_string(),
                    amount: -0.25,
                    operation: ModifierOperation::MultiplyTotal,
                });
            });
        crate::entity::attributes::send_attribute_updates_for_living(
            &self.mob_entity.living_entity,
            vec![Attributes::MOVEMENT_SPEED],
        )
        .await;
        self.get_entity().send_meta_data(
            &[Metadata::new(
                pumpkin_data::tracked_data::witch::DATA_USING_ITEM,
                true,
            )],
            None,
        );
        let world = self.get_entity().world.load();
        world.play_sound(
            Sound::EntityWitchDrink,
            SoundCategory::Hostile,
            &self.get_entity().pos.load(),
        );
    }

    async fn finish_drinking(&self) {
        let potion_id = self.drinking_potion.swap(-1, Ordering::Relaxed);
        self.drinking_time.store(0, Ordering::Relaxed);
        self.mob_entity.living_entity.clear_active_hand().await;
        self.get_entity().send_meta_data(
            &[Metadata::new(
                pumpkin_data::tracked_data::witch::DATA_USING_ITEM,
                false,
            )],
            None,
        );
        self.mob_entity
            .living_entity
            .entity_equipment
            .lock()
            .await
            .equipment
            .insert(EquipmentSlot::MAIN_HAND, ItemStack::EMPTY.clone());
        self.mob_entity
            .living_entity
            .send_equipment_changes(&[(EquipmentSlot::MAIN_HAND, ItemStack::EMPTY.clone())]);
        self.mob_entity
            .living_entity
            .update_attribute(&Attributes::MOVEMENT_SPEED, |attribute| {
                attribute.remove_modifier(DRINKING_SPEED_MODIFIER_ID);
            });
        crate::entity::attributes::send_attribute_updates_for_living(
            &self.mob_entity.living_entity,
            vec![Attributes::MOVEMENT_SPEED],
        )
        .await;

        let stack = Self::potion_stack(potion_id);
        crate::item::potion::PotionContents::apply_effects_to(
            &self.mob_entity.living_entity,
            crate::item::potion::PotionContents::read_potion_effects(&stack),
            1.0,
            crate::item::potion::PotionApplicationSource::Normal,
        )
        .await;
    }
}

impl NBTStorage for WitchEntity {}

impl Mob for WitchEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick<'a>(&'a self, _caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            let remaining = self.drinking_time.load(Ordering::Relaxed);
            if remaining > 0 {
                if self.drinking_time.fetch_sub(1, Ordering::Relaxed) == 1 {
                    self.finish_drinking().await;
                }
                return;
            }

            let living = &self.mob_entity.living_entity;
            let potion = if living.entity.touching_water.load(Ordering::Relaxed)
                && !living.has_effect(&StatusEffect::WATER_BREATHING).await
                && rand::random::<f32>() < 0.15
            {
                Some(pumpkin_data::potion::Potion::WATER_BREATHING.id as i32)
            } else if living.entity.fire_ticks.load(Ordering::Relaxed) > 0
                && !living.has_effect(&StatusEffect::FIRE_RESISTANCE).await
                && rand::random::<f32>() < 0.15
            {
                Some(pumpkin_data::potion::Potion::FIRE_RESISTANCE.id as i32)
            } else if living.health.load() < living.get_max_health() && rand::random::<f32>() < 0.05
            {
                Some(pumpkin_data::potion::Potion::HEALING.id as i32)
            } else {
                let target = self.mob_entity.target.lock().await.clone();
                if let Some(target) = target {
                    let distance = living
                        .entity
                        .pos
                        .load()
                        .squared_distance_to_vec(&target.get_entity().pos.load());
                    if distance > 121.0
                        && !living.has_effect(&StatusEffect::SPEED).await
                        && rand::random::<f32>() < 0.5
                    {
                        Some(pumpkin_data::potion::Potion::SWIFTNESS.id as i32)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some(potion_id) = potion {
                self.start_drinking(potion_id).await;
            }
        })
    }
}
