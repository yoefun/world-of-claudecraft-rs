# Roadmap

| Rewrite | Parity target | Intent |
| --- | --- | --- |
| **0.1.0** (shipped) | `combat-slice` | Bevy offline Warrior combat: wolves, XP/loot, thin server health |
| **0.2.0** (shipped) | `framework` | Content tables, 9 classes, inventory, quests, vendor, WS host |
| **0.3.0** (shipped) | `online-alive` | SimContext + multi-player Entity; online client; death; combat/motion/bags core |
| **0.4–0.9** (folded into 1.0-pre) | persist → professions-pvp | Landed via R1–R3 parallel batches on `develop` |
| **1.0.0-pre** (shipped) | `completion` | Talents, loot rules, bank/mail/market, professions, zones, dungeon, PvP, deeds |
| **1.0.0** (this branch) | `stable` | Tick-phase contract, CI on `develop`, docs/demo hygiene |
| **1.1.0** (this branch) | `combat-depth` | Data-driven ability effects: heal, AoE, miss/crit, interrupt, taunt |
| **1.2.0** (this branch) | `content-depth` | Mining/smith, dungeon trash, second instance, ability-mod talents |
| **1.3.0** (shipped) | `online-hard` | Reconnect park/resume, snapshot AOI, Postgres production notes |
| **1.4.0** (shipped) | `client-compat` | Online version gate (title preflight + Hello identity) |
| **1.5.0** (shipped) | `client-update` | Signed full + bsdiff delta packages; `woc-updater` launcher (Linux x86_64) |
| **1.6.0** (shipped) | `class-engine` | Combo, stealth, absorb, interrupt lockout, Charge/Blink/Life Tap, hunter mana |
| **1.7.0** (shipped) | `class-identity` | Rogue stealth+combo, priest shield, warrior Charge, mage Blink, hunter Aspect |
| **1.8.0** (shipped) | `class-forms` | Warrior stances, shaman/druid forms, paladin aura/seal, warlock Fear |
| **1.9.0** (shipped) | `quest-loop` | Accept / progress / complete gates, prerequisite chains, generic E / log / map |
| **1.10.0** (shipped) | `quest-depth` | Abandon, party share, daily reset, explore/escort objectives, choice rewards |
| **1.11.0** (shipped) | `npc-services` | Vendor buyback, durability/repair, profession/class trainers, hearth |
| **1.12.0** (shipped) | `gear-depth` | Class gear rules, jewelry, secondary stats, upgrade drops |
| **1.13.0** (shipped) | `gear-slots` | Dual-wield, Finger2, catalog quality, main-hand enchants |
| **1.14.0** (shipped) | `reputation` | Hub factions, standing ladder, vendor discount/gates |
| **1.15.0** (shipped) | `gear-more` | Extra slots, Hunter DW, OH enchant, instance loot quality |
| **1.16.0** (shipped) | `economy-depth` | Auctioneer, bids, 12/24/48 h, soulbound, banker + mailbox NPCs |
| **1.17.0** (shipped) | `party-depth` | Party verbs, frames, XP split, park-safe roster, ready check |
| **1.18.0** (shipped) | `raid` | Convert 5→10, two groups, raid chat, realm cap 10 |
| **1.19.0** (shipped) | `guilds` | Create/invite/ranks, guild+officer chat, MOTD, persist |
| **1.20.0** (shipped) | `parcel-bank` | Offline parcels, postage/cap/expiry/return, client compose, bank repair |
| **1.21.0** (shipped) | `mounts` | Riding ranks, learnable mounts, **V** toggle, Expert flying |
| **1.22.0** (shipped) | `kill-loop` | Per-template respawn, leash reset, loot count/TTL, pet credit, mob abilities |
| **1.23.0** (shipped) | `dungeon-depth` | Playable Crypt/Barrow enter/leave, isolation, parent GY |
| **1.24.0** (shipped) | `delve-depth` | Isolated Hollow keys, auto-advance, entrance `(8, -6)` |
| **1.25.0** (shipped) | `class-depth` | Distinct regen, 5-slot kits, paladin aura cycle, pet Bite/Firebolt, HUD stance |

## Class depth (shipped as `1.25.0`)

**Audit (2026-08-14):** playable class system **68%** vs this program’s scorecard. `1.6.0`–`1.8.0` identity DoD is ~95% shipped; remaining work is depth, not a second spellbook port.

