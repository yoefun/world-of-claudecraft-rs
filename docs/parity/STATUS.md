# Parity status

**Current rewrite:** `1.25.0` / `friends` (PROTOCOL_REV=11).  
**Post-completion program:** closed through `online-hard` — see [`docs/ROADMAP.md`](../ROADMAP.md).  
**Runbook:** [`../client-update.md`](../client-update.md). Class identity is `1.6.0`–`1.8.0`; quest-loop/depth are `1.9.0`–`1.10.0`; NPC services is `1.11.0`; gear depth is `1.12.0`; gear slots shipped as `1.13.0`; reputation shipped as `1.14.0`; gear-more shipped as `1.15.0`; economy depth shipped as `1.16.0`; party-depth shipped as `1.17.0`; raid shipped as `1.18.0`; guilds shipped as `1.19.0`; parcel-bank shipped as `1.20.0`; mounts shipped as `1.21.0`; kill-loop shipped as `1.22.0`; dungeon-depth shipped as `1.23.0`; delve-depth shipped as `1.24.0`; friends shipped as `1.25.0`. Dungeon/delve were renumbered past kill-loop `1.22.0`.

## Friends (`friends`) — done

Design: [`../superpowers/specs/2026-08-14-friends-design.md`](../superpowers/specs/2026-08-14-friends-design.md)  
Plan: [`../superpowers/plans/2026-08-14-friends.md`](../superpowers/plans/2026-08-14-friends.md)

| Subsystem | Status | Notes |
| --- | --- | --- |
| add/remove | done | directory or live name; unidirectional; cap 50 |
| ignore / unignore | done | ignore kicks friend; cap 50; whisper-only filter |
| whisper | done | targeted `player_tx`; parked = offline |
| presence | done | intents; `{name} has come/gone online.` |
| persist | done | `RealmEconomy.social` |
| client O panel | done | `/add` `/w` `/ignore` `/invite`; party/market O still win |
| protocol | done | rev 11 |

## Kill loop (`kill-loop`) — done

Design: [`../superpowers/specs/2026-08-13-kill-loop-design.md`](../superpowers/specs/2026-08-13-kill-loop-design.md)  
Plan: [`../superpowers/plans/2026-08-13-kill-loop.md`](../superpowers/plans/2026-08-13-kill-loop.md) · Polish: [`../superpowers/plans/2026-08-14-kill-loop-polish.md`](../superpowers/plans/2026-08-14-kill-loop-polish.md)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Per-template respawn | done | `Respawn.delay_sec`; wolves 30 s; `mire_terror` 300 s |
| Instance never-revive | done | Crypt/barrow/delve `delay_sec = 0` |
| Leash reset | done | Restore HP, clear auras/threat |
| Leash evade | done | No re-aggro until Home |
| `MobSpot` packs | done | `count` + `radius`; Wolf Run ≥5 |
| Loot `count` + TTL | done | Honor `LootEntry.count`; piles expire at 2400 ticks |
| Victim instance loot | done | Killer `InstanceAt` else victim |
| Quest skip Need/Greed | done | `ItemKind::Quest` piles stay FFA |
| Pet kill credit | done | `Owner` rewrite in `collect_pending_mob_kills` |
| Mob abilities | done | `wolf_bite` / `warden_smash` / `terror_slam` |
| Threat switch | done | 1.1× ratio |
| Portal zone seed | done | Tag bytes, not `tag.len()` |
| Protocol | done | Rev 10; additive `SimEvent::Loot.count` |

## Dungeon depth (`dungeon-depth`) — done

Design: [`../superpowers/specs/2026-08-13-instance-depth-design.md`](../superpowers/specs/2026-08-13-instance-depth-design.md)  
Plan: [`../superpowers/plans/2026-08-13-instance-depth.md`](../superpowers/plans/2026-08-13-instance-depth.md)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Client **E** enter/leave | done | Bevy **E** at Crypt/Barrow portal in and out |
| 5-yard sim gate + leave-to-entrance | done | Leave lands on portal, not zone spawn |
| Snapshot isolation | done | Cross-instance players/mobs hidden from `entities` |
| Pet `InstanceAt` follow | done | Pets copy owner instance; summon in-instance works |
| Parent-zone graveyard | done | Death release uses Eastbrook/Mirefen GY by parent zone |
| Persist eject | done | `instance:` saves load at parent entrance coords |
| Client | done | `EnterInstance` / `LeaveInstance` from `input.rs` |
| Protocol | done | Rev 10; additive `instance_id` / `instance_name` / `delve_room` |

