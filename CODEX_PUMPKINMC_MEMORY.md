# PumpkinMC Codex Memory — Java 1.21.4 Parity

Last compacted: 2026-09-01 (Asia/Dhaka)  
Repository: `C:\Users\potato\Desktop\Minecraft Rust\pumpkin`  
Branch: `master`  
Upstream: `Pumpkin-MC/Pumpkin`  
Fork: `potatosips/Pumpkin`

## Mission

Bring this Rust Pumpkin server toward genuinely verified one-to-one Minecraft Java Edition 1.21.4 gameplay parity. This includes protocol behavior, mechanics, physics, AI, commands, inventories, items and data components, block entities, dimensions, persistence, redstone, timing, malformed input, and client-visible behavior.

Never equate a successful build, client login, or narrow unit test with global parity. Work in coherent batches; verify against Mojang 1.21.4 mappings/bytecode and, where practical, reproduce the same scenario on Vanilla and Pumpkin.

## Working rules

- Inspect the current worktree before relying on this memory. Source, official 1.21.4 bytecode, repeatable runtime evidence, and newer corrections outrank old narrative claims.
- Preserve unrelated user changes. Never stage or rewrite them incidentally.
- Use `rg` for discovery and `apply_patch` for edits.
- Validation ladder: focused test, `cargo check -p pumpkin`, `cargo fmt --all -- --check`, `git diff --check`, then live/dual-server testing when behavior is observable only at runtime.
- Do not claim completion while any feature family or required runtime evidence remains missing.
- Do not delegate to Gemini. Codex owns implementation and validation unless the user explicitly changes that.
- Record only durable facts here. Raw chat transcripts, repeated status updates, speculative plans, and superseded handovers are intentionally excluded.

## Environment

- Project root: `C:\Users\potato\Desktop\Minecraft Rust`
- Pumpkin source: `C:\Users\potato\Desktop\Minecraft Rust\pumpkin`
- Vanilla 1.21.4 server: `C:\Users\potato\Desktop\Minecraft Rust\vanilla_server`
- Prism test client: `C:\Users\potato\Desktop\Minecraft Rust\PrismLauncher`
- Differential bots: `C:\Users\potato\Desktop\Minecraft Rust\test_bot`
- Local Pumpkin Java port: `25565`; local Vanilla comparison port: `25575`
- Public test server: `pumpkinmc.dnslab.win`; recommended client `1.21.4`; supported range advertised in README as `1.7.2–26.2`
- Remote deployment path previously used: `/home/cloud-user/docker/pumpkin/` on the Singapore host. Credentials are deliberately not stored in this file.

## Build and deployment quick reference

The repository README is the user-facing authority for full instructions.

Windows:

```powershell
cargo build -p pumpkin
cargo build -p pumpkin --release
```

Outputs are under `target\debug\pumpkin.exe` or `target\release\pumpkin.exe`. Run from a directory containing the intended `pumpkin.toml` and world data.

Docker:

```bash
docker build -t pumpkinmc .
docker run --name pumpkin -p 25565:25565 -p 19132:19132/udp -v pumpkin-data:/pumpkin pumpkinmc
```

For production, use Compose with persistent config/world volumes, inspect with `docker logs -f`, stop cleanly before replacing a container, and back up persistent data before upgrades.

## Authoritative completed-work summary

The git history is the exact implementation record. Major verified batches include:

- Protocol/login stability: Java entity metadata disconnect corrections, malformed-packet hardening, login timeout/rate handling, spawn/respawn and cross-dimension synchronization, Java/Bedrock compatibility fixes.
- Commands and data: broad `/data` target/source/modify grammar, selectors, gamerules, scoreboard behavior, item/entity slots, text components, storage persistence, property-bearing block predicates, and error/result parity.
- Inventory and items: anvil calculations, enchanting, crafting remainders and repair, brewing, lectern permissions/validity, books, item components, equipment persistence, durability, death drops, and menu transfer rules.
- Entities and persistence: shared living/mob NBT, attributes/modifiers, cross-chunk snapshot protection, falling-block state/NBT/landing/restart behavior, owner/tame/love/age state, equipment and drops.
- Combat/projectiles: explosion gamerules, projectile deflection, arrows/crossbows, potion and spectral effects, knockback, TNT behavior, ender pearls, chorus fruit, armor/enchantment mitigation, and mob equipment enchantment paths.
- Blocks/world: falling blocks, anvils, concrete powder, dragon eggs, scaffolding, pointed dripstone, amethyst, bubble columns, cactus, sugar cane, bamboo, leaves/mangrove propagules, farmland/path survival, ice/frosted ice, weather ice/snow, cauldrons, coral, kelp/seagrass, vines/cave vines, cocoa, azalea, nylium, grass/mycelium, turtle eggs, note blocks, lily pads, and mushroom growth.
- Passive mobs: growth/breeding lifecycle, sheep color mixing/dyeing, cow/mooshroom milk/stew/lightning, pigs/villagers under lightning, parrots, wolves and wolf armor, cats/ocelots, rabbits, foxes, sniffers, turtles, frogs/frogspawn/tadpoles, chickens, and snow golems.
- Equines: age/food/healing/temper/taming, ownership and persistence, breeding/genetics and mule crosses, variants/llama strength, saddle and body equipment, chested storage, mount screens, quick move and validity, death drops, steering/charged jump, armor mitigation, regeneration, skeleton/zombie horse core, skeleton traps, regional difficulty, rider equipment/enchantments, undead genetics, feeding sounds, mouth/rearing metadata and exact animation lifetimes.

