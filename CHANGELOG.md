# Changelog

## 1.14.0 — 2026-08-13

### Added

- **1.14.0 `parcel-bank`:** Slot-accurate `take_from_slot` / `put_stack` for bank, mail, and AH moves — worn gear and main-hand enchants survive deposit, withdraw, parcel, and listing.
- Client **K** **G** banks any non-quest stack (not junk-only); **I** **S**/**Y** send item or wallet copper, **1–9** numbered collect, **X** return; compose field for offline recipient names.
- `CharacterDirectory` on `Sim` plus persist `list_mailbox_directory` — mail to parked or offline characters by name.
- `RepairAll` includes banked gear; postage 1c, player inbox cap 20, 24h tick expiry with return to sender; AH listings carry durability/enchant on persist and expire/cancel as the same instance.
- Protocol rev stays **8** (`mail_postage`, mail/listing instance fields, `MailReturn` are additive).

## 1.13.0 — 2026-08-13

### Added

- **1.13.0 `gear-slots`:** Warrior/Rogue dual-wield routes a second OneHand into OffHand; Finger2 fills after Finger.
- Catalog `ItemQuality` multipliers (Poor 0.9 / Common 1.0 / Uncommon 1.1 / Rare 1.2) on AP/armor/sta/SP.
- Main-hand enchants from vendor oils: Coarse Whetstone (+6 AP), Minor Wizard Oil (+6 SP). Broken MH skips weapon AP and enchant stats.
- Client C-sheet lists Finger2, quality prefixes, and MH enchant; unequip keys **1–9**.
- Protocol rev stays **8** (`finger2`, `main_hand_enchant`, stack `enchant_id` are additive).

## 1.12.0 — 2026-08-13

### Added

- **1.12.0 `gear-depth`:** `can_equip` class/armor caps (Cloth→Plate) and weapon `allowed_classes`; two-hand/ranged weapons clear off-hand into bags.
- Jewelry slots: Neck + one Finger (additive on protocol rev **8**).
- Stamina (`sta * 2` HP) and spell power on heals/spells; sim-authoritative secondary stats on equip.
- Independent loot rolls: one pile per successful `LootEntry` (no early `break`).
- Upgrade drops: `crypt_cleaver`, `hag_focus`, pendant/ring ladder.
- Client character sheet shows AP/Armor/SP from snapshot; numbered bag keys **1–9**; unequip keys **1–8**.

## 1.11.0 — 2026-08-13

### Added

- NPC services (`1.11.0` / `npc-services`): `NpcService` roster and session snapshot for vendors, repairers, profession trainers, class trainers, and innkeepers.
- Quest-item vendor sell block plus capped buyback for recently sold items.
- Gear durability, combat wear, and sim-authoritative `RepairAll` at repair NPCs.
- NPC-gated profession training, class trainer confirmation, and innkeeper hearth bind/use flow.

## 1.10.0 — 2026-08-13

### Added

- Quest depth (`1.10.0` / `quest-depth`, protocol rev **8**): abandon (**L** then **X**), party share (**L** then **Y**), tick-epoch dailies (`wolf_patrol`), explore (`scout_north_road`), escort (`courier_to_the_gate` + `Escort` column), choice rewards at Wilkes (**1/2/3**).

## 1.9.0 — 2026-08-13

### Added

- Quest loop (`1.9.0` / `quest-loop`): giver and turn-in NPC checks, `QuestDef.requires` chains, ready toast, generic **E** accept/turn-in, quest log names + objective counts, offer-aware map markers.

## 1.8.0 — 2026-08-13

### Added

- **1.6.0 `class-engine`:** protocol rev **7** snapshot fields (`combo_points`, `stealthed`, `stance_id`, `absorb`) and `ToggleStealth` / `CycleStance` / `ToggleForm`.
- Absorb shields soak damage before HP; interrupt lockout on Kick / Earth Shock / Counterspell; rogue combo builder/spend; Execute dumps leftover rage.
- Stealth: rogue **Z**, aggro skip until melee, 0.7 move, breaks on hit or most abilities.
- Charge / Blink / Life Tap / Power Word: Shield engine verbs (stubs off-kit until 1.7). Frost Nova self-AoE. Hunter spends mana.
- **1.7.0 `class-identity`:** default bars put warrior Charge, mage Blink, priest Power Word: Shield, and hunter Aspect of the Hawk on slot 5. Rend / Shadow Word: Pain / Counterspell / Multi-Shot stay in `ABILITIES` off-bar. Aspect and Battle Shout auras grant 1.1 outgoing damage. Rogue action bar hints **[Z] Stealth**.
- **1.8.0 `class-forms`:** warrior **F** battle/defensive stance; shaman Lightning Shield + **F** Ghost Wolf; druid **F** Travel Form (1.4 move, breaks on hit); warlock Life Tap + Fear (breaks on damage); paladin Devotion Aura at spawn and Crusader Strike seal. Travel-form haste stacks with slows via min×max (stealth still `min`). Immolate / Flame Shock stay in `ABILITIES` off-bar.
- Class-kit identity: per-ability auras (no shared Rend-on-everything), Execute HP gate, Holy Shock heal-or-harm, stun/slow CC, paladin/shaman/druid heals; kits expanded to 4–5 slots.

## 1.5.0 — 2026-08-13

### Added

- `woc-update` crate: signed full `.tar.zst` pack, per-file bsdiff `.wocdelta`, staging apply, ed25519 manifest verification.
- `woc-pack` CI tool and `woc-updater` player launcher (Linux x86_64).
- `woc-pack --gen-key` prints ed25519 seed and pubkey hex for one-time secret setup.
- `/version` field `update_manifest_url` (`WOC_UPDATE_MANIFEST_URL` on the server).
- Packaged title **Update** button execs sibling `woc-updater` when incompatible and URL is set.
- `.github/workflows/client-release.yml` tag publish (full + optional delta from previous release).
- Runbook: [`docs/client-update.md`](docs/client-update.md).

## 1.4.0 — 2026-08-13

### Added

- Online client version gate (`client-compat`): title `/version` preflight, Hello identity, Welcome kick.

- `woc-version::check_compat` fail-closed policy (semver floor + exact `protocol_rev`).
- `GET /version` fields `protocol_rev` and `min_client_version` (`WOC_MIN_CLIENT_VERSION` override).
- Hello additive `protocol_rev` / `rewrite_version`; server rejects missing or stale identity before spawn.
- Title Online Continue blocked until compatible; Welcome protocol skew returns to Title.

## 1.3.0 — 2026-08-13

### Added

- Typed sparse-column `World` is the sim actor store (`AGENTS.md` + `docs/architecture/ecs.md`); fat `Entity` removed.
- Post-completion program: design, implementation plan, and parallel dispatch for `1.0.0` (`stable`) through `1.3.0` (`online-hard`).
- Wave 0 `stable`: nine locked tick phases; CI on `develop`; `docs/parity/DEMO.md`; drop stub/framework-slice copy.
- Wave 1 `combat-depth` on ECS columns: `AbilityEffect` (heal, cleave AoE, interrupt, taunt), miss/crit hit table, priest `flash_heal`, warrior `taunt`.
- Wave 2 `content-depth`: mining → blacksmithing (`copper_shortsword`); dungeon trash packs; `mirefen_barrow`; ability-mod talents (`cleave_targets_plus` / `heal_pct` / `crit_pct`).
- Wave 3 `online-hard`: park/resume on WS close; 80 yd snapshot AOI; Postgres documented as the `DATABASE_URL` production path.
- Rewrite version `1.0.0-pre` → `1.3.0`; parity `completion` → `online-hard`.
- Protocol rev **6**: pending Need/Greed loot snapshot, bank copper vault, market `mine` flag; `BankDepositCopper` / `BankWithdrawCopper`.
- Party Need/Greed is wired into mob loot spawn (eligible mates within 40 yd); pending piles skip FFA auto-pickup.
- `LootCorpse` interact claims ground piles / corpse-adjacent loot (respects rolls).
- Bank copper vault (deposit/withdraw) with durable persist (`bank_copper`).
- Client economy UX: bags equip/use/sell; bank numbered withdraw + copper vault; market list/buy/cancel; loot roll keys **1/2/3**; party loot mode **[** / **]**.
- Basic combat UX polish: keys **1–5** fire class kit slots; **Tab** cycles hostile targets; **Esc** clears target and stops auto-attack.
- Protocol rev **5**: intent `clear_target`; snapshot `ability_bar` / `gcd` / `auto_attack` for the action-bar HUD.
- Action bar shows known/locked kit abilities with CD/GCD state; aura strip on the bar; ability-hit / damage-taken combat toasts.
- Advanced combat polish: talent **tier gates** (5 pts/tier), numbered spend (**1–5** in talent panel), effect/bonus summary on talent + character panels; **T** pet summon/dismiss for hunter/warlock; talent learn/respec toasts.
- Procedural character / creature visual catalog (`woc-sim::visual_catalog`): class-, template-, and role-keyed mesh recipes (players, NPCs, mobs, pets, loot).
- Bevy scene loading: Eastbrook building meshes from physics AABBs, hub beacons, zone-gate portal arches, campfire props; biome-tinted terrain bands + zone sky/ambient on travel.
- Character create: rotating 3D class preview silhouette behind the UI panel.
- In-world scene load for NPCs/mobs/herbs: nameplates, quest/vendor overhead markers, target ground ring, idle bob; gather nodes spawned into the realm with herb visuals.
- Enter-world toast summarizing NPC / foe / herb counts; visual spawn/despawn lifecycle for corpses, loot, and pets.
- Bevy **minimap** (top-right disc: zone label, coords, terrain, hubs/portals/quests/mobs/allies, facing arrow).
- Bevy **world map** window (**M**, Esc closes): current zone band terrain, POI legend, player arrow.
- Pure `woc-sim::map_view` projection + terrain paint (upstream +X-left / +Z-up canvas convention) with unit tests.
- Upstream-aligned locomotion hysteresis (`woc-sim::locomotion`) + procedural walk/run limb swing for players, NPCs, wolves, and boars.
- Soft visual remove fade when entities leave the snapshot (loot pickup, pet dismiss, disconnect); tipped corpse pose for dead actors.
- Shared climb-aware `entity_motion::step_toward` used by mob chase/leash and pet follow.
- Player vertical motion: jump (Space) with coyote window, gravity/landing, fall damage; lake swim tread + shore hop; travel flight toggle (V) with Space/Ctrl vertical.
- Protocol rev **4**: intent `jump` / `descend` / `fly_toggle`; snapshot `on_ground` / `flying` / `swimming` (serde defaults keep older peers working).

### Changed

- Mail panel keybind **M → I** (inbox) so **M** matches upstream World Map.
- Visual catalog parts carry a `PartRole` (body/head/legs/prop) so the client can drive a gait cycle.
- `step_player_motion` now takes full `PlayerIntent` and returns optional fall-damage effects.

### Fixed

- Clippy `too_many_arguments` on mail send / market list (nightly).
- Gather nodes are not auto-looted on proximity (Interact / Gather only).
- Need/Greed loot rules were implemented but never started on mob kills; FFA pickup no longer steals rolling piles.
- `LootCorpse` was a no-op stub.

## 1.0.0-pre — 2026-07-28

### Added

- Upstream-aligned continuous strip heightfield (seed `20061`, pin `a3e5e959`): biome bands, hubs, lakes, camps, Sowfield flatten, ridge/rim walls, terrace.
- Golden harness `crates/woc-sim/tests/data/terrain_golden.json` (noise + height/steepness; ε≈1e-3 vs JS).
- `WorldSpatial` content layer + absolute strip coordinates for Eastbrook / Eastfen / Mirefen / Thornpeak.
- Chunked Bevy terrain/water sampling via the same `terrain_height` / `ground_height` functions.
- Talents: class trees (3/class, tiers 1–2), spend/respec; damage/hp/armor/resource effects.
- Party loot rules: FFA + Need/Greed rolls via sim RNG; kill credit share within 40 yd.
- Economy: personal bank deposit/withdraw; mail send/collect; auction list/buy/cancel/expire.
- Durable realm economy (`realm_economy` / in-memory): mail + auction survive restart; offline AH settlement via system mail.
- Online persist loop: authenticated WS Hello (`token` + `character_id`), inject CharacterSave on enter, autosave on disconnect + periodic economy checkpoint.
- Professions sim: train herbalism/alchemy → gather nodes → craft recipes.
- Zone transition Eastbrook ↔ Eastfen ↔ Mirefen ↔ Thornpeak without wiping player progression.
- Thornpeak zone3 NPCs/mobs/quests; deeper talent trees.
- Dungeon instances with unique keys (party-shared); overworld actors preserved.
- Delve loop (`eastbrook_hollow`): 3 rooms → advance → reward.
- Light PvP: duel challenge/accept, open-world PvP flag, honor currency.
- World boss deed (`mire_terror` / eastfen_mire_terror): one-shot honor, persisted `completed_deeds`.
- Consumable HoT linger after rations/salves.
- Protocol rev 3: authenticated Hello fields; absolute inventory/bank `slot` indices.
- Persist R4+ character fields: zone, talents, bank, honor, professions, deeds (backward-compatible JSON).
- Bevy client panels: talents (N), bank (K), mail (M), market (U).

### Changed

- World bounds: `WORLD_MAX_X=180`, `z ∈ [-180, 900]` (replaces square `WORLD_HALF=120` bowl).
- Zone portals teleport on the shared coordinate system without wiping other-zone actors.
- Player/mob/pet motion climb limit `1.5` rise/run + `ground_height` footing.
- Parity target `online-alive` → `completion`.
- Rewrite version `0.3.0` → `1.0.0-pre`.
- Mirefen filled from placeholder to open-world content.
- Online WS snapshots are per-player (no longer broadcast primary-player view to all sockets).

## 0.3.0 — 2026-07-28

### Added

- Multi-player sticky realm: per-player xp/copper, intent map, spawn/despawn without resetting Eastbrook.
- Expanded `SimContext` + locked tick-phase fingerprint.
- Death / spirit release / Eastbrook graveyard respawn.
- Combat core: GCD, timed casts, DoT auras, threat stub; snapshot auras/cast/`is_dead`.
- Deeper bags: Head/OffHand/Legs/Feet, `UseItem` consumables, level-req equip.
- Tab targeting + clear target.
- Bevy client online mode (`ws://127.0.0.1:8787/ws/game`) alongside offline.
- Content stubs: talents, zone2 placeholders, graveyards, dungeons.
- Client module split (title / char create / world / input / hud / online).

### Changed

- Parity target `framework` → `online-alive` (upstream pin remains 0.31.0).
- Rewrite version `0.2.0` → `0.3.0`.

## 0.2.0 — 2026-07-28

### Added

- `woc-content` crate: classes, abilities, items, mobs, NPCs, quests, Eastbrook layout.
- Protocol rev 2: interactions, inventory/equipment/quest snapshots, `WorldHost`, WS envelopes.
- Framework sim: content-driven Eastbrook spawn, backpack inventory, equipment + stat recalc.
- Nine class create path with starter kits and primary abilities.
- Quest accept / kill-collect credit / turn-in (≥3 Eastbrook quests).
- NPC talk + vendor buy/sell.
- `woc-server` WebSocket host at `/ws/game` embedding `woc-sim`.
- Bevy client: class select, E interact, B bags, L quest log, quest tracker.

### Changed

- Parity target `combat-slice` → `framework` (upstream pin remains 0.31.0).

## 0.1.0 — 2026-07-27

### Added

- Cargo workspace scaffold: `woc-version`, `woc-protocol`, `woc-sim`, `woc-server`, `woc-client`.
- Upstream pin tracking via `VERSION.toml` / `UPSTREAM.md` against TypeScript World of ClaudeCraft **0.31.0** (`a3e5e9596a8e`).
- Deterministic 20 Hz sim with mulberry32 RNG, Eastbrook-like heightfield, Warrior motion.
- Combat slice: wolf camp, auto-attack, Heroic Strike, XP/loot/level-up.
- Bevy offline client (title → character create → in-world) with minimal HUD.
- Thin `woc-server` exposing `/health` and `/version`.
- Parity checklist in `docs/parity/STATUS.md`.