## Delve depth (`delve-depth`) — done

Design: [`../superpowers/specs/2026-08-13-instance-depth-design.md`](../superpowers/specs/2026-08-13-instance-depth-design.md)  
Plan: [`../superpowers/plans/2026-08-13-instance-depth.md`](../superpowers/plans/2026-08-13-instance-depth.md)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Unique `{id}#{seq}` keys / no overworld despawn | done | Two players get two Hollows; Eastbrook wolves/NPCs stay |
| Auto-advance on kill tick | done | Room clears advance on the kill tick |
| Hollow entrance `(8, -6)` | done | Off spawn; **E** within 5 yards enters |
| Client **E** enter + room HUD | done | `EnterDelve` at portal; HUD shows `Eastbrook Hollow — Room N` |
| Leave / Hearth / release abort | done | No reward; lands on entrance coords |
| Dungeon Finder / lockout / raid boss | n/a | Explicit non-goals |

## Mounts / riding (`mounts`) — done

| Subsystem | Status | Notes |
| --- | --- | --- |
| Riding ranks | done | Apprentice L2 / Journeyman L5 / Expert L8 at Ross |
| Mount items | done | Pony 1.6, steed 2.0, gryphon flying 2.0; learn-on-use |
| **V** toggle | done | Replaces free travel flight; `fly_toggle` wire bit reused |
| Combat / instance dismount | done | Hit, ability, death, dungeon/delve, swim |
| Persist | done | Additive completion JSON; load dismounted |
| Client | done | Mount child mesh; riding rank on sheet; train button |
| Protocol | done | Rev 10; `TrainRiding` + `mounted` / `riding_rank` |

## Parcel and bank (`parcel-bank`) — done

Design: [`../superpowers/specs/2026-08-13-parcel-bank-design.md`](../superpowers/specs/2026-08-13-parcel-bank-design.md)  
Plan: [`../superpowers/plans/2026-08-13-parcel-bank.md`](../superpowers/plans/2026-08-13-parcel-bank.md)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Offline parcels | done | `CharacterDirectory` on `Sim`; realm-boot load |
| Client send / numbered collect / return | done | **S**/**Y**/**1–9**/**X**; compose field |
| Postage / inbox cap / expiry | done | 1c, 20, 24h ticks; system mail uncapped |
| Bank any non-quest stack | done | Client **G** deposits first non-quest bag stack |
| Repair includes warehouse | done | `repair_cost` sums `Bank.bank` |
| NPC gates | kept | Banker Holme / Eastbrook Post from `1.16.0` |

## Guilds (`guilds`) — done

Design: [`../superpowers/specs/2026-08-13-guilds-design.md`](../superpowers/specs/2026-08-13-guilds-design.md)  
Plan: [`../superpowers/plans/2026-08-13-guilds.md`](../superpowers/plans/2026-08-13-guilds.md)

| Subsystem | Status | Notes |
| --- | --- | --- |
| create/invite/leave | done | durable id; tick TTL 1200 |
| ranks / kick / transfer / disband | done | leader/officer/member |
| guild + officer chat | done | member-only fan-out |
| MOTD | done | officer+; max 240 |
| persist | done | `RealmEconomy.guilds` |
| client J panel | done | compose A-Z/digits/`/`; `/invite` by name; Ctrl+verbs; Esc closes |
| protocol | done | rev 10 |
| Guild bank / calendar / friends | n/a | Explicit non-goals |

## Party depth (`party-depth`) — done

Design: [`../superpowers/specs/2026-08-13-party-raid-design.md`](../superpowers/specs/2026-08-13-party-raid-design.md)  
Plan: [`../superpowers/plans/2026-08-13-party-raid.md`](../superpowers/plans/2026-08-13-party-raid.md)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Client invite / accept / decline | done | **G** on player target; **O**/**P** prompts |
| Kick / promote / disband | done | Leader verbs; panel **Y** / **Minus** / **Backspace** |
| Invite TTL | done | 600 ticks; pending dropped silently |
| Snapshot roster + frames | done | `party_members` even when mate is outside AOI |
| Park-safe membership | done | `park_player` must not `on_despawn` |
| Classic XP split | done | `group_xp`; n=2 → 75% each |
| Ready check | done | 300 ticks; parked do not block early complete |
| Protocol rev 9 | done | New WS verbs + additive-looking snapshot fields with a rev bump |

