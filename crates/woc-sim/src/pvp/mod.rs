//! Deterministic duel and open-world PvP state.
//!
//! The host owns one [`PvpState`]. Route duel interactions through
//! [`challenge_duel`], [`accept_duel`], and [`toggle_pvp`], then call
//! [`tick_pvp`] once after combat. This keeps PvP bookkeeping additive and
//! does not introduce a new simulation tick phase.

use crate::ecs::components::{Auras, ClassKit, Combat, Health, Progress, Spirit, Transform};
use crate::ecs::World;
use crate::types::INTERACT_RANGE;
use woc_protocol::{EntityId, SimEvent};

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
    world: &World,
    challenger: EntityId,
    challenged: EntityId,
) -> Result<(), PvpError> {
    validate_pair(world, challenger, challenged)?;
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
    world: &World,
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
    accept_duel(state, world, challenged, challenger, events)
}

/// Accept the challenge from `challenger` and begin a duel.
pub fn accept_duel(
    state: &mut PvpState,
    world: &World,
    challenged: EntityId,
    challenger: EntityId,
    events: &mut Vec<SimEvent>,
) -> Result<(), PvpError> {
    validate_pair(world, challenger, challenged)?;
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
pub fn toggle_pvp(world: &mut World, player_id: EntityId) -> Result<bool, PvpError> {
    if world.get::<ClassKit>(player_id).is_none() {
        return Err(PvpError::PlayerNotFound);
    }
    let Some(progress) = world.get_mut::<Progress>(player_id) else {
        return Err(PvpError::NotAPlayer);
    };
    progress.pvp_flagged = !progress.pvp_flagged;
    Ok(progress.pvp_flagged)
}

/// Resolve duels at 1 HP and award honor for duel wins or flagged-player kills.
///
/// Call once after combat damage and kill events, before player death is
/// finalized. Death bookkeeping is also cleared defensively during restore.
pub fn tick_pvp(state: &mut PvpState, world: &mut World, events: &mut Vec<SimEvent>) {
    let mut still_active = Vec::with_capacity(state.active.len());
    let mut resolved_duels = Vec::new();

    for duel in std::mem::take(&mut state.active) {
        let Some(a) = world.get::<Health>(duel.challenger) else {
            continue;
        };
        let Some(b) = world.get::<Health>(duel.challenged) else {
            continue;
        };
        if world.get::<ClassKit>(duel.challenger).is_none()
            || world.get::<ClassKit>(duel.challenged).is_none()
        {
            continue;
        }
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
        restore_duel_player(world, duel.challenger);
        restore_duel_player(world, duel.challenged);
        events.push(SimEvent::DuelEnded { winner, loser });
        award_honor(world, winner, events);
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
            let killer_is_player = world.get::<ClassKit>(*killer).is_some();
            let victim_is_flagged = world
                .get::<ClassKit>(*victim)
                .and_then(|_| world.get::<Progress>(*victim))
                .map(|p| p.pvp_flagged)
                .unwrap_or(false);
            killer_is_player && victim_is_flagged
        })
        .collect();

    for (killer, _) in flagged_kills {
        award_honor(world, killer, events);
    }
}

fn validate_pair(
    world: &World,
    challenger: EntityId,
    challenged: EntityId,
) -> Result<(), PvpError> {
    if challenger == challenged {
        return Err(PvpError::CannotChallengeSelf);
    }
    if world.get::<ClassKit>(challenger).is_none() {
        return Err(PvpError::PlayerNotFound);
    }
    if world.get::<ClassKit>(challenged).is_none() {
        return Err(PvpError::TargetNotFound);
    }
    let a_alive = world
        .get::<Health>(challenger)
        .map(|h| h.alive)
        .unwrap_or(false);
    let b_alive = world
        .get::<Health>(challenged)
        .map(|h| h.alive)
        .unwrap_or(false);
    if !a_alive || !b_alive {
        return Err(PvpError::PlayerDead);
    }
    let (Some(a), Some(b)) = (
        world.get::<Transform>(challenger),
        world.get::<Transform>(challenged),
    ) else {
        return Err(PvpError::OutOfRange);
    };
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    if dx * dx + dz * dz > DUEL_RANGE * DUEL_RANGE {
        return Err(PvpError::OutOfRange);
    }
    Ok(())
}

fn restore_duel_player(world: &mut World, player_id: EntityId) {
    if let Some(h) = world.get_mut::<Health>(player_id) {
        h.hp = h.hp_max;
        h.alive = true;
    }
    if let Some(c) = world.get_mut::<Combat>(player_id) {
        c.auto_attack = false;
        c.target = None;
        c.swing_timer = 0.0;
        c.cast = None;
    }
    if let Some(a) = world.get_mut::<Auras>(player_id) {
        a.auras.clear();
    }
    if let Some(s) = world.get_mut::<Spirit>(player_id) {
        s.corpse_x = None;
        s.corpse_z = None;
    }
}

fn award_honor(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) {
    if world.get::<ClassKit>(player_id).is_none() {
        return;
    }
    let Some(progress) = world.get_mut::<Progress>(player_id) else {
        return;
    };
    progress.honor = progress.honor.saturating_add(HONOR_PER_KILL);
    events.push(SimEvent::HonorGained {
        player: player_id,
        amount: HONOR_PER_KILL,
    });
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::Progress;
    use woc_content::PlayerClass;

    fn players_in_range() -> World {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Alice", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 2.0, 0.0);
        world
    }

    #[test]
    fn challenge_and_accept_starts_duel_in_range() {
        let world = players_in_range();
        let mut pvp = PvpState::default();
        let mut events = Vec::new();
        assert_eq!(challenge_duel(&mut pvp, &world, 1, 2), Ok(()));
        assert_eq!(accept_duel(&mut pvp, &world, 2, 1, &mut events), Ok(()));
        assert!(pvp.is_dueling(1, 2));
        assert!(events
            .iter()
            .any(|event| matches!(event, SimEvent::DuelStarted { a: 1, b: 2 })));
    }

    #[test]
    fn duel_ends_at_one_hp_restores_players_and_grants_honor() {
        let mut world = players_in_range();
        let mut pvp = PvpState::default();
        let mut events = Vec::new();
        challenge_duel(&mut pvp, &world, 1, 2).unwrap();
        accept_duel(&mut pvp, &world, 2, 1, &mut events).unwrap();
        events.clear();
        if let Some(h) = world.get_mut::<Health>(1) {
            h.hp = h.hp_max - 10.0;
        }
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = 1.0;
        }
        tick_pvp(&mut pvp, &mut world, &mut events);
        assert!(!pvp.is_dueling(1, 2));
        assert_eq!(world.get::<Progress>(1).unwrap().honor, HONOR_PER_KILL);
        assert!(world.get::<Health>(1).unwrap().hp > 1.0);
        assert!(world.get::<Health>(2).unwrap().hp > 1.0);
    }

    #[test]
    fn toggle_pvp_flips_flag() {
        let mut world = players_in_range();
        assert_eq!(toggle_pvp(&mut world, 1), Ok(true));
        assert!(world.get::<Progress>(1).unwrap().pvp_flagged);
        assert_eq!(toggle_pvp(&mut world, 1), Ok(false));
        assert!(!world.get::<Progress>(1).unwrap().pvp_flagged);
    }
}
