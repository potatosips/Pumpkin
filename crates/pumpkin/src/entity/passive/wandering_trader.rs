use crossbeam::atomic::AtomicCell;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::random::RandomImpl;
use std::borrow::Cow;
use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use pumpkin_data::entity::EntityType;
use pumpkin_data::{
    attributes::Attributes,
    data_component::DataComponent,
    data_component_impl::{DataComponentImpl, EquipmentSlot, PotionContentsImpl},
    effect::StatusEffect,
    item::Item,
    item_stack::ItemStack,
    potion::Potion,
    sound::{Sound, SoundCategory},
    statistic::{CustomStatistic, StatisticCategory},
};
use pumpkin_inventory::{
    merchant::merchant_screen_handler::MerchantScreenHandler,
    screen_handler::{BoxFuture, InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler},
};
use pumpkin_nbt::tag::NbtTag;
use pumpkin_protocol::{
    bedrock::client::CUpdateTrade,
    codec::{item_stack_seralizer::ItemStackSerializer, var_int::VarInt, var_long::VarLong},
    java::client::play::{CMerchantOffers, MerchantOffer},
};
use pumpkin_util::text::TextComponent;
use pumpkin_world::inventory::SimpleInventory;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase, EntityBaseFuture, NBTStorage, NbtFuture,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        avoid_entity::AvoidEntityGoal, escape_danger::EscapeDangerGoal,
        look_at_entity::LookAtEntityGoal, look_at_trading_player::LookAtTradingPlayerGoal,
        move_towards_restriction::MoveTowardsRestrictionGoal, swim::SwimGoal,
        trade_with_player::TradeWithPlayerGoal, wander_around::WanderAroundGoal,
        wander_to_position::WanderToPositionGoal,
    },
    experience_orb::ExperienceOrbEntity,
    mob::{Mob, MobEntity},
    player::Player,
};

pub struct WanderingTraderEntity {
    pub mob_entity: MobEntity,
    ageable_data: AgeableData,
    despawn_delay: AtomicI32,
    drinking_time: AtomicI32,
    drinking_item: AtomicI32,
    is_trading: AtomicBool,
    wander_target: AtomicCell<Option<BlockPos>>,
    offers: Mutex<Vec<MerchantOffer>>,
    merchant_inventory: Arc<SimpleInventory>,
    trading_player: std::sync::Mutex<Option<(Uuid, u8)>>,
    trade_sound_cooldown: AtomicI32,
    self_weak: std::sync::Mutex<Option<Weak<Self>>>,
}

#[derive(Clone, Copy)]
struct WanderingTrade {
    item: &'static Item,
    emerald_cost: u8,
    count: u8,
    max_uses: i32,
}

macro_rules! trade {
    ($item:ident, $cost:literal, $count:literal, $uses:literal) => {
        WanderingTrade {
            item: &Item::$item,
            emerald_cost: $cost,
            count: $count,
            max_uses: $uses,
        }
    };
}