## Raid (`raid`) — done

| Subsystem | Status | Notes |
| --- | --- | --- |
| Convert 5-man → raid | done | Leader **Equals**; two groups of 5 |
| Realm cap 10 | done | `MAX_REALM_PLAYERS` 8 → 10 |
| Raid chat | done | `raid` channel; notices still realm-broadcast |
| Raid frames | done | `G1` / `G2` prefixes |
| 10-man encounter content | n/a | Explicit non-goal |

## Economy depth (`economy-depth`) — done

Design: [`../superpowers/specs/2026-08-13-economy-depth-design.md`](../superpowers/specs/2026-08-13-economy-depth-design.md)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Auctioneer Lise | done | Eastbrook `(4, 6)`; Talk opens session; `[U]` |
| Instance listings | done | Slot take; durability + enchant persist |
| Bidding | done | `MarketBid`; outbid mail; expire-win mails item + proceeds |
| Duration 12/24/48h | done | Fees 5/10/20c; ticks 864k / 1.728M / 3.456M |
| Search / pages | done | Client filter `/`; `[` `]` pages of 8 |
| Soulbound | done | OnEquip gear / OnPickup quest; blocks list + mail |
| Banker Holme | done | Eastbrook `(6, 6)`; `[K]`; `"Talk to a banker first."` |
| Eastbrook Post | done | Eastbrook `(0, 8)`; `[I]`; `"Talk to a mailbox first."` |
| Protocol | done | Rev 8 additive bid/bound/`can_bank`/`can_mail` |

## Gear more (`gear-more`) — done

| Subsystem | Status | Notes |
| --- | --- | --- |
| Extra slots | done | Shoulder/Back/Wrist/Hands/Waist + Trinket/Trinket2 |
| Hunter dual-wield | done | Warrior/Rogue/Hunter; shaman still no |
| OH enchant | done | Second oil → OH; full AP/SP; sheet `[enchant]` |
| Loot quality | done | Stack quality; `max(catalog, roll)` after drop list |
| Client sheet | done | Extra lines; unequip `0-=[]\;` |
| Protocol | done | Rev 8; additive extra slots / `off_hand_enchant` / `quality` |

## Reputation (`reputation`) — done

Design: [`../superpowers/specs/2026-08-13-reputation-design.md`](../superpowers/specs/2026-08-13-reputation-design.md)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Faction table + standing ladder | done | Watch / Circle / Ferry / Highwatch; Neutral 0 |
| Quest + kill grants | done | Party-shared on kills; `Reputation` column |
| Vendor discount / gates | done | Friendly 5%…Exalted 20%; Unfriendly refuse; `watch_signet` |
| Snapshot + persist | done | Additive `reputation` on rev **8**; completion JSON |
| Client sheet | done | **C** lists standing |

## Gear slots (`gear-slots`) — done

| Subsystem | Status | Notes |
| --- | --- | --- |
| Dual-wield | done | Warrior/Rogue; second OneHand → OffHand; OH AP ×0.25 |
| Finger2 | done | Fill Finger then Finger2; jewelry still no wear |
| Quality | done | Poor/Common/Uncommon/Rare multipliers on AP/armor/sta/SP |
| MH enchant | done | Whetstone +6 AP; wizard oil +6 SP; broken MH skips both |
| Client sheet | done | 9 slots; quality prefix; `[enchant]`; unequip 1–9 |
| Protocol | done | Rev 8; additive `finger2` / `main_hand_enchant` / `enchant_id` |

## Gear depth (`gear-depth`) — done

