use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use crate::entity::projectile::{ProjectileHit, is_projectile};
use crate::{
    entity::{
        Entity, EntityBase, EntityBaseFuture, NBTStorage, experience_orb::ExperienceOrbEntity,
        item::ItemEntity, living::LivingEntity, player::Player,
    },
    server::Server,
};
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{
    Block,
    entity::EntityType,
    item_stack::ItemStack,
    tag::{Fluid::MINECRAFT_WATER, Taggable},
};
use pumpkin_protocol::java::client::play::Metadata;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::vector3::Vector3;

pub struct FishingBobberEntity {
    pub entity: Entity,
    pub owner_id: i32,
    pub hooked_entity_id: AtomicI32,
    pub in_ground: AtomicBool,
    pub has_hit: AtomicBool,
    pub wait_countdown: AtomicI32,
    pub bite_countdown: AtomicI32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FishingLootCategory {
    Junk,
    Treasure,
    Fish,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpenWaterType {
    Invalid,
    AboveWater,
    InsideWater,
}

impl FishingBobberEntity {
    const WATER_INERTIA: f64 = 0.8;
    const AIR_INERTIA: f64 = 0.92;
    const GRAVITY: f64 = 0.03;

    /// Selects from Vanilla's `gameplay/fishing/fish` table. Its four entry
    /// weights are cod 60, salmon 25, pufferfish 13, and tropical fish 2.
    fn fish_for_roll(roll: u32) -> &'static pumpkin_data::item::Item {
        use pumpkin_data::item::Item;
        match roll % 100 {
            0..60 => &Item::COD,
            60..85 => &Item::SALMON,
            85..98 => &Item::PUFFERFISH,
            _ => &Item::TROPICAL_FISH,
        }
    }

    fn category_for_roll(roll: u32, luck: i32, open_water: bool) -> FishingLootCategory {
        let junk_weight = (10 - 2 * luck).max(0) as u32;
        let treasure_weight = if open_water {
            (5 + 2 * luck).max(0) as u32
        } else {
            0
        };
        let fish_weight = (85 - luck).max(0) as u32;
        let total = junk_weight + treasure_weight + fish_weight;
        let roll = roll % total.max(1);
        if roll < junk_weight {
            FishingLootCategory::Junk
        } else if roll < junk_weight + treasure_weight {
            FishingLootCategory::Treasure
        } else {
            FishingLootCategory::Fish
        }
    }

    fn junk_for_roll(roll: u32, in_jungle: bool) -> ItemStack {
        use pumpkin_data::item::Item;
        let (item, count) = match roll % if in_jungle { 110 } else { 100 } {
            0..17 => (&Item::LILY_PAD, 1),
            17..27 => (&Item::LEATHER_BOOTS, 1),
            27..37 => (&Item::LEATHER, 1),
            37..47 => (&Item::BONE, 1),
            47..57 => (&Item::POTION, 1),
            57..62 => (&Item::STRING, 1),
            62..64 => (&Item::FISHING_ROD, 1),
            64..74 => (&Item::BOWL, 1),
            74..79 => (&Item::STICK, 1),
            79 => (&Item::INK_SAC, 10),
            80..90 => (&Item::TRIPWIRE_HOOK, 1),
            90..100 => (&Item::ROTTEN_FLESH, 1),
            _ => (&Item::BAMBOO, 1),
        };
        let mut stack = ItemStack::new(count, item);
        if item.id == Item::LEATHER_BOOTS.id || item.id == Item::FISHING_ROD.id {
            if let Some(max_damage) = stack.get_max_damage().filter(|damage| *damage > 0) {
                stack.set_damage(rand::random_range(0..=(max_damage * 9 / 10)));
            }
        }
        stack
    }

    fn treasure_for_roll(roll: u32) -> ItemStack {
        use pumpkin_data::item::Item;
        let item = match roll % 6 {
            0 => &Item::NAME_TAG,
            1 => &Item::SADDLE,
            2 => &Item::BOW,
            3 => &Item::FISHING_ROD,
            4 => &Item::BOOK,
            _ => &Item::NAUTILUS_SHELL,
        };
        let mut stack = ItemStack::new(1, item);
        if item.id == Item::BOW.id || item.id == Item::FISHING_ROD.id {
            if let Some(max_damage) = stack.get_max_damage().filter(|damage| *damage > 0) {
                stack.set_damage(rand::random_range(0..=(max_damage / 4)));
            }
        }
        if matches!(item.id, id if id == Item::BOW.id || id == Item::FISHING_ROD.id || id == Item::BOOK.id)
        {
            let mut random =
                pumpkin_util::random::legacy_rand::LegacyRand::from_seed(rand::random::<u64>());
            stack = pumpkin_inventory::enchanting::generator::enchant_with_levels(
                stack,
                30,
                "#minecraft:on_random_loot",
                &mut random,
            );
        }
        stack
    }

    fn open_water_type_at(&self, pos: &pumpkin_util::math::position::BlockPos) -> OpenWaterType {
        let world = self.entity.world.load();
        let fluid = world.get_fluid(pos);
        let state = world.get_block_state(pos);
        if fluid.has_tag(&MINECRAFT_WATER)
            && state.get_block_collision_shapes_at(pos).next().is_none()
        {
            OpenWaterType::InsideWater
        } else {
            let block = Block::from_state_id(state.id);
            if block == &Block::AIR || block == &Block::LILY_PAD {
                OpenWaterType::AboveWater
            } else {
                OpenWaterType::Invalid
            }
        }
    }

    fn open_water_type_for_layer(
        &self,
        center: pumpkin_util::math::position::BlockPos,
        y_offset: i32,
    ) -> OpenWaterType {
        let mut layer_type = None;
        for x_offset in -2..=2 {
            for z_offset in -2..=2 {
                let pos = pumpkin_util::math::position::BlockPos::new(
                    center.0.x + x_offset,
                    center.0.y + y_offset,
                    center.0.z + z_offset,
                );
                let cell_type = self.open_water_type_at(&pos);
                if cell_type == OpenWaterType::Invalid {
                    return OpenWaterType::Invalid;
                }
                if let Some(layer_type) = layer_type
                    && layer_type != cell_type
                {
                    return OpenWaterType::Invalid;
                }
                layer_type = Some(cell_type);
            }
        }
        layer_type.unwrap_or(OpenWaterType::Invalid)
    }

    fn is_in_open_water(&self) -> bool {
        let center = self.entity.block_pos.load();
        let layers = (-1..=2)
            .map(|y_offset| self.open_water_type_for_layer(center, y_offset))
            .collect::<Vec<_>>();
        Self::is_open_water_sequence(&layers)
    }

    fn is_open_water_sequence(layers: &[OpenWaterType]) -> bool {
        let mut previous = OpenWaterType::Invalid;
        for &current in layers {
            if current == OpenWaterType::Invalid
                || current == OpenWaterType::AboveWater && previous == OpenWaterType::Invalid
                || current == OpenWaterType::InsideWater && previous == OpenWaterType::AboveWater
            {
                return false;
            }
            previous = current;
        }
        true
    }

    pub fn new(entity: Entity, owner: &Player) -> Self {
        let mut owner_pos = owner.living_entity.entity.pos.load();
        owner_pos.y += owner.living_entity.entity.get_eye_height() - 0.1;
        entity.pos.store(owner_pos);

        Self {
            entity,
            owner_id: owner.living_entity.entity.entity_id,
            hooked_entity_id: AtomicI32::new(0),
            in_ground: AtomicBool::new(false),
            has_hit: AtomicBool::new(false),
            wait_countdown: AtomicI32::new(rand::random::<i32>().abs() % 600 + 100),
            bite_countdown: AtomicI32::new(0),
        }
    }

    pub async fn reel_in(&self, player: &Player, rod: &ItemStack) -> i32 {
        let world = self.entity.world.load();
        let hooked_id = self.hooked_entity_id.load(Ordering::Relaxed);

        if hooked_id != 0
            && let Some(hooked) = world.get_entity_by_id(hooked_id)
        {
            let player_pos = player.get_entity().pos.load();
            let hooked_pos = hooked.get_entity().pos.load();
            let delta = player_pos - hooked_pos;
            let motion =
                delta
                    .multiply(0.1, 0.1, 0.1)
                    .add_raw(0.0, delta.length().sqrt() * 0.08, 0.0);
            hooked.get_entity().add_velocity(motion);
            return 3;
        }

        if self.bite_countdown.load(Ordering::Relaxed) > 0 {
            // Caught something!
            player
                .increment_stat(
                    pumpkin_data::statistic::StatisticCategory::Custom,
                    pumpkin_data::statistic::CustomStatistic::FishCaught as i32,
                    1,
                )
                .await;

            let luck =
                rod.get_enchantment_level(&pumpkin_data::enchantment::Enchantment::LUCK_OF_THE_SEA);
            let category = Self::category_for_roll(rand::random(), luck, self.is_in_open_water());
            let biome = world.level.get_rough_biome(&self.entity.block_pos.load());
            let in_jungle = matches!(
                biome.registry_id,
                "jungle" | "sparse_jungle" | "bamboo_jungle"
            );
            let item_stack = match category {
                FishingLootCategory::Junk => Self::junk_for_roll(rand::random(), in_jungle),
                FishingLootCategory::Treasure => Self::treasure_for_roll(rand::random()),
                FishingLootCategory::Fish => ItemStack::new(1, Self::fish_for_roll(rand::random())),
            };

            // Vanilla spawns the catch at the hook and pulls it toward the
            // player instead of inserting it directly into the inventory.
            let hook_pos = self.entity.pos.load();
            let player_pos = player.get_entity().pos.load();
            let delta = player_pos - hook_pos;
            let velocity = delta.multiply(0.1, 0.1, 0.1).add_raw(
                0.0,
                delta.length_squared().sqrt().sqrt() * 0.08,
                0.0,
            );
            let caught_entity = Entity::new(world.clone(), hook_pos, &EntityType::ITEM);
            world
                .spawn_entity(Arc::new(ItemEntity::new_with_velocity(
                    caught_entity,
                    item_stack.clone(),
                    velocity,
                    10,
                )))
                .await;

            ExperienceOrbEntity::spawn(&world, player_pos, rand::random_range(1..=6)).await;

            player
                .trigger_advancement(
                    crate::entity::player::advancement::trigger::AdvancementTrigger::FishedItem {
                        item_id: format!("minecraft:{}", item_stack.item.registry_key),
                    },
                )
                .await;

            return 1;
        }

        if self.in_ground.load(Ordering::Relaxed) {
            return 2;
        }

        0
    }

    #[expect(clippy::too_many_lines)]
    pub async fn process_tick<'a>(&'a self, caller: &'a Arc<dyn EntityBase>, _server: &'a Server) {
        let entity = self.get_entity();
        let world = entity.world.load();

        if self.in_ground.load(Ordering::Relaxed) {
            return;
        }

        let hooked_id = self.hooked_entity_id.load(Ordering::Relaxed);
        if hooked_id != 0 {
            if let Some(hooked) = world.get_entity_by_id(hooked_id) {
                if hooked.get_entity().removed.load(Ordering::Relaxed) {
                    self.hooked_entity_id.store(0, Ordering::Relaxed);
                } else {
                    let mut hooked_pos = hooked.get_entity().pos.load();
                    hooked_pos.y += hooked.get_entity().get_eye_height() * 0.8;
                    entity.set_pos(hooked_pos);
                    return;
                }
            } else {
                self.hooked_entity_id.store(0, Ordering::Relaxed);
            }
        }

        let mut velocity = entity.velocity.load();
        let start_pos = entity.pos.load();

        if entity.touching_water.load(Ordering::Relaxed) {
            velocity.y += 0.02; // Buoyancy

            let bite = self.bite_countdown.load(Ordering::Relaxed);
            if bite > 0 {
                self.bite_countdown.store(bite - 1, Ordering::Relaxed);
                if bite % 5 == 0 {
                    world.spawn_particle(
                        entity.pos.load(),
                        Vector3::new(0.1f32, 0.1f32, 0.1f32),
                        0.0,
                        5,
                        pumpkin_data::particle::Particle::Bubble,
                    );
                }
            } else {
                let wait = self.wait_countdown.load(Ordering::Relaxed);
                if wait > 0 {
                    self.wait_countdown.store(wait - 1, Ordering::Relaxed);
                } else {
                    // Start bite
                    self.bite_countdown.store(40, Ordering::Relaxed);
                    self.wait_countdown
                        .store(rand::random::<i32>().abs() % 600 + 100, Ordering::Relaxed);

                    world.play_sound(
                        Sound::EntityFishingBobberSplash,
                        SoundCategory::Neutral,
                        &entity.pos.load(),
                    );
                }
            }
        } else if !entity.has_no_gravity() {
            velocity.y -= Self::GRAVITY;
        }

        let inertia = if entity.touching_water.load(Ordering::Relaxed) {
            Self::WATER_INERTIA
        } else {
            Self::AIR_INERTIA
        };
        velocity = velocity.multiply(inertia, inertia, inertia);
        entity.velocity.store(velocity);

        let new_pos = start_pos.add(&velocity);

        let search_box = BoundingBox::new(
            Vector3::new(
                start_pos.x.min(new_pos.x),
                start_pos.y.min(new_pos.y),
                start_pos.z.min(new_pos.z),
            ),
            Vector3::new(
                start_pos.x.max(new_pos.x),
                start_pos.y.max(new_pos.y),
                start_pos.z.max(new_pos.z),
            ),
        )
        .expand(0.3, 0.3, 0.3);

        // Basic block collision to stop bobber
        let (block_cols, _) = world
            .get_block_collisions(search_box, caller.as_ref())
            .await;
        if !block_cols.is_empty() {
            self.in_ground.store(true, Ordering::Relaxed);
            entity.velocity.store(Vector3::new(0.0, 0.0, 0.0));
            return;
        }

        entity.set_pos(new_pos);

        let candidates = world.get_entities_at_box(&search_box);
        for cand in candidates {
            if cand.get_entity().entity_id == self.owner_id
                || cand.get_entity().entity_id == entity.entity_id
            {
                continue;
            }

            if is_projectile(cand.get_entity().entity_type) {
                continue;
            }

            let ebb = cand.get_entity().bounding_box.load().expand(0.3, 0.3, 0.3);
            if ebb.intersects(&search_box) {
                self.hooked_entity_id
                    .store(cand.get_entity().entity_id, Ordering::Relaxed);
                entity.send_meta_data(
                    &[Metadata::new(
                        pumpkin_data::tracked_data::fishing_bobber::HOOKED_ENTITY,
                        cand.get_entity().entity_id + 1,
                    )],
                    None,
                );
                return;
            }
        }
    }
}

