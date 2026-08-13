# 1.0.0 acceptance demo

Manual. Requires a GPU client. CI does not run this.

1. Two clients online on one `woc-server`; both see each other move.
2. Create chars, quit, re-login — gear/quests/talents restored (memory or Postgres).
3. Spend talents, cast 3+ abilities, summon a hunter or warlock pet (T).
4. Travel Eastbrook → Eastfen, die, release, respawn at a graveyard.
5. Party Eastbrook Crypt (trash + boss) or Mirefen Barrow → Need/Greed loot (1/2/3).
6. Bank an item and copper; mail copper; list then buy/cancel on the AH; gather + craft a salve or copper shortsword.
7. Duel a player; honor increments.

Footer reads `WoC-rs 1.2.0 · upstream 0.31.0` (`VERSION.toml`).

Actors live in the sim `World` columns (`AGENTS.md`). The Bevy client only presents snapshots.