| Subsystem | Status | Notes |
| --- | --- | --- |
| `can_equip` class/armor | done | Cloth→Plate caps; weapon `allowed_classes` |
| Two-hand occupancy | done | Bow/staff/cleaver clear OH |
| Jewelry | done | Neck + one Finger |
| Stamina / spell power | done | `sta*2` HP; SP on heal/spell |
| Independent loot | done | One pile per successful `LootEntry` |
| Crypt / hag gear | done | `crypt_cleaver` / `hag_focus` |
| Client sheet | done | AP/Armor/SP; 1–9 bags; 1–8 unequip |
| Durability / repair | done | Shipped in `1.11.0` NPC services |
| Quality / enchants / sockets | n/a | Manufacturing draft / non-goal |

## NPC services (`npc-services`) — done

Design: [`../superpowers/specs/2026-08-13-npc-services-design.md`](../superpowers/specs/2026-08-13-npc-services-design.md)  
Plan: [`../superpowers/plans/2026-08-13-npc-services.md`](../superpowers/plans/2026-08-13-npc-services.md)

| Subsystem | Status | Notes |
| --- | --- | --- |
| `NpcService` roster | done | Smith, herbalist, innkeeper; vex trains; Bren repairs |
| Quest-item sell block + buyback | done | Cap 6; session-only |
| Durability + RepairAll | done | 40/30; 1c per point at smith/Bren |
| Profession trainer gate | done | Client Train buttons; `train_profession()` helper unchanged |
| Class trainer | done | Kit refresh toast; talents stay on N-panel |
| Hearth | done | Bind at Mara; 18_000 tick cooldown |

## Client version gate (`client-compat`)

| Subsystem | Status | Notes |
| --- | --- | --- |
| `check_compat` policy | done | `woc-version`; prerelease suffix stripped |
| `/version` protocol + min client | done | `WOC_MIN_CLIENT_VERSION` |
| Hello identity | done | Additive; missing → reject |
| Title Online preflight | done | Fail-closed |
| Welcome kick | done | `version:` → Title |

## Client update packages (`client-update`)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Full `.tar.zst` pack / unpack | done | `woc-update::pack_full` |
| Per-file bsdiff delta | done | One predecessor (`N-1 → N`); skip-version uses full |
| ed25519 signed manifest | done | `WOC_UPDATE_SIGNING_KEY` / `WOC_UPDATE_PUBKEY` |
| `woc-updater` apply + exec client | done | Staging swap; `--once` for title launch |
| `/version` `update_manifest_url` | done | `WOC_UPDATE_MANIFEST_URL` on server |
| Title **Update** button | done | Sibling `woc-updater` when incompatible |
| Linux tag publish workflow | done | `.github/workflows/client-release.yml` |
| Runbook | done | [`docs/client-update.md`](../client-update.md) |

## Class identity (`class-engine` → `class-forms`)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Protocol rev 7 identity snapshot | done | `combo_points` / `stealthed` / `stance_id` / `absorb`; `ToggleStealth` / `CycleStance` / `ToggleForm`. |
| Absorb before HP | done | Shield auras soak; depleted shields pop. |
| Interrupt lockout | done | Kick / Earth Shock / Counterspell set `cast_lockout` (≥1.5s). |
| Combo points | done | Sinister Strike builds; Eviscerate spends (`combo_per_point`). |
| Stealth | done | Rogue **Z**; aggro skip until melee; 0.7 move; breaks on damage / ability. |
| Charge / Blink / Convert | done | Engine verbs; Charge/Blink/Life Tap stubs off-kit until 1.7. |
| Hunter mana | done | Hunter `resource_type` is Mana. |
| Self-AoE | done | Frost Nova fires without a hostile target. |
| Rage from taken + Execute dump | done | Warriors gain rage when hit; Execute dumps leftover rage into damage. |
| 1.7 kit swaps (Charge/Blink/Shield on bar) | done | Warrior Charge; mage Blink; priest PW:S; hunter Aspect (1.1 damage). Dropped on-bar: rend, SW:P, Counterspell, Multi-Shot (still in `ABILITIES`). |
| 1.8 stance / form / shout / fear | done | **F** CycleStance / ToggleForm; Lightning Shield thorns; Fear/Travel Form break on damage. |

### Nine-class signatures