impl NBTStorage for FishingBobberEntity {}

impl EntityBase for FishingBobberEntity {
    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_nbt_storage(&self) -> &dyn NBTStorage {
        self
    }

    fn on_hit(&self, _hit: ProjectileHit) -> EntityBaseFuture<'_, ()> {
        Box::pin(async move {
            self.has_hit.store(true, Ordering::Relaxed);
        })
    }

    fn tick<'a>(
        &'a self,
        caller: &'a Arc<dyn EntityBase>,
        server: &'a Server,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            self.process_tick(caller, server).await;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{FishingBobberEntity, FishingLootCategory, OpenWaterType};
    use pumpkin_data::data_component_impl::{EnchantmentsImpl, StoredEnchantmentsImpl};
    use pumpkin_data::item::Item;

    #[test]
    fn vanilla_fish_loot_weights_cover_exact_roll_ranges() {
        let counts = (0..100).fold([0; 4], |mut counts, roll| {
            let item = FishingBobberEntity::fish_for_roll(roll);
            let index = if item.id == Item::COD.id {
                0
            } else if item.id == Item::SALMON.id {
                1
            } else if item.id == Item::PUFFERFISH.id {
                2
            } else {
                assert_eq!(item.id, Item::TROPICAL_FISH.id);
                3
            };
            counts[index] += 1;
            counts
        });
        assert_eq!(counts, [60, 25, 13, 2]);
    }

    #[test]
    fn vanilla_fishing_category_weights_apply_luck_quality() {
        let counts = (0..100).fold([0; 3], |mut counts, roll| {
            match FishingBobberEntity::category_for_roll(roll, 0, true) {
                FishingLootCategory::Junk => counts[0] += 1,
                FishingLootCategory::Treasure => counts[1] += 1,
                FishingLootCategory::Fish => counts[2] += 1,
            }
            counts
        });
        assert_eq!(counts, [10, 5, 85]);

        let counts = (0..97).fold([0; 3], |mut counts, roll| {
            match FishingBobberEntity::category_for_roll(roll, 3, true) {
                FishingLootCategory::Junk => counts[0] += 1,
                FishingLootCategory::Treasure => counts[1] += 1,
                FishingLootCategory::Fish => counts[2] += 1,
            }
            counts
        });
        assert_eq!(counts, [4, 11, 82]);
    }

    #[test]
    fn treasure_is_excluded_when_hook_is_not_in_open_water() {
        for roll in 0..95 {
            assert_ne!(
                FishingBobberEntity::category_for_roll(roll, 0, false),
                FishingLootCategory::Treasure
            );
        }
    }

    #[test]
    fn bamboo_is_only_in_the_jungle_junk_pool() {
        for roll in 0..100 {
            assert_ne!(
                FishingBobberEntity::junk_for_roll(roll, false).item.id,
                Item::BAMBOO.id
            );
        }
        assert_eq!(
            (0..110)
                .filter(|roll| {
                    FishingBobberEntity::junk_for_roll(*roll, true).item.id == Item::BAMBOO.id
                })
                .count(),
            10
        );
    }

    #[test]
    fn vanilla_open_water_layers_require_water_below_air() {
        use OpenWaterType::{AboveWater, InsideWater, Invalid};
        assert!(FishingBobberEntity::is_open_water_sequence(&[
            InsideWater,
            InsideWater,
            AboveWater,
            AboveWater,
        ]));
        assert!(!FishingBobberEntity::is_open_water_sequence(&[
            AboveWater, AboveWater, AboveWater, AboveWater,
        ]));
        assert!(!FishingBobberEntity::is_open_water_sequence(&[
            InsideWater,
            AboveWater,
            InsideWater,
            AboveWater,
        ]));
        assert!(!FishingBobberEntity::is_open_water_sequence(&[
            InsideWater,
            Invalid,
            AboveWater,
            AboveWater,
        ]));
    }

    #[test]
    fn vanilla_fishing_treasure_enchants_bows_rods_and_books_at_level_thirty() {
        let bow = FishingBobberEntity::treasure_for_roll(2);
        assert_eq!(bow.item.id, Item::BOW.id);
        assert!(
            bow.get_data_component::<EnchantmentsImpl>()
                .is_some_and(|component| !component.enchantment.is_empty())
        );

        let rod = FishingBobberEntity::treasure_for_roll(3);
        assert_eq!(rod.item.id, Item::FISHING_ROD.id);
        assert!(
            rod.get_data_component::<EnchantmentsImpl>()
                .is_some_and(|component| !component.enchantment.is_empty())
        );

        let book = FishingBobberEntity::treasure_for_roll(4);
        assert_eq!(book.item.id, Item::ENCHANTED_BOOK.id);
        assert!(
            book.get_data_component::<StoredEnchantmentsImpl>()
                .is_some_and(|component| !component.enchantment.is_empty())
        );
    }
}
