//! Party invite / accept / leave (size 2–5).

use std::collections::HashMap;

use crate::ecs::components::{ClassKit, Health, Identity, InstanceAt, Transform};
use crate::ecs::World;
use woc_protocol::{EntityId, EntityKind};

/// Hard cap on party membership (inclusive).
pub const MAX_PARTY_SIZE: usize = 5;
/// Below this size a party dissolves.
pub const MIN_PARTY_SIZE: usize = 2;

#[derive(Debug, Clone)]
pub struct Party {
    pub id: u32,
    pub leader: EntityId,
    pub members: Vec<EntityId>,
}

/// Pending invite + live parties for a realm.
#[derive(Debug, Default, Clone)]
pub struct PartyRoster {
    next_id: u32,
    parties: HashMap<u32, Party>,
    /// player → party id
    membership: HashMap<EntityId, u32>,
    /// invitee → inviter
    pending: HashMap<EntityId, EntityId>,
    /// party_id → loot mode
    loot_modes: HashMap<u32, super::loot::LootMode>,
}

/// Side-effects from a party action (mapped to WsServerMsg by the host).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartyEffect {
    Update {
        members: Vec<EntityId>,
    },
    Error {
        message: String,
    },
    /// Soft notice (toast) for invitee / inviter.
    Notice {
        message: String,
    },
}

impl PartyRoster {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            ..Default::default()
        }
    }

    pub fn party_id(&self, player: EntityId) -> Option<u32> {
        self.membership.get(&player).copied()
    }

    /// Party loot mode string for snapshots (`ffa` | `need_greed`).
    pub fn loot_mode(&self, player: EntityId) -> Option<String> {
        let pid = self.party_id(player)?;
        Some(
            self.loot_modes
                .get(&pid)
                .copied()
                .unwrap_or(super::loot::LootMode::Ffa)
                .as_str()
                .to_string(),
        )
    }

    pub fn set_loot_mode(&mut self, player: EntityId, mode: super::loot::LootMode) -> bool {
        let Some(pid) = self.party_id(player) else {
            return false;
        };
        let Some(party) = self.parties.get(&pid) else {
            return false;
        };
        if party.leader != player {
            return false;
        }
        self.loot_modes.insert(pid, mode);
        true
    }

    pub fn members_of(&self, player: EntityId) -> Option<Vec<EntityId>> {
        let pid = self.party_id(player)?;
        self.parties.get(&pid).map(|p| p.members.clone())
    }

    /// Invite `invitee_name` (exact match on player name).
    pub fn invite(
        &mut self,
        inviter: EntityId,
        invitee_name: &str,
        world: &World,
    ) -> Vec<PartyEffect> {
        if !player_exists(world, inviter) {
            return vec![PartyEffect::Error {
                message: "You are not in the realm.".into(),
            }];
        }
        let Some(invitee) = find_player_by_name(world, invitee_name) else {
            return vec![PartyEffect::Error {
                message: format!("No player named '{invitee_name}'."),
            }];
        };
        if invitee == inviter {
            return vec![PartyEffect::Error {
                message: "You cannot invite yourself.".into(),
            }];
        }
        if self.membership.contains_key(&invitee) {
            return vec![PartyEffect::Error {
                message: format!("{invitee_name} is already in a party."),
            }];
        }
        if let Some(pid) = self.party_id(inviter) {
            if let Some(party) = self.parties.get(&pid) {
                if party.members.len() >= MAX_PARTY_SIZE {
                    return vec![PartyEffect::Error {
                        message: "Your party is full.".into(),
                    }];
                }
            }
        }
        self.pending.insert(invitee, inviter);
        let inviter_name = player_name(world, inviter).unwrap_or_else(|| "Someone".into());
        vec![PartyEffect::Notice {
            message: format!("{inviter_name} invited {invitee_name} to a party."),
        }]
    }

    pub fn accept(&mut self, invitee: EntityId, world: &World) -> Vec<PartyEffect> {
        if !player_exists(world, invitee) {
            return vec![PartyEffect::Error {
                message: "You are not in the realm.".into(),
            }];
        }
        let Some(inviter) = self.pending.remove(&invitee) else {
            return vec![PartyEffect::Error {
                message: "You have no pending party invite.".into(),
            }];
        };
        if !player_exists(world, inviter) {
            return vec![PartyEffect::Error {
                message: "The inviter is no longer online.".into(),
            }];
        }
        if self.membership.contains_key(&invitee) {
            return vec![PartyEffect::Error {
                message: "You are already in a party.".into(),
            }];
        }

        // Join inviter's existing party, or form a new one.
        if let Some(pid) = self.party_id(inviter) {
            let Some(party) = self.parties.get_mut(&pid) else {
                self.membership.remove(&inviter);
                return self.form_new_party(inviter, invitee);
            };
            if party.members.len() >= MAX_PARTY_SIZE {
                return vec![PartyEffect::Error {
                    message: "That party is full.".into(),
                }];
            }
            party.members.push(invitee);
            self.membership.insert(invitee, pid);
            let members = party.members.clone();
            return vec![PartyEffect::Update { members }];
        }

        if self.membership.contains_key(&inviter) {
            return vec![PartyEffect::Error {
                message: "The inviter is already in another party.".into(),
            }];
        }
        self.form_new_party(inviter, invitee)
    }

    fn form_new_party(&mut self, leader: EntityId, member: EntityId) -> Vec<PartyEffect> {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let members = vec![leader, member];
        self.parties.insert(
            id,
            Party {
                id,
                leader,
                members: members.clone(),
            },
        );
        self.membership.insert(leader, id);
        self.membership.insert(member, id);
        vec![PartyEffect::Update { members }]
    }

    pub fn leave(&mut self, player: EntityId) -> Vec<PartyEffect> {
        self.pending.remove(&player);
        let Some(pid) = self.membership.remove(&player) else {
            return vec![PartyEffect::Error {
                message: "You are not in a party.".into(),
            }];
        };
        let Some(mut party) = self.parties.remove(&pid) else {
            return vec![PartyEffect::Update {
                members: Vec::new(),
            }];
        };
        party.members.retain(|m| *m != player);
        if party.members.len() < MIN_PARTY_SIZE {
            for m in &party.members {
                self.membership.remove(m);
            }
            return vec![PartyEffect::Update {
                members: Vec::new(),
            }];
        }
        if party.leader == player {
            party.leader = party.members[0];
        }
        let members = party.members.clone();
        for m in &members {
            self.membership.insert(*m, pid);
        }
        self.parties.insert(pid, party);
        vec![PartyEffect::Update { members }]
    }

    /// Drop party bookkeeping when a player despawns / disconnects.
    pub fn on_despawn(&mut self, player: EntityId) -> Vec<PartyEffect> {
        self.pending
            .retain(|invitee, inviter| *invitee != player && *inviter != player);
        if self.membership.contains_key(&player) {
            return self.leave(player);
        }
        Vec::new()
    }
}