| Class | Signature |
| --- | --- |
| Warrior | Charge; **F** battle/defensive (Battle Shout 1.1 / armor_flat + outgoing 0.9) |
| Paladin | Devotion Aura at spawn; Crusader Strike seal DoT |
| Hunter | Aspect of the Hawk (1.1 dmg); mana |
| Rogue | **Z** stealth, combo, Cheap Shot |
| Priest | Power Word: Shield |
| Shaman | Lightning Shield thorns; **F** Ghost Wolf |
| Mage | Blink; Frost Nova self-AoE |
| Warlock | Life Tap; Fear (stun, breaks on damage) |
| Druid | **F** Travel Form (1.4 move, breaks on hit) |

## Quest loop (`1.9.0`)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Giver / turn-in NPC checks | done | Accept/turn-in fail unless the target template matches the table |
| `QuestDef.requires` chains | done | Breadcrumb → hub sequences; integrity + acyclic tests |
| Talk / collect coverage | done | Sim tests beyond the wolf kill path; ready toast |
| Generic **E** accept/turn-in | done | Drop Captain Alden hardcoded ids |
| Named log + objective counts | done | HUD uses `woc-content` `QuestDef` |
| Offer-aware map markers | done | Yellow/green from `npc_quest_offers`, not raw table membership |

## Quest depth (`1.10.0`)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Abandon | done | **L** then **X**; cannot abandon completed |
| Party share | done | **L** then **Y**; 40 yd; skips standing at giver |
| Daily | done | `QuestRepeat::Daily`; `DAILY_PERIOD_TICKS` (12_000); persist `completed_tick` |
| Explore / escort | done | `QuestObjective::{Explore,Escort}`; `Escort` column (not `Owner`) |
| Choice rewards | done | `TurnInQuest.reward_choice`; **1/2/3** at turn-in NPC; **E** does not auto-pick |
| Protocol rev 8 | done | `AbandonQuest` / `ShareQuest` / optional choice index |

## Post-completion (`stable` → `online-hard`)

Legend: `done` · `partial` · `planned` · `deferred` · `n/a`

| Subsystem | Status | Notes |
| --- | --- | --- |
| Tick-phase contract vs `tick_all` | done | Nine named phases matching `tick_all` (pets/auras/PvP/market). |
| CI on `develop` | done | Workflow push/PR includes `develop`. |
| Protocol/crate “stub” / “framework slice” copy | done | Shipped interact actions; crate blurbs. |
| 1.0.0 acceptance demo doc | done | `docs/parity/DEMO.md`. |
| Data-driven `AbilityEffect` | done | Content tables; combat dispatches on the enum. |
| Heal / AoE / interrupt / taunt | done | Priest `flash_heal`; paladin `holy_light` / `holy_shock`; shaman `healing_wave`; druid `rejuvenation`; warrior cleave AoE + `taunt`; shaman `earth_shock` / rogue `kick` / mage `counterspell` interrupt. |
| Miss / crit hit table | done | 5% miss / 10% crit via sim RNG; heals do not miss. |
| Mining + blacksmithing | done | Copper veins in Eastbrook + Eastfen; smelt bar; craft/equip `copper_shortsword`. |
| Dungeon trash packs | done | Crypt and barrow spawn `DungeonTrashSpot` packs on enter. |
| Second dungeon or delve | done | `mirefen_barrow` (boss `barrow_hag`). |
| Ability-modifying talents | done | 4th talent/class: cleave targets / heal% / crit%. |
| Park / resume on reconnect | done | WS close parks the entity; Hello with the same `character_id` resumes it. |
| Snapshot AOI | done | 80 yd for other players/mobs/pets; zone NPCs and pending loot always included. |
| Postgres as documented production path | done | `DATABASE_URL` is the durable realm path; memory is the dev default. |
| ECS `Entity` split | done | Sparse-column `World` is the required actor store. |

## Rewrite 1.0.0-pre ↔ upstream 0.31.0 (`completion`)

Legend: `done` · `partial` · `planned` · `deferred` · `n/a`

Completion design: [`docs/superpowers/specs/2026-07-28-rust-rewrite-completion-design.md`](../superpowers/specs/2026-07-28-rust-rewrite-completion-design.md)  
Completion plan: [`docs/superpowers/plans/2026-07-28-rust-rewrite-completion.md`](../superpowers/plans/2026-07-28-rust-rewrite-completion.md)