// VillagerTrades.WANDERING_TRADER_TRADES in Java 1.21.4: five unique
// selections from level 1 and one selection from level 2.
const COMMON_TRADES: [WanderingTrade; 68] = [
    trade!(SEA_PICKLE, 2, 1, 5),
    trade!(SLIME_BALL, 4, 1, 5),
    trade!(GLOWSTONE, 2, 1, 5),
    trade!(NAUTILUS_SHELL, 5, 1, 5),
    trade!(FERN, 1, 1, 12),
    trade!(SUGAR_CANE, 1, 1, 8),
    trade!(PUMPKIN, 1, 1, 4),
    trade!(KELP, 3, 1, 12),
    trade!(CACTUS, 3, 1, 8),
    trade!(DANDELION, 1, 1, 12),
    trade!(POPPY, 1, 1, 12),
    trade!(BLUE_ORCHID, 1, 1, 8),
    trade!(ALLIUM, 1, 1, 12),
    trade!(AZURE_BLUET, 1, 1, 12),
    trade!(RED_TULIP, 1, 1, 12),
    trade!(ORANGE_TULIP, 1, 1, 12),
    trade!(WHITE_TULIP, 1, 1, 12),
    trade!(PINK_TULIP, 1, 1, 12),
    trade!(OXEYE_DAISY, 1, 1, 12),
    trade!(CORNFLOWER, 1, 1, 12),
    trade!(LILY_OF_THE_VALLEY, 1, 1, 7),
    trade!(OPEN_EYEBLOSSOM, 1, 1, 7),
    trade!(WHEAT_SEEDS, 1, 1, 12),
    trade!(BEETROOT_SEEDS, 1, 1, 12),
    trade!(PUMPKIN_SEEDS, 1, 1, 12),
    trade!(MELON_SEEDS, 1, 1, 12),
    trade!(ACACIA_SAPLING, 5, 1, 8),
    trade!(BIRCH_SAPLING, 5, 1, 8),
    trade!(DARK_OAK_SAPLING, 5, 1, 8),
    trade!(JUNGLE_SAPLING, 5, 1, 8),
    trade!(OAK_SAPLING, 5, 1, 8),
    trade!(SPRUCE_SAPLING, 5, 1, 8),
    trade!(CHERRY_SAPLING, 5, 1, 8),
    trade!(PALE_OAK_SAPLING, 5, 1, 8),
    trade!(MANGROVE_PROPAGULE, 5, 1, 8),
    trade!(RED_DYE, 1, 3, 12),
    trade!(WHITE_DYE, 1, 3, 12),
    trade!(BLUE_DYE, 1, 3, 12),
    trade!(PINK_DYE, 1, 3, 12),
    trade!(BLACK_DYE, 1, 3, 12),
    trade!(GREEN_DYE, 1, 3, 12),
    trade!(LIGHT_GRAY_DYE, 1, 3, 12),
    trade!(MAGENTA_DYE, 1, 3, 12),
    trade!(YELLOW_DYE, 1, 3, 12),
    trade!(GRAY_DYE, 1, 3, 12),
    trade!(PURPLE_DYE, 1, 3, 12),
    trade!(LIGHT_BLUE_DYE, 1, 3, 12),
    trade!(LIME_DYE, 1, 3, 12),
    trade!(ORANGE_DYE, 1, 3, 12),
    trade!(BROWN_DYE, 1, 3, 12),
    trade!(CYAN_DYE, 1, 3, 12),
    trade!(BRAIN_CORAL_BLOCK, 3, 1, 8),
    trade!(BUBBLE_CORAL_BLOCK, 3, 1, 8),
    trade!(FIRE_CORAL_BLOCK, 3, 1, 8),
    trade!(HORN_CORAL_BLOCK, 3, 1, 8),
    trade!(TUBE_CORAL_BLOCK, 3, 1, 8),
    trade!(VINE, 1, 1, 12),
    trade!(PALE_HANGING_MOSS, 1, 1, 12),
    trade!(BROWN_MUSHROOM, 1, 1, 12),
    trade!(RED_MUSHROOM, 1, 1, 12),
    trade!(LILY_PAD, 1, 2, 5),
    trade!(SMALL_DRIPLEAF, 1, 2, 5),
    trade!(SAND, 1, 8, 8),
    trade!(RED_SAND, 1, 4, 6),
    trade!(POINTED_DRIPSTONE, 1, 2, 5),
    trade!(ROOTED_DIRT, 1, 2, 5),
    trade!(MOSS_BLOCK, 1, 2, 5),
    trade!(PALE_MOSS_BLOCK, 1, 2, 5),
];

const SPECIAL_TRADES: [WanderingTrade; 6] = [
    trade!(TROPICAL_FISH_BUCKET, 5, 1, 4),
    trade!(PUFFERFISH_BUCKET, 5, 1, 4),
    trade!(PACKED_ICE, 3, 1, 6),
    trade!(BLUE_ICE, 6, 1, 6),
    trade!(GUNPOWDER, 1, 1, 8),
    trade!(PODZOL, 3, 3, 6),
];

const DRINKING_NONE: i32 = 0;
const DRINKING_INVISIBILITY: i32 = 1;
const DRINKING_MILK: i32 = 2;

fn write_wander_target(target: BlockPos) -> NbtTag {
    NbtTag::IntArray(vec![target.0.x, target.0.y, target.0.z])
}

fn read_wander_target(nbt: &pumpkin_nbt::compound::NbtCompound) -> Option<BlockPos> {
    let [x, y, z] = nbt.get_int_array("wander_target")? else {
        return None;
    };
    Some(BlockPos::new(*x, *y, *z))
}

const fn loaded_trader_age(age: i32) -> i32 {
    if age < 0 { 0 } else { age }
}

