# Minecraft Server in Rust (Pumpkin) — Comprehensive AI Handover & Context

## 1. Executive Summary & Objectives
- **Project Goal**: Develop and customize a high-concurrency, modern Minecraft server in Rust based on the **Pumpkin** architecture, achieving 100% 1-to-1 feature, packet, physics, and AI parity with the official Mojang Vanilla Java Server (version `1.21.4`).
- **Development Workflow**: Live dual-server environment running side-by-side with automated headless bots and dual Minecraft client testing to benchmark, detect packet/behavior micro-differences, and implement precise fixes in Rust.

---

## 2. Infrastructure, Host & Live Ports Reference
- **Local Machine**: Windows 11 (`POTATO` / AtlasOS)
- **Active Workspace Directory**: `C:\Users\potato\Desktop\Minecraft Rust`
- **Pumpkin Server Source**: `C:\Users\potato\Desktop\Minecraft Rust\pumpkin`
- **Vanilla Server Directory**: `C:\Users\potato\Desktop\Minecraft Rust\vanilla_server`
- **Test Bot Suite Directory**: `C:\Users\potato\Desktop\Minecraft Rust\test_bot`

### Live Server Network Ports
| Service | Software | Port | Protocol / Auth Mode | Status |
| :--- | :--- | :--- | :--- | :--- |
| **Rust Server** | Pumpkin (Rust) | `25565` (Java), `19132` (Bedrock) | `online_mode = false`, `encryption = false` | Active Live |
| **Official Vanilla** | Mojang Vanilla Java 1.21.4 | `25575` | `online-mode = false` | Active Live |

---

## 3. Codebase Architecture & Structure
The server is organized into modular Cargo crates under `crates/`:
- **`crates/pumpkin`**: The primary server engine (game loop, world management, entity systems, AI goals/controllers, commands, block interactions).
- **`crates/pumpkin-protocol`**: Packet encoders/decoders for both Java (`1.21.4` protocol 769) and Bedrock protocols.
- **`crates/pumpkin-data`**: Generated registries, block states, item mappings, entity types, and entity metadata (`DataWatchers`).
- **`crates/pumpkin-world`**: Chunk generation, lighting, block storage, and section management.
- **`crates/pumpkin-util`**: Math, position primitives, text components, and utilities.

---

## 4. Key Mechanics & 1-to-1 Vanilla Parity Fixes Implemented

### A. Snow Golem Target Aim & Continuous Look Tracking
* **Files Modified**:
  - `crates/pumpkin/src/entity/ai/control/look_control.rs`
  - `crates/pumpkin/src/entity/ai/goal/snowball_attack.rs`
  - `crates/pumpkin/src/entity/ai/goal/look_around.rs`
  - `crates/pumpkin/src/entity/ai/goal/look_at_entity.rs`
  - `crates/pumpkin/src/entity/mob/mod.rs`
* **Root Cause & Fix**:
  1. **Body & Head Synchronization**: `LookControl::tick` now synchronizes `entity.head_yaw`, `entity.yaw` (body yaw), `entity.body_yaw`, and `entity.pitch` toward the attack target.
  2. **Controller Ticking Order**: In `Mob::tick`, `MoveControl::tick` runs **before** `LookControl::tick`, giving `LookControl` ultimate authority over final orientation so movement steps never overwrite target aim.
  3. **Ambient Distraction Suppression**: `RandomLookAroundGoal` and `LookAtEntityGoal` check `mob.target.is_some()`; if the mob has an active combat target, ambient looking is completely disabled, preventing golems from looking away while attacking.

### B. Snow Golem Chasing & Navigator Path Retention
* **Files Modified**:
  - `crates/pumpkin/src/entity/ai/pathfinder/mod.rs`
  - `crates/pumpkin/src/entity/ai/goal/snowball_attack.rs`
