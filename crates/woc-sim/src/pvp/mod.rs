//! Deterministic duel and open-world PvP state.
//!
//! The host owns one [`PvpState`]. Route duel interactions through
//! [`challenge_duel`], [`accept_duel`], and [`toggle_pvp`], then call
//! [`tick_pvp`] once after combat. This keeps PvP bookkeeping additive and
//! does not introduce a new simulation tick phase.

use crate::entity::Entity;
use crate::types::INTERACT_RANGE;
use woc_protocol::{EntityId, EntityKind, SimEvent};

/// Players must remain within normal interaction range to start a duel.
pub const DUEL_RANGE: f32 = INTERACT_RANGE;
/// Honor awarded for a duel victory or flagged-player kill.
pub const HONOR_PER_KILL: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PvpError {
    PlayerNotFound,
    TargetNotFound,
    NotAPlayer,
    PlayerDead,
    CannotChallengeSelf,
    OutOfRange,
    AlreadyDueling,
    ChallengePending,
    NoPendingChallenge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DuelPair {
    challenger: EntityId,
    challenged: EntityId,
}

/// Pending challenges and active duels for one simulation realm.
#[derive(Debug, Clone, Default)]
pub struct PvpState {
    pending: Vec<DuelPair>,
    active: Vec<DuelPair>,
}

impl PvpState {
    pub fn is_dueling(&self, a: EntityId, b: EntityId) -> bool {
        self.active.iter().any(|duel| duel.matches(a, b))
    }
}

impl DuelPair {
    fn matches(self, a: EntityId, b: EntityId) -> bool {
        (self.challenger == a && self.challenged == b)
            || (self.challenger == b && self.challenged == a)
    }

    fn contains(self, player: EntityId) -> bool {
        self.challenger == player || self.challenged == player
    }
}

/// Record a challenge from `challenger` to `challenged`.
pub fn challenge_duel(
    state: &mut PvpState,
    entities: &[Entity],
    challenger: EntityId,
    challenged: EntityId,
) -> Result<(), PvpError> {
    validate_pair(entities, challenger, challenged)?;
    if state
        .active
        .iter()
        .any(|duel| duel.contains(challenger) || duel.contains(challenged))
    {
        return Err(PvpError::AlreadyDueling);
    }
    if state
        .pending
        .iter()
        .any(|duel| duel.contains(challenger) || duel.contains(challenged))
    {
        return Err(PvpError::ChallengePending);
    }
    state.pending.push(DuelPair {
        challenger,
        challenged,
    });
    Ok(())
}

/// Accept the outstanding challenge targeting `challenged`.
pub fn accept_pending_duel(
    state: &mut PvpState,
    entities: &[Entity],
    challenged: EntityId,
    events: &mut Vec<SimEvent>,
) -> Result<(), PvpError> {
    let Some(challenger) = state
        .pending
        .iter()
        .find(|duel| duel.challenged == challenged)
        .map(|duel| duel.challenger)
    else {
        return Err(PvpError::NoPendingChallenge);
    };
    accept_duel(state, entities, challenged, challenger, events)
}

/// Accept the challenge from `challenger` and begin a duel.
pub fn accept_duel(
    state: &mut PvpState,
    entities: &[Entity],
    challenged: EntityId,
    challenger: EntityId,
    events: &mut Vec<SimEvent>,
) -> Result<(), PvpError> {
    validate_pair(entities, challenger, challenged)?;
    if state
        .active
        .iter()
        .any(|duel| duel.contains(challenger) || duel.contains(challenged))
    {
        return Err(PvpError::AlreadyDueling);
    }
    let Some(index) = state
        .pending
        .iter()
        .position(|duel| duel.challenger == challenger && duel.challenged == challenged)
    else {
        return Err(PvpError::NoPendingChallenge);
    };
    let duel = state.pending.remove(index);
    state.active.push(duel);
    events.push(SimEvent::DuelStarted {
        a: challenger,
        b: challenged,
    });
    Ok(())
}

/// Toggle a player's open-world PvP flag and return its new value.
pub fn toggle_pvp(entities: &mut [Entity], player_id: EntityId) -> Result<bool, PvpError> {
    let Some(player) = entities.iter_mut().find(|entity| entity.id == player_id) else {
        return Err(PvpError::PlayerNotFound);
    };
    if player.kind != EntityKind::Player {
        return Err(PvpError::NotAPlayer);
    }
    player.pvp_flagged = !player.pvp_flagged;
    Ok(player.pvp_flagged)
}

/// Resolve duels at 1 HP and award honor for duel wins or flagged-player kills.
///
/// Call once after combat damage and kill events, before player death is
/// finalized. Death bookkeeping is also cleared defensively during restore.
pub fn tick_pvp(state: &mut PvpState, entities: &mut [Entity], events: &mut Vec<SimEvent>) {
    let mut still_active = Vec::with_capacity(state.active.len());
    let mut resolved_duels = Vec::new();

    for duel in std::mem::take(&mut state.active) {
        let Some(a) = entities
            .iter()
            .find(|entity| entity.id == duel.challenger && entity.kind == EntityKind::Player)
        else {
            continue;
        };
        let Some(b) = entities
            .iter()
            .find(|entity| entity.id == duel.challenged && entity.kind == EntityKind::Player)
        else {
            continue;
        };
        let a_defeated = !a.alive || a.hp <= 1.0;
        let b_defeated = !b.alive || b.hp <= 1.0;
        if !a_defeated && !b_defeated {
            still_active.push(duel);
            continue;
        }
        let (winner, loser) = if a_defeated {
            (duel.challenged, duel.challenger)
        } else {
            (duel.challenger, duel.challenged)
        };
        restore_duel_player(entities, duel.challenger);
        restore_duel_player(entities, duel.challenged);
        events.push(SimEvent::DuelEnded { winner, loser });
        award_honor(entities, winner, events);
        resolved_duels.push((winner, loser));
    }
    state.active = still_active;

    let flagged_kills: Vec<(EntityId, EntityId)> = events
        .iter()
        .filter_map(|event| match event {
            SimEvent::Kill { killer, victim, .. } => Some((*killer, *victim)),
            _ => None,
        })
        .filter(|(killer, victim)| {
            killer != victim
                && !resolved_duels
                    .iter()
                    .any(|pair| pair == &(*killer, *victim))
        })
        .filter(|(killer, victim)| {
            let killer_is_player = entities
                .iter()
                .any(|entity| entity.id == *killer && entity.kind == EntityKind::Player);
            let victim_is_flagged = entities.iter().any(|entity| {
                entity.id == *victim && entity.kind == EntityKind::Player && entity.pvp_flagged
            });
            killer_is_player && victim_is_flagged
        })
        .collect();

    for (killer, _) in flagged_kills {
        award_honor(entities, killer, events);
    }
}

fn validate_pair(
    entities: &[Entity],
    challenger: EntityId,
    challenged: EntityId,
) -> Result<(), PvpError> {
    if challenger == challenged {
        return Err(PvpError::CannotChallengeSelf);
    }
    let Some(a) = entities.iter().find(|entity| entity.id == challenger) else {
        return Err(PvpError::PlayerNotFound);
    };
    let Some(b) = entities.iter().find(|entity| entity.id == challenged) else {
        return Err(PvpError::TargetNotFound);
    };
    if a.kind != EntityKind::Player || b.kind != EntityKind::Player {
        return Err(PvpError::NotAPlayer);
    }
    if !a.alive || !b.alive {
        return Err(PvpError::PlayerDead);
    }
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    if dx * dx + dz * dz > DUEL_RANGE * DUEL_RANGE {
        return Err(PvpError::OutOfRange);
    }
    Ok(())
}

fn restore_duel_player(entities: &mut [Entity], player_id: EntityId) {
    let Some(player) = entities.iter_mut().find(|entity| entity.id == player_id) else {
        return;
    };
    player.hp = player.hp_max;
    player.alive = true;
    player.auto_attack = false;
    player.target = None;
    player.swing_timer = 0.0;
    player.cast = None;
    player.auras.clear();
    player.corpse_x = None;
    player.corpse_z = None;
}

fn award_honor(entities: &mut [Entity], player_id: EntityId, events: &mut Vec<SimEvent>) {
    let Some(player) = entities
        .iter_mut()
        .find(|entity| entity.id == player_id && entity.kind == EntityKind::Player)
    else {
        return;
    };
    player.honor = player.honor.saturating_add(HONOR_PER_KILL);
    events.push(SimEvent::HonorGained {
        player: player_id,
        amount: HONOR_PER_KILL,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::create_player;
    use woc_content::PlayerClass;

    fn players_in_range() -> Vec<Entity> {
        vec![
            create_player(1, "Alice", PlayerClass::Warrior, 0.0, 0.0),
            create_player(2, "Bob", PlayerClass::Mage, 2.0, 0.0),
        ]
    }

    #[test]
    fn challenge_and_accept_starts_duel_in_range() {
        let entities = players_in_range();
        let mut pvp = PvpState::default();
        let mut events = Vec::new();

        assert_eq!(challenge_duel(&mut pvp, &entities, 1, 2), Ok(()));
        assert_eq!(accept_duel(&mut pvp, &entities, 2, 1, &mut events), Ok(()));

        assert!(pvp.is_dueling(1, 2));
        assert!(events
            .iter()
            .any(|event| matches!(event, SimEvent::DuelStarted { a: 1, b: 2 })));
    }

    #[test]
    fn duel_ends_at_one_hp_restores_players_and_grants_honor() {
        let mut entities = players_in_range();
        let mut pvp = PvpState::default();
        let mut events = Vec::new();
        challenge_duel(&mut pvp, &entities, 1, 2).unwrap();
        accept_duel(&mut pvp, &entities, 2, 1, &mut events).unwrap();
        events.clear();
        entities[0].hp = entities[0].hp_max - 10.0;
        entities[1].hp = 1.0;

        tick_pvp(&mut pvp, &mut entities, &mut events);

        assert!(!pvp.is_dueling(1, 2));
        assert_eq!(entities[0].hp, entities[0].hp_max);
        assert_eq!(entities[1].hp, entities[1].hp_max);
        assert!(entities[0].alive && entities[1].alive);
        assert_eq!(entities[0].honor, HONOR_PER_KILL);
        assert!(events.iter().any(|event| matches!(
            event,
            SimEvent::DuelEnded {
                winner: 1,
                loser: 2
            }
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            SimEvent::HonorGained {
                player: 1,
                amount: HONOR_PER_KILL
            }
        )));
    }

    #[test]
    fn duel_death_restores_loser_without_death_state() {
        let mut entities = players_in_range();
        let mut pvp = PvpState::default();
        let mut events = Vec::new();
        challenge_duel(&mut pvp, &entities, 1, 2).unwrap();
        accept_duel(&mut pvp, &entities, 2, 1, &mut events).unwrap();
        events.clear();
        entities[1].hp = 0.0;
        entities[1].alive = false;
        entities[1].corpse_x = Some(entities[1].x);
        entities[1].corpse_z = Some(entities[1].z);

        tick_pvp(&mut pvp, &mut entities, &mut events);

        assert!(entities[1].alive);
        assert_eq!(entities[1].hp, entities[1].hp_max);
        assert_eq!(entities[1].corpse_x, None);
        assert_eq!(entities[1].corpse_z, None);
        assert!(events.iter().any(|event| matches!(
            event,
            SimEvent::DuelEnded {
                winner: 1,
                loser: 2
            }
        )));
    }

    #[test]
    fn killing_flagged_player_grants_honor() {
        let mut entities = players_in_range();
        entities[1].pvp_flagged = true;
        entities[1].hp = 0.0;
        entities[1].alive = false;
        let mut events = vec![SimEvent::Kill {
            killer: 1,
            victim: 2,
            victim_name: "Bob".into(),
        }];

        tick_pvp(&mut PvpState::default(), &mut entities, &mut events);

        assert_eq!(entities[0].honor, HONOR_PER_KILL);
        assert!(events.iter().any(|event| matches!(
            event,
            SimEvent::HonorGained {
                player: 1,
                amount: HONOR_PER_KILL
            }
        )));
    }

    #[test]
    fn toggle_pvp_updates_player_flag() {
        let mut entities = players_in_range();

        assert_eq!(toggle_pvp(&mut entities, 1), Ok(true));
        assert!(entities[0].pvp_flagged);
        assert_eq!(toggle_pvp(&mut entities, 1), Ok(false));
        assert!(!entities[0].pvp_flagged);
    }
}
