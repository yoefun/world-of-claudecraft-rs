# 1.0.0 acceptance demo

Manual. Requires a GPU client. CI does not run this.

1. Two clients online on one `woc-server`; both see each other move.
2. Create chars, quit, re-login — same HP/bag/position (park/resume); gear/quests/talents restored (memory or Postgres).
3. Spend talents, cast 3+ abilities, summon a hunter or warlock pet (T). Rogue: **Z** stealth → Cheap Shot opener → Sinister Strike → Eviscerate. Druid/shaman **F** Travel Form / Ghost Wolf; warlock Fear; warrior **F** stance.
4. Travel Eastbrook → Eastfen, die, release, respawn at a graveyard.
5. Party Eastbrook Crypt (trash + boss) or Mirefen Barrow → Need/Greed loot (1/2/3).
6. Bank an item and copper; mail copper; list then buy/cancel on the AH; gather + craft a salve or copper shortsword.
7. Duel a player; honor increments.
8. Warrior spawn shows a full cloth extra set; mage cannot equip a sword; crypt warden drops crypt_cleaver; C-sheet AP/Armor/SP update on equip.
9. Town Crier **E** → accept Report to Alden → **E** Captain Alden (talk) → **E** turn-in → **E** accept Wolves → kill 3 → ready toast → **E** turn-in → **E** accept Boar Tusks.
10. Quest log **L**: **X** abandons the tracked quest; **Y** shares it with in-range party mates. After wolves, Alden offers the daily Wolf Patrol (resets every 12_000 ticks). Walk the north road for Scout the North Road. After tusks, escort the courier. Trader Wilkes: Arms of the Watch → **1/2/3** choose ration.
11. Buy a ration from Wilkes; sell a fang and buy it back; boar tusk will not sell.
12. Wear down the starter sword on wolves; repair at Smith Brann.
13. Train Mining at Brann, Herbalism at Wren.
14. Bind at Innkeeper Mara, run to Wolf Run, press H.
15. Rogue: equip a second dagger into OffHand; wear two rings; buy a Coarse Whetstone from Smith Brann, use it, see AP rise and `[Coarse Sharpening]` on the C-sheet.
16. After Report + Wolves + Scout, Eastbrook Watch is Friendly; Wilkes sells Watch Signet (**C** sheet lists standing).

Footer reads `WoC-rs 1.14.0 · upstream 0.31.0` (`VERSION.toml`). Nine-class signatures: stealth, shield, Charge, Blink, Aspect, Devotion/seal, Lightning Shield, Fear, Travel Form.

Online path: title **2 Online** must show `Online: compatible` before Continue proceeds to Login.

Actors live in the sim `World` columns (`AGENTS.md`). The Bevy client only presents snapshots.