fn use_item_finish_pitch(roll: f32) -> f32 {
    0.9 + roll.clamp(0.0, 1.0) * 0.2
}

const fn trade_interaction_eligible(
    is_spawn_egg: bool,
    is_alive: bool,
    is_trading: bool,
    is_baby: bool,
) -> bool {
    !is_spawn_egg && is_alive && !is_trading && !is_baby
}

const fn required_drink(is_night: bool, is_invisible: bool) -> i32 {
    if is_night && !is_invisible {
        DRINKING_INVISIBILITY
    } else if !is_night && is_invisible {
        DRINKING_MILK
    } else {
        DRINKING_NONE
    }
}

impl WanderingTraderEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let trader = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            // Vanilla's constructor leaves this at zero; the natural spawner assigns 48,000.
            despawn_delay: AtomicI32::new(0),
            drinking_time: AtomicI32::new(0),
            drinking_item: AtomicI32::new(DRINKING_NONE),
            is_trading: AtomicBool::new(false),
            wander_target: AtomicCell::new(None),
            offers: Mutex::new(Vec::new()),
            merchant_inventory: Arc::new(SimpleInventory::new(3)),
            trading_player: std::sync::Mutex::new(None),
            trade_sound_cooldown: AtomicI32::new(0),
            self_weak: std::sync::Mutex::new(None),
        };
        let mob_arc = Arc::new(trader);
        *mob_arc
            .self_weak
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::downgrade(&mob_arc));
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
            goal_selector.add_goal(1, Box::new(TradeWithPlayerGoal::new(0.5)));
            for (kind, distance) in [
                (&EntityType::ZOMBIE, 8.0),
                (&EntityType::HUSK, 8.0),
                (&EntityType::DROWNED, 8.0),
                (&EntityType::ZOMBIE_VILLAGER, 8.0),
                (&EntityType::ZOMBIFIED_PIGLIN, 8.0),
                (&EntityType::EVOKER, 12.0),
                (&EntityType::VINDICATOR, 8.0),
                (&EntityType::VEX, 8.0),
                (&EntityType::PILLAGER, 15.0),
                (&EntityType::ILLUSIONER, 12.0),
                (&EntityType::ZOGLIN, 10.0),
            ] {
                goal_selector.add_goal(1, Box::new(AvoidEntityGoal::new(kind, distance, 0.5, 0.5)));
            }
            goal_selector.add_goal(1, EscapeDangerGoal::new(0.5));
            goal_selector.add_goal(1, Box::new(LookAtTradingPlayerGoal::new()));
            goal_selector.add_goal(2, Box::new(WanderToPositionGoal::new(2.0, 0.35)));
            goal_selector.add_goal(4, Box::new(MoveTowardsRestrictionGoal::new(0.35)));
            goal_selector.add_goal(8, Box::new(WanderAroundGoal::new(0.35)));
            goal_selector.add_goal(
                9,
                Box::new(LookAtEntityGoal::new(
                    mob_weak.clone(),
                    &EntityType::PLAYER,
                    3.0,
                    1.0,
                    false,
                )),
            );
            goal_selector.add_goal(10, LookAtEntityGoal::for_any_mob(mob_weak, 8.0));
        };

        mob_arc
    }

    pub fn despawn_delay(&self) -> i32 {
        self.despawn_delay.load(Ordering::Relaxed)
    }

    pub fn set_despawn_delay(&self, delay: i32) {
        self.despawn_delay.store(delay, Ordering::Relaxed);
    }

    pub fn wander_target(&self) -> Option<BlockPos> {
        self.wander_target.load()
    }

    pub fn set_wander_target(&self, target: Option<BlockPos>) {
        self.wander_target.store(target);
    }

    fn merchant_offer(trade: WanderingTrade) -> MerchantOffer {
        MerchantOffer {
            base_cost_a: ItemStackSerializer(Cow::Owned(ItemStack::new(
                trade.emerald_cost,
                &Item::EMERALD,
            ))),
            output: ItemStackSerializer(Cow::Owned(ItemStack::new(trade.count, trade.item))),
            cost_b: None,
            reward_exp: true,
            uses: 0,
            max_uses: trade.max_uses,
            xp: 1,
            special_price: 0,
            price_multiplier: 0.05,
            demand: 0,
        }
    }

    async fn generate_trades(&self) {
        let mut offers = self.offers.lock().await;
        if !offers.is_empty() {
            return;
        }
        let mut common = COMMON_TRADES.to_vec();
        let mut random = self.get_entity_random();
        for _ in 0..5 {
            let index = random.next_bounded_i32(common.len() as i32) as usize;
            offers.push(Self::merchant_offer(common.remove(index)));
        }
        let special = SPECIAL_TRADES[random.next_bounded_i32(SPECIAL_TRADES.len() as i32) as usize];
        offers.push(Self::merchant_offer(special));
    }

    fn can_continue_trading(
        &self,
        inventory_player: &dyn InventoryPlayer,
        player_uuid: Uuid,
        sync_id: u8,
    ) -> bool {
        let Some(player) = inventory_player.as_any().downcast_ref::<Player>() else {
            return false;
        };
        let range = player
            .living_entity
            .get_attribute_value(&Attributes::ENTITY_INTERACTION_RANGE)
            + 4.0;
        self.get_entity().is_alive()
            && self.mob_entity.living_entity.health.load() > 0.0
            && self
                .trading_player
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .is_some_and(|(uuid, id)| *uuid == player_uuid && *id == sync_id)
            && self
                .get_entity()
                .bounding_box
                .load()
                .squared_magnitude(player.eye_position())
                < range * range
    }

    async fn complete_trade(&self, offer_index: usize, player_uuid: Uuid) {
        let reward_exp = {
            let mut offers = self.offers.lock().await;
            let Some(offer) = offers.get_mut(offer_index) else {
                return;
            };
            offer.uses += 1;
            offer.reward_exp
        };
        self.get_entity()
            .play_sound(Sound::EntityWanderingTraderTrade);
        self.trade_sound_cooldown.store(20, Ordering::Relaxed);
        let world = self.get_entity().world.load();
        if reward_exp {
            let reward = self.get_entity_random().next_inbetween_i32(3, 6) as u32;
            ExperienceOrbEntity::spawn(
                &world,
                self.get_entity().pos.load().add_raw(0.0, 0.5, 0.0),
                reward,
            )
            .await;
        }
        if let Some(player) = world.get_player_by_uuid(player_uuid) {
            player
                .trigger_advancement(
                    crate::entity::player::advancement::trigger::AdvancementTrigger::TradedWithVillager,
                )
                .await;
        }
    }

    async fn open_trading_screen(&self, player: &Arc<Player>) {
        if let Some(sync_id) = player.open_handled_screen(self, None).await {
            let offers = self.offers.lock().await.clone();
            let java = CMerchantOffers::new(
                VarInt(i32::from(sync_id)),
                offers.clone(),
                VarInt(1),
                VarInt(0),
                false,
                false,
            );
            let bedrock = CUpdateTrade {
                container_id: sync_id,
                r#type: 15,
                size: VarInt(0),
                trader_tier: VarInt(0),
                entity_unique_id: VarLong(i64::from(self.get_entity().entity_id)),
                last_trading_player: VarLong(i64::from(player.entity_id())),
                display_name: ScreenHandlerFactory::get_display_name(self).to_pretty_console(),
                use_new_trade_screen: true,
                using_economy_trade: true,
                data: super::villager::VillagerEntity::bedrock_trade_data(&offers, 1),
            };
            player
                .client
                .enqueue_packet_editioned(&java, &bedrock)
                .await;
        }
    }

    fn invisibility_potion() -> ItemStack {
        let mut stack = ItemStack::new(1, &Item::POTION);
        stack.patch.push((
            DataComponent::PotionContents,
            Some(
                PotionContentsImpl {
                    potion_id: Some(Potion::INVISIBILITY.id as i32),
                    custom_color: None,
                    custom_effects: Vec::new(),
                    custom_name: None,
                }
                .to_dyn(),
            ),
        ));
        stack
    }

    async fn start_drinking(&self, kind: i32) {
        let stack = if kind == DRINKING_MILK {
            ItemStack::new(1, &Item::MILK_BUCKET)
        } else {
            Self::invisibility_potion()
        };
        self.drinking_item.store(kind, Ordering::Relaxed);
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
    }

    async fn finish_drinking(&self) {
        let kind = self.drinking_item.swap(DRINKING_NONE, Ordering::Relaxed);
        self.drinking_time.store(0, Ordering::Relaxed);
        self.mob_entity.living_entity.clear_active_hand().await;
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

        let (consume_sound, finish_sound) = if kind == DRINKING_MILK {
            self.mob_entity
                .living_entity
                .reset_effects_and_attributes()
                .await;
            (
                Sound::EntityWanderingTraderDrinkMilk,
                Sound::EntityWanderingTraderReappeared,
            )
        } else if kind == DRINKING_INVISIBILITY {
            let stack = Self::invisibility_potion();
            crate::item::potion::PotionContents::apply_effects_to(
                &self.mob_entity.living_entity,
                crate::item::potion::PotionContents::read_potion_effects(&stack),
                1.0,
                crate::item::potion::PotionApplicationSource::Normal,
            )
            .await;
            (
                Sound::EntityWanderingTraderDrinkPotion,
                Sound::EntityWanderingTraderDisappeared,
            )
        } else {
            return;
        };
        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();
        world.play_sound_fine(consume_sound, SoundCategory::Neutral, &pos, 1.0, 1.0);
        world.play_sound_fine(
            finish_sound,
            SoundCategory::Neutral,
            &pos,
            1.0,
            use_item_finish_pitch(self.get_entity_random().next_f32()),
        );
    }
}