Recent fork commits at compaction:

- `9f576c6ba` — equine eating animations and sound parity
- `2de6dcc1d` — equine mounts and skeleton-trap parity
- `b56310939` — persistent attribute modifiers
- `e6195ddfa` — equine breeding genetics and equipment persistence
- `ce065a964` — horse-family taming and breeding parity
- `c091ec2e9` — broad Java 1.21.4 gameplay parity batch

## Important corrections to old handovers

- Old Gemini claims such as “100% parity,” “all tests prove parity,” and fixed test totals were not valid completion evidence.
- Minecraft Java 1.21.4 uses protocol 769. Old documents that label it protocol 776 are wrong.
- Generated latest-version item data is not automatically authoritative for 1.21.4. In particular, 1.21.4 `Equippable` does not contain later `equip_on_interact`, `can_be_sheared`, or `shearing_sound` fields; saddle shearing must not be backported into the 1.21.4 target.
- Runtime server status in old documents is historical, not current. Always inspect live processes/ports before relying on it.
- A passing workspace test suite proves only covered behavior. It does not establish complete Vanilla parity.

## Latest verified equine evidence

- Mojang 1.21.4 mapped bytecode confirms skeleton/zombie horses reject interaction while untamed and delegate to `AbstractHorse.mobInteract` when tamed. They now receive normal applicable horse-food healing/growth without breeding.
- `Horse.mobInteract` and `AbstractChestedHorse.mobInteract` feed first, then make an untamed horse rear on a non-food item. Pumpkin now mirrors that ordering for horse, donkey, mule, and llama.
- AbstractHorse mouth flag `0x40` lasts 30 ticks. Standing flag `0x20` clears eating and lasts 20 grounded ticks; airborne ticks pause the standing counter. Focused tests cover these boundaries.
- Skeleton-trap bytecode evidence confirms 10-block activation, visual lightning, four horse/rider pairs, 60-tick invulnerability, persistence, bow/helmet equipment, zero helmet drop chance, regional enchantments, and triangular launch velocity deviation `1.1485`.
- Skeleton traps use regional effective difficulty multiplied by `0.01` and obey mob-spawning rules.

## Current claim boundary and next work

Global parity remains unproven. Continue from current code evidence, not from the order of old handovers. Highest-value remaining work includes:

1. Runtime/client differential tests for mount screen lifecycle, equine steering/jump feel, mouth/rearing animation, underwater skeleton-horse behavior, and the complete skeleton-trap encounter.
2. Core AI/navigation gaps: partial-block collision shapes, fence/door routing, swimming, path existence/range/home restrictions, jumping in move control, and team-aware targeting.
3. Ender dragon/boss behavior, advanced mob goals, villager systems, and other entity families with explicit TODOs or incomplete runtime paths.
4. Redstone/block-entity edge cases: piston carried state, comparator/light updates, jukebox/shulker events, sculk shrieker/warden behavior, and scheduled-neighbor ordering.
5. Persistence stress cases: crash recovery, autosave races, unloaded destinations, passenger/vehicle trees, cross-dimension references, and species-specific NBT not covered by focused tests.
6. Exact advancements/criteria, statistics, loot tables, recipes, world generation, structures, raids, portals, dimensions, and malformed/network edge cases across all supported client paths.

For each new batch, capture the Vanilla source/bytecode fact, Pumpkin path, implementation, focused tests, build gates, runtime comparison if needed, and the honest remaining boundary.

## Compaction record

This file replaces and compacts the former root-level documents:

- `BUILD_WINDOWS_EXE.md`
- `DOCKER_DEPLOYMENT.md`
- `MEMORY.md`
- `PROJECT_HANDOVER_CONTEXT.md`
- `gemini pumpkin 1.md`
- `codex pumpkin 1.md`
- `codex-pumpkin.md`
- the previous oversized `CODEX_PUMPKINMC_MEMORY.md`

Those files contained duplicate conversation transcripts, stale runtime claims, superseded agent instructions, and repeated progress logs. Unique durable facts and operational instructions were retained above; repository README files and unrelated documentation are intentionally preserved.
