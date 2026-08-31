use std::sync::{
    Arc, Weak,
    atomic::{AtomicU8, Ordering},
};

use pumpkin_data::{
    entity::EntityType,
    item::Item,
    sound::SoundCategory,
    tag::{self, Taggable},
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::java::client::play::Metadata;

use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, NBTStorage, NbtFuture,
    ageable::AgeableMob,
    ai::goal::{
        breed::BreedGoal, eat_grass::EatGrassGoal, escape_danger::EscapeDangerGoal,
        follow_parent::FollowParentGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, swim::SwimGoal, tempt::TemptGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;

const TEMPT_ITEMS: &[&Item] = &[&Item::WHEAT];

fn mixed_offspring_color(first: u8, second: u8, choose_second: bool) -> u8 {
    let pair = if first <= second {
        (first, second)
    } else {
        (second, first)
    };
    match pair {
        (0, 11) => 3,   // white + blue = light blue
        (0, 13) => 5,   // white + green = lime
        (0, 14) => 6,   // white + red = pink
        (0, 15) => 7,   // white + black = gray
        (0, 7) => 8,    // white + gray = light gray
        (11, 13) => 9,  // blue + green = cyan
        (11, 14) => 10, // blue + red = purple
        (6, 10) => 2,   // pink + purple = magenta
        (4, 14) => 1,   // yellow + red = orange
        _ if choose_second => second,
        _ => first,
    }
}

fn dye_color_from_item(item: &Item) -> Option<u8> {
    if !item.has_tag(&tag::Item::C_DYES) {
        return None;
    }
    let name = item.registry_key.strip_suffix("_dye")?;
    Some(match name {
        "white" => 0,
        "orange" => 1,
        "magenta" => 2,
        "light_blue" => 3,
        "yellow" => 4,
        "lime" => 5,
        "pink" => 6,
        "gray" => 7,
        "light_gray" => 8,
        "cyan" => 9,
        "purple" => 10,
        "blue" => 11,
        "brown" => 12,
        "green" => 13,
        "red" => 14,
        "black" => 15,
        _ => return None,
    })
}

const fn event_dye_color(color: u8) -> crate::plugin::api::events::entity::entity_dye::DyeColor {
    use crate::plugin::api::events::entity::entity_dye::DyeColor;
    match color {
        0 => DyeColor::White,
        1 => DyeColor::Orange,
        2 => DyeColor::Magenta,
        3 => DyeColor::LightBlue,
        4 => DyeColor::Yellow,
        5 => DyeColor::Lime,
        6 => DyeColor::Pink,
        7 => DyeColor::Gray,
        8 => DyeColor::LightGray,
        9 => DyeColor::Cyan,
        10 => DyeColor::Purple,
        11 => DyeColor::Blue,
        12 => DyeColor::Brown,
        13 => DyeColor::Green,
        14 => DyeColor::Red,
        _ => DyeColor::Black,
    }
}

pub struct SheepEntity {
    pub mob_entity: MobEntity,
    color_and_sheared: AtomicU8,
    pub ageable_data: crate::entity::ageable::AgeableData,
}

impl SheepEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let sheep = Self {
            mob_entity,
            color_and_sheared: AtomicU8::new(0),
            ageable_data: crate::entity::ageable::AgeableData::default(),
        };
        let mob_arc = Arc::new(sheep);
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

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, EscapeDangerGoal::new(1.25));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.1, TEMPT_ITEMS)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.1)));
            goal_selector.add_goal(5, Box::new(EatGrassGoal::default()));
            goal_selector.add_goal(6, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    fn get_packed_byte(&self) -> u8 {
        self.color_and_sheared.load(Ordering::Relaxed)
    }

    pub fn get_color(&self) -> u8 {
        self.get_packed_byte() & 0x0F
    }

    pub fn is_sheared(&self) -> bool {
        (self.get_packed_byte() & 0x10) != 0
    }

    fn set_packed_and_sync(&self, byte: u8) {
        self.color_and_sheared.store(byte, Ordering::Relaxed);
        self.mob_entity.living_entity.entity.send_meta_data(
            &[Metadata::new(
                pumpkin_data::tracked_data::sheep::WOOL_ID,
                byte as i8,
            )],
            None,
        );
    }

    pub fn set_color(&self, color: u8) {
        let byte = (self.get_packed_byte() & 0xF0) | (color & 0x0F);
        self.set_packed_and_sync(byte);
    }

    pub fn set_sheared(&self, sheared: bool) {
        let byte = if sheared {
            self.get_packed_byte() | 0x10
        } else {
            self.get_packed_byte() & !0x10
        };
        self.set_packed_and_sync(byte);
    }
}

impl crate::entity::ageable::AgeableMob for SheepEntity {
    fn get_ageable_data(&self) -> &crate::entity::ageable::AgeableData {
        &self.ageable_data
    }
}