* **Root Cause & Fix**:
  1. **Path Thrashing**: `Navigator::set_progress` previously cleared `self.current_path = None` on every tick. Now, if the destination is <= 1.5m from the existing goal, the computed path is preserved, enabling smooth multi-block navigation.
  2. **Active Pursuit**: `SnowballAttackGoal` continuously navigates toward the target with `speed = 1.25` when distance is > 4.0m (16.0 m^2) or line of sight is broken.

### C. Snowball Friendly Fire & Knockback
* **Files Modified**:
  - `crates/pumpkin/src/entity/projectile/snowball.rs`
* **Fix**:
  - Removed exclusion preventing snowballs from hitting Snow Golems. Snowballs apply `0.4` horizontal knockback and trigger retaliatory revenge targeting on hit entities.

### D. Zombie & Undead Daylight Sun Burning & Visual Flames
* **Files Modified**:
  - `crates/pumpkin/src/entity/mob/mod.rs`
  - `crates/pumpkin/src/entity/mod.rs`
* **Root Cause & Fix**:
  1. **Direct Flag & Metadata Broadcast**: `set_on_fire_for_ticks` immediately stores `has_visual_fire = true` and updates `Flag::OnFire` (`0x01` on `DATA_SHARED_FLAGS_ID`).
  2. **Non-blocking Event Dispatch**: Combust plugin events are spawned asynchronously (`tokio::spawn`), preventing worker thread deadlocks.
  3. **Spawn Packet Metadata**: `send_java_spawn_packet` in `entity/mod.rs` packages the entity's current `flags` byte in `CSetEntityMetadata` alongside `CSpawnEntity` so joining/tracking players immediately see burning flame overlays.
  4. **Daylight Exposure Raycast**: Open-sky 40-block vertical raycast verifies absence of solid blocks above eye level during daytime (`0..12000`).

### E. Snow Layer Placement on Grass & Foliage
* **Files Modified**:
  - `crates/pumpkin/src/block/blocks/snow.rs`
  - `crates/pumpkin/src/entity/passive/snow_golem.rs`
* **Fix**:
  - `can_place_at` supports `GRASS_BLOCK`, `DIRT`, and solid blocks.
  - Snow Golems replace `SHORT_GRASS`, `FERN`, and `DEAD_BUSH` with snow layers when walking over them.

### G. Additional Comprehensive Parity Fixes Completed & Tested
1. **Respawn Anchor Comparator Output & Charges Scaling** (`crates/pumpkin/src/block/blocks/respawn_anchor.rs`):
   - Stepped redstone comparator output: 0 charges = 0, 1 = 3, 2 = 7, 3 = 11, 4 = 15.
2. **Group Revenge AI & Swarming for Pack Mobs** (`crates/pumpkin/src/entity/ai/goal/revenge.rs`, `zombified_piglin.rs`):
   - Implemented `RevengeGoal::set_alert_others()` alerting all nearby pack mobs within 32 blocks.
3. **Falling Anvil / Stalactite Helmet Damage Isolation** (`crates/pumpkin/src/entity/living.rs`):
   - Implemented `#minecraft:damages_helmet` tag filtering degrading only helmet slot durability.
4. **Global Tag System String Registry Key Fallback** (`crates/pumpkin-data/src/generated/tag.rs`, `damage_type.rs`):
   - Added string identifier fallback to `Taggable::has_tag` for 100% tag resolution across damage types.
5. **Sugar Cane Placement & Water/Waterlogged Check** (`crates/pumpkin/src/block/blocks/plant/sugar_cane.rs`):
   - Corrected neighbor checking for water, frosted ice, and waterlogged states.
6. **Mushroom Plant Ground Placement Rules** (`crates/pumpkin/src/block/blocks/plant/mushroom_plant.rs`):
   - Added `#minecraft:overrides_mushroom_light_requirement` and solid block support.