impl AgeableMob for WanderingTraderEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl NBTStorage for WanderingTraderEntity {
    fn write_nbt<'a>(
        &'a self,
        nbt: &'a mut pumpkin_nbt::compound::NbtCompound,
    ) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.write_nbt(nbt).await;
            self.write_ageable_nbt(nbt);
            nbt.put_int("DespawnDelay", self.despawn_delay());
            if let Some(target) = self.wander_target() {
                nbt.put("wander_target", write_wander_target(target));
            }
            let offers = self.offers.lock().await;
            if !offers.is_empty() {
                let recipes = offers
                    .iter()
                    .map(|offer| {
                        let mut recipe = pumpkin_nbt::compound::NbtCompound::new();
                        let mut buy = pumpkin_nbt::compound::NbtCompound::new();
                        offer.base_cost_a.0.write_item_stack(&mut buy);
                        recipe.put_compound("buy", buy);
                        if let Some(cost_b) = &offer.cost_b
                            && !cost_b.0.is_empty()
                        {
                            let mut buy_b = pumpkin_nbt::compound::NbtCompound::new();
                            cost_b.0.write_item_stack(&mut buy_b);
                            recipe.put_compound("buyB", buy_b);
                        }
                        let mut sell = pumpkin_nbt::compound::NbtCompound::new();
                        offer.output.0.write_item_stack(&mut sell);
                        recipe.put_compound("sell", sell);
                        recipe.put_int("uses", offer.uses);
                        recipe.put_int("maxUses", offer.max_uses);
                        recipe.put_bool("rewardExp", offer.reward_exp);
                        recipe.put_int("xp", offer.xp);
                        recipe.put_float("priceMultiplier", offer.price_multiplier);
                        recipe.put_int("specialPrice", offer.special_price);
                        recipe.put_int("demand", offer.demand);
                        NbtTag::Compound(recipe)
                    })
                    .collect();
                let mut offers_nbt = pumpkin_nbt::compound::NbtCompound::new();
                offers_nbt.put("Recipes", NbtTag::List(recipes));
                nbt.put_compound("Offers", offers_nbt);
            }
        })
    }

    fn read_nbt_non_mut<'a>(
        &'a self,
        nbt: &'a pumpkin_nbt::compound::NbtCompound,
    ) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.mob_entity.read_nbt_non_mut(nbt).await;
            self.read_ageable_nbt(nbt);
            // WanderingTrader.readAdditionalSaveData explicitly prevents a
            // malformed/edited negative Age from creating a baby trader.
            self.set_age(loaded_trader_age(self.get_age()));
            if let Some(delay) = nbt.get_int("DespawnDelay") {
                self.set_despawn_delay(delay);
            }
            self.set_wander_target(read_wander_target(nbt));
            let mut offers = self.offers.lock().await;
            offers.clear();
            if let Some(recipes) = nbt
                .get_compound("Offers")
                .and_then(|offers| offers.get_list("Recipes"))
            {
                for tag in recipes {
                    let Some(recipe) = tag.extract_compound() else {
                        continue;
                    };
                    let buy = recipe
                        .get_compound("buy")
                        .and_then(ItemStack::read_item_stack);
                    let buy_b = recipe
                        .get_compound("buyB")
                        .and_then(ItemStack::read_item_stack);
                    let sell = recipe
                        .get_compound("sell")
                        .and_then(ItemStack::read_item_stack);
                    if let (Some(buy), Some(sell)) = (buy, sell)
                        && !buy.is_empty()
                        && !sell.is_empty()
                        && buy_b.as_ref().is_none_or(|stack| !stack.is_empty())
                    {
                        offers.push(MerchantOffer {
                            base_cost_a: buy.into(),
                            output: sell.into(),
                            cost_b: buy_b.map(Into::into),
                            reward_exp: recipe.get_bool("rewardExp").unwrap_or(true),
                            uses: recipe.get_int("uses").unwrap_or(0),
                            max_uses: recipe.get_int("maxUses").unwrap_or(12),
                            xp: recipe.get_int("xp").unwrap_or(1),
                            special_price: recipe.get_int("specialPrice").unwrap_or(0),
                            price_multiplier: recipe.get_float("priceMultiplier").unwrap_or(0.05),
                            demand: recipe.get_int("demand").unwrap_or(0),
                        });
                    }
                }
            }
        })
    }
}