**Definition of done:** [`docs/superpowers/specs/2026-08-14-class-depth-design.md`](superpowers/specs/2026-08-14-class-depth-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-14-class-depth.md`](superpowers/plans/2026-08-14-class-depth.md)

Energy 10/s, mana 8/2 s, rage decay OOC; rogue Sprint + hunter Multi-Shot + priest SW:P + mage Counterspell on the 1–5 bar; paladin **F** Devotion/Retribution; HUD paints `stance_id`; hunter Bite / warlock Firebolt. Protocol rev stays **10**. Ability ranks, 3 talent specs, bear/cat, and pet bars stay out of scope. If another wave takes `1.25.0` first, shift by one.

## Completion program (closed)

**Definition of done:** [`docs/superpowers/specs/2026-07-28-rust-rewrite-completion-design.md`](superpowers/specs/2026-07-28-rust-rewrite-completion-design.md)  
**Implementation + parallel dispatch:** [`docs/superpowers/plans/2026-07-28-rust-rewrite-completion.md`](superpowers/plans/2026-07-28-rust-rewrite-completion.md)

Gameplay-core rewrite against upstream **0.31.0** is **shipped** as `1.0.0-pre`. Remaining work is contract-close and depth, not a second port.

## Post-completion program (current)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-post-completion-program-design.md`](superpowers/specs/2026-08-13-post-completion-program-design.md)  
**Implementation + parallel dispatch:** [`docs/superpowers/plans/2026-08-13-post-completion-program.md`](superpowers/plans/2026-08-13-post-completion-program.md)  
**Max-parallel schedule:** [`docs/superpowers/plans/2026-08-13-parallel-post-completion.md`](superpowers/plans/2026-08-13-parallel-post-completion.md)

Upstream pin remains **0.31.0** unless explicitly bumped. Browser/Electron/Web3/RL/admin/i18n stay non-goals. New per-actor gameplay state must be a `World` component column (`AGENTS.md`); do not reintroduce a fat `Entity`.

## Reputation (shipped as `1.14.0`)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-reputation-design.md`](superpowers/specs/2026-08-13-reputation-design.md)

Four hub factions on a player `Reputation` column. Quest and kill grants; Friendly vendor discount and gated `watch_signet`; Unfriendly refuse. Protocol rev stays **8**.

## Gear depth (shipped as `1.12.0`)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-gear-depth-design.md`](superpowers/specs/2026-08-13-gear-depth-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-13-gear-depth.md`](superpowers/plans/2026-08-13-gear-depth.md)

Equipment stays on `Bags`. `can_equip` is the single class/armor/level gate. Two-hand and ranged weapons occupy the off-hand. Neck + one Finger. Stamina and spell power are sim-authoritative. Gear-depth shipped as `1.12.0` because `1.9.0` was taken by quest-loop. Durability/repair is in shipped NPC services.

## Guilds (this branch as `1.16.0`)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-guilds-design.md`](superpowers/specs/2026-08-13-guilds-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-13-guilds.md`](superpowers/plans/2026-08-13-guilds.md)

`GuildRoster` on `Sim`, keyed by durable character id (like mail). Parties stay ephemeral. Protocol rev **9**. Guild bank, calendar, friends/ignore stay out of scope.

## Gear slots (shipped as `1.13.0`)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-gear-slots-design.md`](superpowers/specs/2026-08-13-gear-slots-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-13-gear-slots.md`](superpowers/plans/2026-08-13-gear-slots.md)

Warrior/Rogue dual-wield a second OneHand into OffHand. Rings fill Finger then Finger2. Catalog `ItemQuality` multiplies gear stats. Vendor oils apply a main-hand enchant. Protocol rev stays **8**.

## Kill loop (shipped as `1.22.0`)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-kill-loop-design.md`](superpowers/specs/2026-08-13-kill-loop-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-13-kill-loop.md`](superpowers/plans/2026-08-13-kill-loop.md)

Close spawn → fight → loot → respawn: per-template respawn (instance trash never), leash HP reset, `MobSpot` packs, loot `count` + 120 s TTL, pet last-hit credits the owner, three mob abilities, 1.1× threat switch. Planned as `1.14.0`; renumbered to `1.22.0` after develop shipped `1.14.0`–`1.21.0` in parallel. Protocol rev stays **10**. No new tick phase.

## Gear more (shipped as `1.15.0`)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-gear-more-design.md`](superpowers/specs/2026-08-13-gear-more-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-13-gear-more.md`](superpowers/plans/2026-08-13-gear-more.md)

Extra armor + trinket slots. Hunter dual-wield. Off-hand oils on the sheet. Instance loot quality rolls after drop selection. Protocol rev stays **8**.

## Instance depth (shipped as `1.23.0` + `1.24.0`)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-instance-depth-design.md`](superpowers/specs/2026-08-13-instance-depth-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-13-instance-depth.md`](superpowers/plans/2026-08-13-instance-depth.md)

`1.23.0` makes 5-man dungeons playable (Bevy **E** at the portal, 5-yard sim gate, leave-to-entrance, snapshot isolation, pet follow, parent-zone graveyard, persist eject). `1.24.0` gives Eastbrook Hollow unique `{id}#{seq}` keys so it stops wiping the overworld, auto-advances cleared rooms, and moves the entrance to `(8, -6)` with client **E** enter and room HUD. These milestones were renumbered past kill-loop `1.22.0`. Protocol rev stays **10**. Dungeon Finder, lockouts, and 10-man raid encounters stay out of scope.

## Mounts and riding (shipped as `1.21.0`)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-mounts-riding-design.md`](superpowers/specs/2026-08-13-mounts-riding-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-13-mounts-riding.md`](superpowers/plans/2026-08-13-mounts-riding.md)