7. **Big Dripleaf Projectile Impact & Tilt States** (`crates/pumpkin/src/block/blocks/plant/big_dripleaf.rs`):
   - Projectile impact tilts dripleaf directly to `Tilt::Full` with authentic sound.
   - Timed progression `Tilt::Unstable` (10t) -> `Tilt::Partial` (10t) -> `Tilt::Full` (100t) -> `Tilt::None`.
8. **Splash Potion Campfire/Candle Extinguishing** (`crates/pumpkin/src/entity/projectile/splash_potion.rs`, `ignition.rs`):
   - Extinguishes lit campfires, soul campfires, and candles with extinguish audio.
9. **Chiseled Bookshelf Statistics** (`crates/pumpkin/src/block/blocks/chiseled_bookshelf.rs`):
   - `StatisticCategory::Used(Item::CHISELED_BOOKSHELF)` incremented on book insertion/removal.
10. **Farmland Rain Hydration** (`crates/pumpkin/src/block/blocks/farmland.rs`):
    - Open-sky rain hydrates farmland to moisture level 7 during random ticks.
11. **Piglin Pacification & Golden Armor Safe Check** (`crates/pumpkin/src/entity/mob/piglin.rs`):
    - Golden helmet, chestplate, leggings, and boots pacify Piglins.
12. **Beacon Pyramid Power Tiers, Range & Duration Scaling** (`crates/pumpkin/src/block/entities/beacon.rs`):
    - 4 pyramid tiers ($20, 30, 40, 50$ block radius; $220, 260, 300, 340$ tick duration).
13. **Enchanting Table Lapis & Level Consumption** (`crates/pumpkin-inventory/src/enchanting/enchanting_screen_handler.rs`):
    - Exact Vanilla lapis (1-3) and level (1-3) consumption and seed reroll.
14. **Phantom AI & Insomnia Mechanics** (`crates/pumpkin/src/entity/mob/phantom.rs`):
    - Circling and swoop attack AI with flap, swoop, and bite sounds; dynamic size damage formula.
15. **Shulker Combat & Closed-Shell Defense** (`crates/pumpkin/src/entity/mob/shulker.rs`, `shulker_bullet.rs`):
    - 20-armor closed-shell defense (80% mitigation) and projectile immunity.
16. **Brewing Stand Automation & Crafting Remainders** (`crates/pumpkin/src/block/entities/brewing_stand.rs`):
    - Fuel consumption and bottle/bucket remainder conversion on brew finish.
17. **Anvil Prior Work Penalty & Repair Formulas** (`crates/pumpkin-inventory/src/anvil/anvil_screen_handler.rs`):
    - Prior work exponential scaling ($P(k) = 2k + 1$) and slot 1 item consumption.
18. **Armor Stand Equipment Drops** (`crates/pumpkin/src/entity/decoration/armor_stand.rs`):
    - Equipment drops on all destruction paths.
19. **Silk Touch Experience Suppression** (`crates/pumpkin/src/block/mod.rs`):
    - Suppressed XP drops on blocks broken with Silk Touch.
20. **Experience Orb Magnetism** (`crates/pumpkin/src/entity/experience_orb.rs`):
    - 8-block inverse quadratic player attraction.
21. **Daylight Detector Weather Calculation** (`crates/pumpkin/src/block/entities/daylight_detector.rs`):
    - Sun angle, rain (5.0 multiplier), thunder, and inverted mode power calculation.

---

## 5. Verification & Test Suite Status
- **Vanilla Parity Test Suite**: **76 tests passed, 0 failed**.
  - `pumpkin`: 58 tests passed
  - `pumpkin-inventory`: 18 tests passed
- **Full Workspace Compilation**: `cargo check --workspace` passed with 0 errors.
- **Server Binary**: `cargo build --bin pumpkin` built successfully.
- **Run Tests Command**:
  ```powershell
  $env:Path = "C:\Program Files\Git\cmd;C:\Users\potato\.cargo\bin;" + $env:Path
  cargo test -p pumpkin vanilla --no-fail-fast
  cargo test -p pumpkin-inventory
  ```