impl Mob for WanderingTraderEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_wandering_trader(&self) -> Option<&WanderingTraderEntity> {
        Some(self)
    }

    fn get_trading_player(&self) -> Option<Arc<Player>> {
        let trading_player = *self
            .trading_player
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (player_uuid, _) = trading_player?;
        self.get_entity()
            .world
            .load()
            .get_player_by_uuid(player_uuid)
    }

    fn stop_trading(&self) {
        self.is_trading.store(false, Ordering::Relaxed);
        *self
            .trading_player
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }

    fn mob_tick<'a>(&'a self, caller: &'a Arc<dyn EntityBase>) -> EntityBaseFuture<'a, ()> {
        Box::pin(async move {
            self.ageable_ai_step();
            self.trade_sound_cooldown
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    (value > 0).then_some(value - 1)
                })
                .ok();
            let drinking = self.drinking_time.load(Ordering::Relaxed);
            if drinking > 0 {
                if self.drinking_time.fetch_sub(1, Ordering::Relaxed) == 1 {
                    self.finish_drinking().await;
                }
            } else {
                let living = &self.mob_entity.living_entity;
                let is_night = self
                    .get_entity()
                    .world
                    .load()
                    .level_time
                    .lock()
                    .await
                    .is_night();
                let drink = required_drink(
                    is_night,
                    living.has_effect(&StatusEffect::INVISIBILITY).await,
                );
                if drink != DRINKING_NONE {
                    self.start_drinking(drink).await;
                }
            }

            let delay = self.despawn_delay();
            if delay > 0 && !self.is_trading.load(Ordering::Relaxed) {
                let remaining = self.despawn_delay.fetch_sub(1, Ordering::Relaxed) - 1;
                if remaining == 0 {
                    self.get_entity()
                        .world
                        .load()
                        .remove_entity(caller.as_ref())
                        .await;
                }
            }
        })
    }

    fn mob_interact<'a>(
        &'a self,
        player: &'a Arc<Player>,
        stack: &'a mut ItemStack,
    ) -> EntityBaseFuture<'a, bool> {
        Box::pin(async move {
            if trade_interaction_eligible(
                stack.item == &Item::WANDERING_TRADER_SPAWN_EGG,
                self.get_entity().is_alive(),
                self.is_trading.load(Ordering::Relaxed),
                self.is_baby(),
            ) {
                player
                    .increment_stat(
                        StatisticCategory::Custom,
                        CustomStatistic::TalkedToVillager as i32,
                        1,
                    )
                    .await;
                self.generate_trades().await;
                self.open_trading_screen(player).await;
                return true;
            }
            self.mob_entity.mob_interact(player, stack).await
        })
    }
}

