# Reputation — `1.14.0` / `reputation`

**Status:** Implementation (2026-08-13).  
**Rewrite target:** `1.14.0` / parity `reputation` (after shipped `1.13.0` / `gear-slots`).  
**Upstream pin:** unchanged (`0.31.0`).  
**Protocol:** stay on rev **8**. Snapshot `reputation` / vendor `discount_pct` and `SimEvent::ReputationChanged` are additive (`#[serde(default)]`).

## 1. Goal

Players earn standing with four hub factions. Standing is sim-authoritative: quest turn-in and mob kills grant it; vendors discount and gate stock from it. The client only renders `TickSnapshot`.

## 2. Ladder (compressed)

Stored as `i32`, Neutral at 0, missing row = Neutral 0.

| Rank | At |
| --- | --- |
| Hated | −4200 |
| Hostile | −3000 |
| Unfriendly | −1500 |
| Neutral | 0 |
| Friendly | 500 |
| Honored | 1500 |
| Revered | 3000 |
| Exalted | 6000 (cap 6299) |

Vendor buy discount: Friendly 5% / Honored 10% / Revered 15% / Exalted 20% (ceiling, never free). Unfriendly and worse refuse trade.

## 3. Factions

`eastbrook_watch`, `eastfen_circle`, `mirefen_ferry`, `highwatch`. NPCs carry `NpcDef.faction`. Mobs carry `MobTemplate.kill_reputation`. Quests carry `QuestReward.reputation`.

Demo: Report to Alden (150) + Wolves (250) + three young-wolf kills (75) = 475 Neutral. Scout the North Road (100) reaches Friendly. Wilkes then sells `watch_signet` and applies the 5% discount.

## 4. Actor model

Per-player `Reputation` column (`values: HashMap<String, i32>`). Insert on `create_player` only. Persist in the completion JSON blob (`reputation` array). No Bevy gameplay components.

## 5. Non-goals

Hostile NPC attack, opposite-faction hate, wall-clock decay, reputation tab beyond the C sheet, protocol rev bump.
