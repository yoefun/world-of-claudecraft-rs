use woc_manufacturing::content::items::ITEM_DEFS;
use woc_manufacturing::content::nodes::node_by_id;
use woc_manufacturing::item::ItemId;
use woc_manufacturing::professions::duration::craft_cast_seconds;
use woc_manufacturing::professions::session::ProfessionSession;
use woc_manufacturing::professions::types::{DenyReason, NodeId, ProfessionId, RecipeId, Vec2};
use woc_manufacturing::rng::XorShift64;
use woc_manufacturing::ticks_from_seconds;

#[derive(Debug, Eq, PartialEq)]
struct ProfessionSnapshot {
    inventory_counts: Vec<(ItemId, u16)>,
    gold_copper: u32,
    skills: Vec<(ProfessionId, u16)>,
    tick: u64,
    last_deny: Option<DenyReason>,
    last_masterwork: Option<RecipeId>,
}

fn play(seed: u64) -> ProfessionSnapshot {
    let mut rng = XorShift64::new(seed);
    let mut s = ProfessionSession::new_eastbrook();
    let node = node_by_id(NodeId(1)).unwrap();
    s.pos = node.pos;
    s.start_gather(NodeId(1)).unwrap();
    s.advance(60);
    s.complete_ready(&mut rng).unwrap();
    while s.inventory.count(ItemId::CopperOre) >= 2 {
        s.start_craft(RecipeId::SmeltCopper, 1).unwrap();
        s.advance(ticks_from_seconds(craft_cast_seconds(0)));
        s.complete_ready(&mut rng).unwrap();
    }
    ProfessionSnapshot {
        inventory_counts: ITEM_DEFS
            .iter()
            .map(|def| (def.id, s.inventory.count(def.id)))
            .collect(),
        gold_copper: s.gold.copper,
        skills: ProfessionId::ALL
            .iter()
            .map(|id| (*id, s.skills.get(*id)))
            .collect(),
        tick: s.tick,
        last_deny: s.last_deny,
        last_masterwork: s.last_masterwork,
    }
}

#[test]
fn same_seed_replays_byte_identical_profession_state() {
    assert_eq!(play(7), play(7));
}

#[test]
fn eastbrook_loop_can_mine_smelt_and_forge_a_sword() {
    let mut rng = XorShift64::new(1);
    let mut s = ProfessionSession::new_eastbrook();
    let node = node_by_id(NodeId(1)).unwrap();

    s.pos = node.pos;
    while s.inventory.count(ItemId::CopperOre) < 6 {
        s.start_gather(NodeId(1)).unwrap();
        s.advance(60);
        s.complete_ready(&mut rng).unwrap();
        let ready = s.node_ready.get(&NodeId(1)).copied().unwrap_or(0);
        if s.tick < ready {
            s.advance((ready - s.tick) as u32);
        }
    }

    s.pos = Vec2 { x: 0.0, z: 0.0 };

    for _ in 0..3 {
        s.start_craft(RecipeId::SmeltCopper, 1).unwrap();
        s.advance(ticks_from_seconds(craft_cast_seconds(0)));
        s.complete_ready(&mut rng).unwrap();
    }
    assert_eq!(s.inventory.count(ItemId::CopperBar), 3);

    s.start_craft(RecipeId::CopperShortsword, 1).unwrap();
    s.advance(ticks_from_seconds(craft_cast_seconds(0)));
    s.complete_ready(&mut rng).unwrap();

    assert_eq!(s.inventory.count(ItemId::CopperShortsword), 1);
    assert_eq!(s.inventory.count(ItemId::SmithingFlux), 0);
}