impl ScreenHandlerFactory for WanderingTraderEntity {
    fn create_screen_handler<'a>(
        &'a self,
        sync_id: u8,
        player_inventory: &'a Arc<pumpkin_inventory::player::player_inventory::PlayerInventory>,
        player: &'a dyn InventoryPlayer,
    ) -> BoxFuture<'a, Option<SharedScreenHandler>> {
        Box::pin(async move {
            let self_weak = self
                .self_weak
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()?;
            let player_uuid = player
                .as_any()
                .downcast_ref::<Player>()
                .map_or_else(Uuid::nil, |player| player.get_entity().entity_uuid);
            let mut handler = MerchantScreenHandler::new(
                sync_id,
                player_inventory,
                self.merchant_inventory.clone(),
                self.offers.lock().await.clone(),
            )
            .await;
            self.is_trading.store(true, Ordering::Relaxed);
            *self
                .trading_player
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((player_uuid, sync_id));
            let validity_weak = self_weak.clone();
            handler.validity_check = Some(Box::new(move |player| {
                validity_weak
                    .upgrade()
                    .is_some_and(|trader| trader.can_continue_trading(player, player_uuid, sync_id))
            }));
            let update_weak = self_weak.clone();
            handler.on_trade_updated = Some(Box::new(move |has_result| {
                if let Some(trader) = update_weak.upgrade()
                    && trader
                        .trade_sound_cooldown
                        .compare_exchange(0, 20, Ordering::Relaxed, Ordering::Relaxed)
                        .is_ok()
                {
                    trader.get_entity().play_sound(if has_result {
                        Sound::EntityWanderingTraderYes
                    } else {
                        Sound::EntityWanderingTraderNo
                    });
                }
            }));
            let close_weak = self_weak.clone();
            handler.on_close = Some(Box::new(move || {
                let close_weak = close_weak.clone();
                Box::pin(async move {
                    if let Some(trader) = close_weak.upgrade() {
                        trader.is_trading.store(false, Ordering::Relaxed);
                        *trader
                            .trading_player
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
                    }
                })
            }));
            handler.on_trade = Some(Box::new(move |offer_index| {
                let self_weak = self_weak.clone();
                Box::pin(async move {
                    if let Some(trader) = self_weak.upgrade() {
                        trader.complete_trade(offer_index, player_uuid).await;
                    }
                })
            }));
            Some(Arc::new(Mutex::new(handler)) as SharedScreenHandler)
        })
    }

    fn get_display_name(&self) -> TextComponent {
        TextComponent::translate("entity.minecraft.wandering_trader", [])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_day_night_drink_selection() {
        assert_eq!(required_drink(true, false), DRINKING_INVISIBILITY);
        assert_eq!(required_drink(false, true), DRINKING_MILK);
        assert_eq!(required_drink(true, true), DRINKING_NONE);
        assert_eq!(required_drink(false, false), DRINKING_NONE);
    }

    #[test]
    fn wander_target_uses_vanilla_int_array_key() {
        let target = BlockPos::new(-12, 70, 345);
        let mut nbt = pumpkin_nbt::compound::NbtCompound::new();
        nbt.put("wander_target", write_wander_target(target));
        assert_eq!(
            nbt.get_int_array("wander_target"),
            Some(&[-12, 70, 345][..])
        );
        assert_eq!(read_wander_target(&nbt), Some(target));

        nbt.put("wander_target", NbtTag::IntArray(vec![1, 2]));
        assert_eq!(read_wander_target(&nbt), None);
    }

    #[test]
    fn loaded_wandering_trader_age_is_never_negative() {
        assert_eq!(loaded_trader_age(-24_000), 0);
        assert_eq!(loaded_trader_age(120), 120);
    }

    #[test]
    fn use_item_finish_sound_uses_vanilla_pitch_range() {
        assert_eq!(use_item_finish_pitch(0.0), 0.9);
        assert_eq!(use_item_finish_pitch(1.0), 1.1);
    }

    #[test]
    fn trade_interaction_gate_matches_vanilla() {
        assert!(trade_interaction_eligible(false, true, false, false));
        assert!(!trade_interaction_eligible(true, true, false, false));
        assert!(!trade_interaction_eligible(false, false, false, false));
        assert!(!trade_interaction_eligible(false, true, true, false));
        assert!(!trade_interaction_eligible(false, true, false, true));
    }

    #[test]
    fn vanilla_trade_pool_sizes_and_offer_shape() {
        assert_eq!(COMMON_TRADES.len(), 68);
        assert_eq!(SPECIAL_TRADES.len(), 6);
        let offer = WanderingTraderEntity::merchant_offer(COMMON_TRADES[0]);
        assert_eq!(offer.uses, 0);
        assert_eq!(offer.max_uses, 5);
        assert_eq!(offer.xp, 1);
        assert!(offer.reward_exp);
        assert_eq!(offer.special_price, 0);
        assert_eq!(offer.price_multiplier, 0.05);
        assert_eq!(offer.demand, 0);
    }
}