impl NBTStorage for SheepEntity {
    fn write_nbt<'a>(&'a self, nbt: &'a mut NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async {
            self.mob_entity.write_nbt(nbt).await;
            self.write_ageable_nbt(nbt);
            self.write_animal_nbt(nbt);
            nbt.put_bool("Sheared", self.is_sheared());
            nbt.put_byte("Color", self.get_color() as i8);
        })
    }

    fn read_nbt_non_mut<'a>(&'a self, nbt: &'a NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            self.read_ageable_nbt(nbt);
            self.read_animal_nbt(nbt);
            let current = self.color_and_sheared.load(Ordering::Relaxed);
            let sheared = nbt
                .get_bool("Sheared")
                .or_else(|| nbt.get_byte("Sheared").map(|b| b == 1));
            let color = nbt.get_byte("Color").map(|color| color as u8);
            if sheared.is_some() || color.is_some() {
                let sheared = sheared.unwrap_or(current & 0x10 != 0);
                let color = color.unwrap_or(current) & 0x0F;
                let byte = color | if sheared { 0x10 } else { 0 };
                self.color_and_sheared.store(byte, Ordering::Relaxed);
            }
        })
    }
}

impl super::animal::Animal for SheepEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        use pumpkin_data::tag::Taggable;
        item_stack
            .item
            .has_tag(&pumpkin_data::tag::Item::MINECRAFT_SHEEP_FOOD)
            || TEMPT_ITEMS.iter().any(|i| i.id == item_stack.item.id)
    }
}

impl Mob for SheepEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick<'a>(&'a self, _caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move { self.ageable_ai_step() })
    }

    fn on_eating_grass(&self) -> EntityBaseFuture<'_, ()> {
        Box::pin(async {
            self.set_sheared(false);
        })
    }

    fn get_sheep(&self) -> Option<&SheepEntity> {
        Some(self)
    }

    fn configure_bred_child<'a>(
        &'a self,
        mate: &'a dyn EntityBase,
        child: &'a Arc<dyn EntityBase>,
    ) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            let Some(child_sheep) = child.get_mob().and_then(Mob::get_sheep) else {
                return;
            };
            let Some(mate_sheep) = mate.get_mob().and_then(Mob::get_sheep) else {
                return;
            };
            let color = mixed_offspring_color(
                self.get_color(),
                mate_sheep.get_color(),
                rand::random::<bool>(),
            );
            child_sheep
                .color_and_sheared
                .store(color & 0x0f, Ordering::Relaxed);
        })
    }

    fn mob_interact<'a>(
        &'a self,
        player: &'a Arc<Player>,
        item_stack: &'a mut ItemStack,
    ) -> EntityBaseFuture<'a, bool> {
        Box::pin(async move {
            if let Some(color) = dye_color_from_item(item_stack.get_item())
                && color != self.get_color()
            {
                let entity = &self.mob_entity.living_entity.entity;
                let mut event = crate::plugin::api::events::entity::entity_dye::EntityDyeEvent::new(
                    entity.entity_id,
                    event_dye_color(color),
                    Some(player.clone()),
                );
                let world = entity.world.load();
                if let Some(server) = world.server.upgrade() {
                    server.plugin_manager.fire(&server, &mut event).await;
                }
                if event.cancelled {
                    return false;
                }

                self.set_color(color);
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                world.play_sound(
                    Sound::ItemDyeUse,
                    SoundCategory::Players,
                    &entity.pos.load(),
                );
                return true;
            }

            use super::animal::Animal;
            self.animal_interact(player, item_stack, Sound::EntitySheepAmbient)
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_dye_item_mapping() {
        assert_eq!(dye_color_from_item(&Item::WHITE_DYE), Some(0));
        assert_eq!(dye_color_from_item(&Item::LIGHT_BLUE_DYE), Some(3));
        assert_eq!(dye_color_from_item(&Item::LIGHT_GRAY_DYE), Some(8));
        assert_eq!(dye_color_from_item(&Item::RED_DYE), Some(14));
        assert_eq!(dye_color_from_item(&Item::BLACK_DYE), Some(15));
        assert_eq!(dye_color_from_item(&Item::STICK), None);
    }

    #[test]
    fn sheep_color_and_sheared_bit_packing() {
        let color = 14u8; // Red
        let sheared = false;
        let mut byte = (color & 0x0F) | if sheared { 0x10 } else { 0 };
        assert_eq!(byte & 0x0F, 14);
        assert_eq!((byte & 0x10) != 0, false);

        // Shear the sheep
        byte |= 0x10;
        assert_eq!(byte & 0x0F, 14);
        assert_eq!((byte & 0x10) != 0, true);

        // Eat grass / regrow wool
        byte &= !0x10;
        assert_eq!((byte & 0x10) != 0, false);
    }

    #[test]
    fn vanilla_breeding_color_recipes_and_fallback() {
        assert_eq!(mixed_offspring_color(0, 15, false), 7);
        assert_eq!(mixed_offspring_color(14, 4, false), 1);
        assert_eq!(mixed_offspring_color(10, 6, true), 2);
        assert_eq!(mixed_offspring_color(12, 3, false), 12);
        assert_eq!(mixed_offspring_color(12, 3, true), 3);
    }
}