/// Party members within `range` yards of the killer share kill credit / XP.
pub const PARTY_CREDIT_RANGE: f32 = 40.0;

/// Other party members near the killer share kill credit.
pub fn kill_credit_share(
    roster: &PartyRoster,
    world: &World,
    killer: EntityId,
) -> Vec<EntityId> {
    let Some(killer_t) = world.get::<Transform>(killer) else {
        return Vec::new();
    };
    if world.get::<ClassKit>(killer).is_none() {
        return Vec::new();
    }
    let killer_inst = world
        .get::<InstanceAt>(killer)
        .and_then(|i| i.instance_id.clone());
    roster
        .members_of(killer)
        .map(|m| {
            m.into_iter()
                .filter(|id| *id != killer)
                .filter(|id| {
                    world.get::<ClassKit>(*id).is_some()
                        && world
                            .get::<Health>(*id)
                            .map(|h| h.alive)
                            .unwrap_or(false)
                        && world.get::<Transform>(*id).map(|mate| {
                            let dx = mate.x - killer_t.x;
                            let dz = mate.z - killer_t.z;
                            (dx * dx + dz * dz).sqrt() <= PARTY_CREDIT_RANGE
                                && world
                                    .get::<InstanceAt>(*id)
                                    .and_then(|i| i.instance_id.clone())
                                    == killer_inst
                        }).unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn player_exists(world: &World, id: EntityId) -> bool {
    world.get::<ClassKit>(id).is_some()
}

fn find_player_by_name(world: &World, name: &str) -> Option<EntityId> {
    for id in world.ids::<ClassKit>() {
        if world
            .get::<Identity>(id)
            .is_some_and(|i| i.kind == EntityKind::Player && i.name == name)
        {
            return Some(id);
        }
    }
    None
}

fn player_name(world: &World, id: EntityId) -> Option<String> {
    world.get::<Identity>(id).map(|i| i.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::spawn::world_from_entities;
    use crate::entity::create_player;
    use woc_content::PlayerClass;

    fn players() -> Vec<crate::entity::Entity> {
        vec![
            create_player(1, "Alice", PlayerClass::Warrior, 0.0, 0.0),
            create_player(2, "Bob", PlayerClass::Mage, 1.0, 0.0),
            create_player(3, "Carol", PlayerClass::Rogue, 2.0, 0.0),
            create_player(4, "Dave", PlayerClass::Warrior, 3.0, 0.0),
            create_player(5, "Eve", PlayerClass::Mage, 4.0, 0.0),
            create_player(6, "Frank", PlayerClass::Rogue, 5.0, 0.0),
        ]
    }

    fn world_from(entities: &[crate::entity::Entity]) -> World {
        world_from_entities(entities)
    }

    fn form_party(roster: &mut PartyRoster, world: &World, a: EntityId, b: EntityId) {
        let name = world
            .get::<Identity>(b)
            .map(|i| i.name.clone())
            .unwrap();
        let effects = roster.invite(a, &name, world);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, PartyEffect::Notice { .. })),
            "invite should notify: {effects:?}"
        );
        let effects = roster.accept(b, world);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, PartyEffect::Update { members } if members.len() == 2)),
            "accept should form party of 2: {effects:?}"
        );
    }

    #[test]
    fn invite_accept_forms_party_of_two() {
        let entities = players();
        let world = world_from(&entities);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        let members = roster.members_of(1).expect("alice in party");
        assert_eq!(members, vec![1, 2]);
        assert_eq!(roster.party_id(1), roster.party_id(2));
        assert!(roster.party_id(1).is_some());
    }

    #[test]
    fn invite_unknown_name_errors() {
        let entities = players();
        let world = world_from(&entities);
        let mut roster = PartyRoster::new();
        let effects = roster.invite(1, "Nobody", &world);
        assert!(matches!(effects.as_slice(), [PartyEffect::Error { .. }]));
    }

    #[test]
    fn leave_dissolves_pair() {
        let entities = players();
        let world = world_from(&entities);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        let effects = roster.leave(2);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, PartyEffect::Update { members } if members.is_empty())),
            "leave should dissolve: {effects:?}"
        );
        assert!(roster.party_id(1).is_none());
        assert!(roster.party_id(2).is_none());
    }

    #[test]
    fn party_grows_to_five_then_rejects_sixth() {
        let entities = players();
        let world = world_from(&entities);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        for invitee in [3u32, 4, 5] {
            let name = world
                .get::<Identity>(invitee)
                .unwrap()
                .name
                .clone();
            roster.invite(1, &name, &world);
            let effects = roster.accept(invitee, &world);
            assert!(
                effects
                    .iter()
                    .any(|e| matches!(e, PartyEffect::Update { .. })),
                "join {invitee}: {effects:?}"
            );
        }
        assert_eq!(roster.members_of(1).unwrap().len(), MAX_PARTY_SIZE);
        let effects = roster.invite(1, "Frank", &world);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, PartyEffect::Error { .. })),
            "sixth invite should fail: {effects:?}"
        );
        assert_eq!(roster.members_of(1).unwrap().len(), MAX_PARTY_SIZE);
    }

    #[test]
    fn kill_credit_share_lists_party_mates() {
        let entities = players();
        let world = world_from(&entities);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        let share = kill_credit_share(&roster, &world, 1);
        assert_eq!(share, vec![2]);
        assert!(kill_credit_share(&roster, &world, 99).is_empty());
    }

    #[test]
    fn accept_without_invite_errors() {
        let entities = players();
        let world = world_from(&entities);
        let mut roster = PartyRoster::new();
        let effects = roster.accept(2, &world);
        assert!(matches!(effects.as_slice(), [PartyEffect::Error { .. }]));
    }
}
