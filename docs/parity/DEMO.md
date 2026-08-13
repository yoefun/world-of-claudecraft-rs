# 1.0.0 acceptance demo

Manual. Requires a GPU client. CI does not run this.

1. Two clients online on one `woc-server`; both see each other move.
2. Create chars, quit, re-login — gear/quests/talents restored (memory or Postgres).
3. Spend talents, cast 3+ abilities, summon a hunter or warlock pet (T).
4. Travel Eastbrook → Eastfen, die, release, respawn at a graveyard.
5. Party a dungeon boss (Eastbrook Crypt) → Need/Greed loot (1/2/3).
6. Bank an item and copper; mail copper; list then buy/cancel on the AH; gather + craft one salve.
7. Duel a player; honor increments.

Footer reads `WoC-rs 1.0.0-pre · upstream 0.31.0` until the stable tag; after `1.0.0` / `1.1.0` bumps, match `VERSION.toml`.

Actors live in the sim `World` columns (`AGENTS.md`). The Bevy client only presents snapshots.