Free **V** travel flight is replaced by a gated mount loop. Players train riding at Stable Master Ross, learn a mount item, and toggle with **V**. Combat/instance dismount. Protocol rev stays **10** (additive riding fields). Tick fingerprint unchanged.

## Economy depth (shipped as `1.16.0`)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-economy-depth-design.md`](superpowers/specs/2026-08-13-economy-depth-design.md)

Auctioneer Lise, instance listings, 5% cut, mail-always proceeds, then bids, 12/24/48 h, client filter/pages, OnEquip/OnPickup binds, and Eastbrook Banker Holme plus Eastbrook Post. Protocol rev stays **8**.

## Parcel and bank (shipped as `1.20.0`)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-parcel-bank-design.md`](superpowers/specs/2026-08-13-parcel-bank-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-13-parcel-bank.md`](superpowers/plans/2026-08-13-parcel-bank.md)

Offline delivery uses a realm `CharacterDirectory` loaded at boot from persist. The client sends parcels (**S** in the mail panel) and collects by row; bank **G** deposits the first non-quest bag stack. Postage, inbox cap, tick-based expiry, and **MailReturn** are in. Repair includes banked gear. Banker/mailbox NPC gates from `1.16.0` stay. Additive on protocol rev **10**.


## Party depth (shipped as `1.17.0`) + raid (shipped as `1.18.0`)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-party-raid-design.md`](superpowers/specs/2026-08-13-party-raid-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-13-party-raid.md`](superpowers/plans/2026-08-13-party-raid.md)

`1.17.0` makes 5-man parties playable (invite/decline/kick/promote/disband, snapshot frames, classic XP split, park-safe membership, ready check). `1.18.0` converts a full party into a 10-player raid of two groups and raises `MAX_REALM_PLAYERS` to 10. Protocol rev **9**. Guilds and Dungeon Finder stay out of scope.

## Client version gate (current)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-client-version-update-design.md`](superpowers/specs/2026-08-13-client-version-update-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-13-client-version-update.md`](superpowers/plans/2026-08-13-client-version-update.md)

Online Bevy clients must not enter a realm with a mismatched rewrite version or `protocol_rev`. Hello identity fields stay additive; class-identity snapshot is protocol rev **7**. Packaged incremental updates shipped in **1.5.0**.

## Client update packages (current)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-client-update-packages-design.md`](superpowers/specs/2026-08-13-client-update-packages-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-13-client-update-packages.md`](superpowers/plans/2026-08-13-client-update-packages.md)  
**Runbook:** [`docs/client-update.md`](client-update.md)

Players start `woc-updater`. CI on version tags packs a zstd full archive plus a per-file bsdiff from the previous GitHub Release. Skip-version downloads full. Windows/macOS and Velopack/Electron stay out of scope.

## Shipped: quest loop (`1.9.0`)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-quest-loop-design.md`](superpowers/specs/2026-08-13-quest-loop-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-13-quest-loop.md`](superpowers/plans/2026-08-13-quest-loop.md)

Close the playable accept → progress → ready → turn-in loop (giver/turn-in NPC checks, `requires` chains, generic client **E**, named log + objective counts). Does not add new objective kinds, abandon, or a protocol rev.

## Shipped: quest depth (`1.10.0`)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-quest-depth-design.md`](superpowers/specs/2026-08-13-quest-depth-design.md)

Abandon (**L** then **X**), party share (**L** then **Y**), tick-epoch dailies, explore/escort objectives, and turn-in choice rewards (**1/2/3**). Protocol rev **8**.

## Shipped: NPC services (`1.11.0`)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-npc-services-design.md`](superpowers/specs/2026-08-13-npc-services-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-13-npc-services.md`](superpowers/plans/2026-08-13-npc-services.md)

Town NPCs are the sim-authoritative front for buy/sell (quest-item block + buyback), durability/repair, profession trainers, class confirmation, and hearth bind. Bank/mail/AH stay HUD-gated. Protocol rev stays **8** (NPC session fields are additive). Rewrite target **1.11.0** / `npc-services`.

## Internal: sim ECS columns (done)

Gameplay actors in `woc-sim` live in a typed sparse-column `World` (simpler systems, O(1) lookup, sparse loot/NPC). The fat `Vec<Entity>` path is deleted. Parity/protocol unchanged.

**Design:** [`docs/superpowers/specs/2026-08-13-sim-ecs-design.md`](superpowers/specs/2026-08-13-sim-ecs-design.md)  
**Plan (historical):** [`docs/superpowers/plans/2026-08-13-sim-ecs.md`](superpowers/plans/2026-08-13-sim-ecs.md)  
**Rules:** [`docs/architecture/ecs.md`](architecture/ecs.md) · [`AGENTS.md`](../AGENTS.md)

## Parallel execution

Main agent freezes protocol/sim contracts per wave, dispatches subagents on isolated branches with exclusive path ownership, then merges by dependency and runs workspace tests. See the active plan’s “Main-agent merge playbook”.