Sim ECS (internal, post-completion): [`../superpowers/specs/2026-08-13-sim-ecs-design.md`](../superpowers/specs/2026-08-13-sim-ecs-design.md) · [`../superpowers/plans/2026-08-13-sim-ecs.md`](../superpowers/plans/2026-08-13-sim-ecs.md)

### Gameplay-core (completion gate)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Version / upstream pin | done | `1.25.0` / friends (upstream still 0.31.0) |
| Quest accept / progress / turn-in loop | done | Giver/turn-in/requires gates; talk+collect tests; generic E; named log |
| `woc-content` Eastbrook tables | done | |
| Deterministic tick (20 Hz) | done | locked phase fingerprint |
| Seeded RNG (mulberry32) | done | |
| SimContext seam | done | emit/lookup/mutate |
| Multi-player actor economy | done | typed ECS columns in `World` (source of truth) |
| Sticky WS realm | done | authenticated Hello + per-player snapshots |
| Client online mode | done | token + character_id Hello |
| Death / spirit / graveyard | done | |
| Combat core (GCD/cast/aura/threat) | done | DoT + consumable HoT; ability bar + Tab/Esc combat UX; class-kit identity (Execute, CC, dual Holy Shock) |
| Deeper bags / consumables | done | absolute inventory/bank slot indices |
| Tab targeting | done | |
| Player motion / colliders | done | |
| Mob respawn / social aggro | done | |
| `woc-persist` + auth API | done | R4 fields + deeds; WS load/save loop |
| Client login / char select | done | |
| Multi-ability kits | done | 4–5 slots/class; Execute / HealOrHarm / named DoT-HoT-CC auras |
| Talents / loadouts | done | 4/class; tier gates; numbered spend + pet key; stat + ability-mod effects |
| Pets | done | hunter/warlock |
| Zone2 + zone3 / Mirefen | done | Eastfen + Mirefen + Thornpeak quests/mobs |
| Party + chat | done | kill credit within 40 yd + same instance |
| Group loot rules | done | FFA + Need/Greed; rolls start on mob loot; pending in snapshot |
| Dungeons / instances | done | unique instance keys; party share; overworld preserved; crypt/barrow trash |
| Delves | done | eastbrook_hollow 3-room loop + reward |
| Bank + mail | done | durable character bank + copper vault; mail keyed by character UUID |
| Auction market | done | Auctioneer Lise; instance listings; 5% cut; mail-always proceeds |
| Professions gather/craft | done | herbalism → alchemy; mining → blacksmithing sword |
| Duel + PvP honor | done | |
| World boss + deeds | done | one-shot deed completion persisted |
| Client economy/talent chrome | done | N/K/I/U panels; **M** world map + minimap |
| Heightfield terrain | done | Continuous strip seed `20061`; golden ε≈1e-3 vs upstream pin; editor custom maps deferred |
| Procedural character / scene visuals | done | Class/template mesh recipes; buildings, portals, zone atmosphere; create preview |
| In-world NPC/mob scene load | done | Nameplates, quest/vendor markers, target ring, gather herbs, spawn lifecycle |
| Entity walk / remove presentation | done | Locomotion hysteresis, procedural gait limbs, corpse tip, soft despawn fade |
| Jump / swim / travel flight | done | Coyote jump, gravity + fall damage, lake tread; mounts replace free V flight |
| Sim typed ECS columns | done | Sparse `World` in `woc-sim`; Bevy stays presentation-only |
| Byte-identical terrain/combat | n/a | Explicit non-goal |
| Minimap / world map UI | done | Functional Bevy paint (not full DESIGN.md chrome) |

### Explicit deferred / non-goals

| Subsystem | Status |
| --- | --- |
| Map editor / `setActiveWorldContent` custom heightfields | deferred |
| Browser Three.js / Electron / Capacitor | deferred |
| Web3 / wallets / cosmetics shop | deferred |
| Gymnasium RL / headless env | deferred |
| Full i18n catalogs | deferred |
| Admin SPA / Discord OAuth polish | deferred |
| Vale Cup / Card Duel / Fiesta | deferred |

## Prior rewrites

0.3.0 `online-alive`, 0.2.0 `framework`, 0.1.0 `combat-slice` — superseded by the table above.
