//! Party invite / accept / leave (size 2–5).

use std::collections::HashMap;

use crate::ecs::components::{ClassKit, Health, Identity, InstanceAt, Transform};
use crate::ecs::World;
use woc_protocol::{EntityId, EntityKind};

/// Hard cap on party membership (inclusive).
pub const MAX_PARTY_SIZE: usize = 5;
/// Below this size a party dissolves.
pub const MIN_PARTY_SIZE: usize = 2;
/// Invite lifetime in sim ticks (30 s at 20 Hz).
pub const INVITE_TTL_TICKS: u64 = 600;
/// Ready-check lifetime in sim ticks (15 s at 20 Hz).
pub const READY_CHECK_TTL_TICKS: u64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKind {
    Party,
    Raid,
}

#[derive(Debug, Clone)]
pub struct PendingInvite {
    pub inviter: EntityId,
    pub expires_tick: u64,
}

#[derive(Debug, Clone)]
pub struct ReadyCheck {
    pub party_id: u32,
    pub expires_tick: u64,
    pub responses: HashMap<EntityId, bool>,
}

#[derive(Debug, Clone)]
pub struct Party {
    pub id: u32,
    pub leader: EntityId,
    pub members: Vec<EntityId>,
    pub kind: GroupKind,
    pub raid_groups: [Vec<EntityId>; 2],
}

/// Pending invite + live parties for a realm.
#[derive(Debug, Default, Clone)]
pub struct PartyRoster {
    next_id: u32,
    parties: HashMap<u32, Party>,
    /// player → party id
    membership: HashMap<EntityId, u32>,
    /// invitee → pending invite
    pending: HashMap<EntityId, PendingInvite>,
    /// party_id → loot mode
    loot_modes: HashMap<u32, super::loot::LootMode>,
    ready: Option<ReadyCheck>,
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
        now_tick: u64,
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
        self.pending.insert(
            invitee,
            PendingInvite {
                inviter,
                expires_tick: now_tick.saturating_add(INVITE_TTL_TICKS),
            },
        );
        let inviter_name = player_name(world, inviter).unwrap_or_else(|| "Someone".into());
        vec![PartyEffect::Notice {
            message: format!("{inviter_name} invited {invitee_name} to a party."),
        }]
    }

    pub fn expire_invites(&mut self, now_tick: u64) {
        self.pending.retain(|_, p| p.expires_tick > now_tick);
    }

    pub fn decline(&mut self, invitee: EntityId, world: &World) -> Vec<PartyEffect> {
        let Some(_pending) = self.pending.remove(&invitee) else {
            return vec![PartyEffect::Error {
                message: "You have no pending party invite.".into(),
            }];
        };
        let name = player_name(world, invitee).unwrap_or_else(|| "Someone".into());
        vec![PartyEffect::Notice {
            message: format!("{name} declined the invite."),
        }]
    }

    pub fn accept(&mut self, invitee: EntityId, world: &World) -> Vec<PartyEffect> {
        if !player_exists(world, invitee) {
            return vec![PartyEffect::Error {
                message: "You are not in the realm.".into(),
            }];
        }
        let Some(pending) = self.pending.remove(&invitee) else {
            return vec![PartyEffect::Error {
                message: "You have no pending party invite.".into(),
            }];
        };
        let inviter = pending.inviter;
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
                kind: GroupKind::Party,
                raid_groups: [Vec::new(), Vec::new()],
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
            .retain(|invitee, p| *invitee != player && p.inviter != player);
        if self.membership.contains_key(&player) {
            return self.leave(player);
        }
        Vec::new()
    }

    pub fn leader_of(&self, player: EntityId) -> Option<EntityId> {
        let pid = self.party_id(player)?;
        self.parties.get(&pid).map(|p| p.leader)
    }

    pub fn kind_of(&self, player: EntityId) -> Option<GroupKind> {
        let pid = self.party_id(player)?;
        self.parties.get(&pid).map(|p| p.kind)
    }

    pub fn pending_inviter_name(&self, invitee: EntityId, world: &World) -> String {
        let Some(p) = self.pending.get(&invitee) else {
            return String::new();
        };
        player_name(world, p.inviter).unwrap_or_default()
    }

    pub fn raid_group_of(&self, player: EntityId) -> u8 {
        let Some(pid) = self.party_id(player) else {
            return 0;
        };
        let Some(party) = self.parties.get(&pid) else {
            return 0;
        };
        if party.kind != GroupKind::Raid {
            return 0;
        }
        if party.raid_groups[1].contains(&player) {
            1
        } else {
            0
        }
    }

    pub fn ready_snapshot(
        &self,
        player: EntityId,
        _now_tick: u64,
    ) -> Option<woc_protocol::ReadyCheckSnapshot> {
        let check = self.ready.as_ref()?;
        let pid = self.party_id(player)?;
        if check.party_id != pid {
            return None;
        }
        let total = self.members_of(player).map(|m| m.len() as u32).unwrap_or(0);
        let ready_count = check.responses.values().filter(|v| **v).count() as u32;
        Some(woc_protocol::ReadyCheckSnapshot {
            expires_tick: check.expires_tick,
            you_responded: check.responses.contains_key(&player),
            ready_count,
            total,
        })
    }

    pub fn kick(&mut self, leader: EntityId, name: &str, world: &World) -> Vec<PartyEffect> {
        if self.leader_of(leader) != Some(leader) {
            return vec![PartyEffect::Error {
                message: "You are not the party leader.".into(),
            }];
        }
        let Some(target) = find_player_by_name(world, name) else {
            return vec![PartyEffect::Error {
                message: format!("No player named '{name}'."),
            }];
        };
        if target == leader {
            return vec![PartyEffect::Error {
                message: "You cannot kick yourself.".into(),
            }];
        }
        if self.party_id(target) != self.party_id(leader) {
            return vec![PartyEffect::Error {
                message: "That player is not in your party.".into(),
            }];
        }
        let mut effects = self.leave(target);
        effects.insert(
            0,
            PartyEffect::Notice {
                message: format!("{name} was removed from the party."),
            },
        );
        effects
    }

    pub fn promote(&mut self, leader: EntityId, name: &str, world: &World) -> Vec<PartyEffect> {
        if self.leader_of(leader) != Some(leader) {
            return vec![PartyEffect::Error {
                message: "You are not the party leader.".into(),
            }];
        }
        let Some(target) = find_player_by_name(world, name) else {
            return vec![PartyEffect::Error {
                message: format!("No player named '{name}'."),
            }];
        };
        if self.party_id(target) != self.party_id(leader) {
            return vec![PartyEffect::Error {
                message: "That player is not in your party.".into(),
            }];
        }
        let pid = self.party_id(leader).unwrap();
        if let Some(party) = self.parties.get_mut(&pid) {
            party.leader = target;
        }
        vec![PartyEffect::Notice {
            message: format!("{name} is now the leader."),
        }]
    }

    pub fn disband(&mut self, leader: EntityId) -> Vec<PartyEffect> {
        if self.leader_of(leader) != Some(leader) {
            return vec![PartyEffect::Error {
                message: "You are not the party leader.".into(),
            }];
        }
        let Some(pid) = self.party_id(leader) else {
            return vec![PartyEffect::Error {
                message: "You are not in a party.".into(),
            }];
        };
        if let Some(party) = self.parties.remove(&pid) {
            for m in &party.members {
                self.membership.remove(m);
            }
            self.pending.retain(|invitee, p| {
                !party.members.contains(invitee) && !party.members.contains(&p.inviter)
            });
        }
        self.loot_modes.remove(&pid);
        if self.ready.as_ref().is_some_and(|r| r.party_id == pid) {
            self.ready = None;
        }
        vec![PartyEffect::Update {
            members: Vec::new(),
        }]
    }

    pub fn ready_check(&mut self, leader: EntityId, now_tick: u64) -> Vec<PartyEffect> {
        if self.leader_of(leader) != Some(leader) {
            return vec![PartyEffect::Error {
                message: "You are not the party leader.".into(),
            }];
        }
        let Some(pid) = self.party_id(leader) else {
            return vec![PartyEffect::Error {
                message: "You are not in a party.".into(),
            }];
        };
        if self.ready.is_some() {
            return vec![PartyEffect::Error {
                message: "A ready check is already running.".into(),
            }];
        }
        self.ready = Some(ReadyCheck {
            party_id: pid,
            expires_tick: now_tick.saturating_add(READY_CHECK_TTL_TICKS),
            responses: HashMap::new(),
        });
        vec![PartyEffect::Notice {
            message: "Ready check started.".into(),
        }]
    }

    pub fn ready_respond(
        &mut self,
        player: EntityId,
        ready: bool,
        world: &World,
        connected: &[EntityId],
    ) -> Vec<PartyEffect> {
        let Some(pid) = self.party_id(player) else {
            return vec![PartyEffect::Error {
                message: "You are not in a party.".into(),
            }];
        };
        {
            let Some(check) = self.ready.as_mut() else {
                return vec![PartyEffect::Error {
                    message: "There is no ready check.".into(),
                }];
            };
            if check.party_id != pid {
                return vec![PartyEffect::Error {
                    message: "There is no ready check.".into(),
                }];
            }
            check.responses.insert(player, ready);
        }
        let members = self.members_of(player).unwrap_or_default();
        let waiting = self.ready.as_ref().is_some_and(|check| {
            connected
                .iter()
                .any(|m| members.contains(m) && !check.responses.contains_key(m))
        });
        if !waiting {
            return self.finish_ready_check(world);
        }
        Vec::new()
    }

    pub fn expire_ready_check(&mut self, now_tick: u64, world: &World) -> Vec<PartyEffect> {
        let expired = self
            .ready
            .as_ref()
            .is_some_and(|check| check.expires_tick <= now_tick);
        if !expired {
            return Vec::new();
        }
        self.finish_ready_check(world)
    }

    fn finish_ready_check(&mut self, world: &World) -> Vec<PartyEffect> {
        let Some(check) = self.ready.take() else {
            return Vec::new();
        };
        let Some(party) = self.parties.get(&check.party_id) else {
            return Vec::new();
        };
        let mut yes: Vec<EntityId> = Vec::new();
        let mut no: Vec<EntityId> = Vec::new();
        for m in &party.members {
            if check.responses.get(m).copied().unwrap_or(false) {
                yes.push(*m);
            } else {
                no.push(*m);
            }
        }
        if no.is_empty() {
            return vec![PartyEffect::Notice {
                message: "Everyone is ready.".into(),
            }];
        }
        vec![PartyEffect::Notice {
            message: format!(
                "Ready: {}. Not ready: {}.",
                ready_names(world, &yes),
                ready_names(world, &no)
            ),
        }]
    }

    pub fn convert_to_raid(&mut self, leader: EntityId) -> Vec<PartyEffect> {
        if self.leader_of(leader) != Some(leader) {
            return vec![PartyEffect::Error {
                message: "You are not the party leader.".into(),
            }];
        }
        let Some(pid) = self.party_id(leader) else {
            return vec![PartyEffect::Error {
                message: "You are not in a party.".into(),
            }];
        };
        let Some(party) = self.parties.get_mut(&pid) else {
            return vec![PartyEffect::Error {
                message: "You are not in a party.".into(),
            }];
        };
        if party.kind == GroupKind::Raid {
            return vec![PartyEffect::Error {
                message: "Already a raid.".into(),
            }];
        }
        if party.members.len() != MAX_PARTY_SIZE {
            return vec![PartyEffect::Error {
                message: "You need a full party of 5 to convert to a raid.".into(),
            }];
        }
        party.kind = GroupKind::Raid;
        party.raid_groups[0] = party.members.clone();
        party.raid_groups[1].clear();
        vec![PartyEffect::Notice {
            message: "Converted to a raid.".into(),
        }]
    }

    pub fn convert_to_party(&mut self, leader: EntityId) -> Vec<PartyEffect> {
        if self.leader_of(leader) != Some(leader) {
            return vec![PartyEffect::Error {
                message: "You are not the party leader.".into(),
            }];
        }
        let Some(pid) = self.party_id(leader) else {
            return vec![PartyEffect::Error {
                message: "You are not in a raid.".into(),
            }];
        };
        let Some(party) = self.parties.get_mut(&pid) else {
            return vec![PartyEffect::Error {
                message: "You are not in a raid.".into(),
            }];
        };
        if party.kind != GroupKind::Raid {
            return vec![PartyEffect::Error {
                message: "You are not in a raid.".into(),
            }];
        }
        if party.members.len() > MAX_PARTY_SIZE {
            return vec![PartyEffect::Error {
                message: "Too many members to convert to a party.".into(),
            }];
        }
        party.kind = GroupKind::Party;
        party.raid_groups = [Vec::new(), Vec::new()];
        vec![PartyEffect::Notice {
            message: "Converted to a party.".into(),
        }]
    }
}

/// Party members within `range` yards of the killer share kill credit / XP.
pub const PARTY_CREDIT_RANGE: f32 = 40.0;

/// Classic-era group XP: bonus tenths 10/15/20/25/30 for n=1..=5, then split by n (clamped 1..=10).
pub fn group_xp(mob_xp: u32, n: usize) -> u32 {
    let n = n.clamp(1, 10);
    let bonus_n = n.min(5);
    let bonus_tenths: u64 = 10 + 5 * (bonus_n as u64 - 1); // 10,15,20,25,30
    (mob_xp as u64 * bonus_tenths / (10 * n as u64)) as u32
}

/// Other party members near the killer share kill credit.
pub fn kill_credit_share(roster: &PartyRoster, world: &World, killer: EntityId) -> Vec<EntityId> {
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
                        && world.get::<Health>(*id).map(|h| h.alive).unwrap_or(false)
                        && world
                            .get::<Transform>(*id)
                            .map(|mate| {
                                let dx = mate.x - killer_t.x;
                                let dz = mate.z - killer_t.z;
                                (dx * dx + dz * dz).sqrt() <= PARTY_CREDIT_RANGE
                                    && world
                                        .get::<InstanceAt>(*id)
                                        .and_then(|i| i.instance_id.clone())
                                        == killer_inst
                            })
                            .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn player_exists(world: &World, id: EntityId) -> bool {
    world.get::<ClassKit>(id).is_some()
}

fn find_player_by_name(world: &World, name: &str) -> Option<EntityId> {
    world.ids::<ClassKit>().into_iter().find(|&id| {
        world
            .get::<Identity>(id)
            .is_some_and(|i| i.kind == EntityKind::Player && i.name == name)
    })
}

fn player_name(world: &World, id: EntityId) -> Option<String> {
    world.get::<Identity>(id).map(|i| i.name.clone())
}

fn ready_names(world: &World, ids: &[EntityId]) -> String {
    if ids.is_empty() {
        return "none".into();
    }
    ids.iter()
        .filter_map(|id| player_name(world, *id))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use woc_content::PlayerClass;

    fn world_with_players(n: usize) -> World {
        let mut world = World::new();
        let names = ["Alice", "Bob", "Carol", "Dave", "Eve", "Frank"];
        let classes = [
            PlayerClass::Warrior,
            PlayerClass::Mage,
            PlayerClass::Rogue,
            PlayerClass::Warrior,
            PlayerClass::Mage,
            PlayerClass::Rogue,
        ];
        for i in 0..n {
            crate::ecs::spawn::create_player(
                &mut world,
                (i + 1) as EntityId,
                names[i],
                classes[i],
                i as f32,
                0.0,
            );
        }
        world
    }

    fn form_party(roster: &mut PartyRoster, world: &World, a: EntityId, b: EntityId) {
        let name = world.get::<Identity>(b).map(|i| i.name.clone()).unwrap();
        let effects = roster.invite(a, &name, world, 0);
        assert!(effects
            .iter()
            .any(|e| matches!(e, PartyEffect::Notice { .. })));
        let effects = roster.accept(b, world);
        assert!(effects
            .iter()
            .any(|e| matches!(e, PartyEffect::Update { members } if members.len() == 2)));
    }

    #[test]
    fn invite_accept_forms_party_of_two() {
        let world = world_with_players(2);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        assert_eq!(roster.members_of(1).unwrap(), vec![1, 2]);
    }

    #[test]
    fn invite_unknown_name_errors() {
        let world = world_with_players(1);
        let mut roster = PartyRoster::new();
        let effects = roster.invite(1, "Nobody", &world, 0);
        assert!(matches!(effects.as_slice(), [PartyEffect::Error { .. }]));
    }

    #[test]
    fn leave_dissolves_pair() {
        let world = world_with_players(2);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        let effects = roster.leave(2);
        assert!(effects
            .iter()
            .any(|e| matches!(e, PartyEffect::Update { members } if members.is_empty())));
        assert!(roster.party_id(1).is_none());
    }

    #[test]
    fn party_grows_to_five_then_rejects_sixth() {
        let world = world_with_players(6);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        for other in 3..=5 {
            let name = world.get::<Identity>(other).unwrap().name.clone();
            let _ = roster.invite(1, &name, &world, 0);
            let _ = roster.accept(other, &world);
        }
        assert_eq!(roster.members_of(1).unwrap().len(), 5);
        let effects = roster.invite(1, "Frank", &world, 0);
        assert!(matches!(effects.as_slice(), [PartyEffect::Error { .. }]));
    }

    #[test]
    fn kill_credit_share_returns_mates_in_range() {
        let world = world_with_players(2);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        let mates = kill_credit_share(&roster, &world, 1);
        assert!(mates.contains(&2));
    }

    #[test]
    fn accept_without_invite_errors() {
        let world = world_with_players(2);
        let mut roster = PartyRoster::new();
        let effects = roster.accept(2, &world);
        assert!(matches!(effects.as_slice(), [PartyEffect::Error { .. }]));
    }

    #[test]
    fn decline_clears_pending_and_notifies() {
        let world = world_with_players(2);
        let mut roster = PartyRoster::new();
        let _ = roster.invite(1, "Bob", &world, 0);
        let effects = roster.decline(2, &world);
        assert!(effects.iter().any(|e| matches!(
            e,
            PartyEffect::Notice { message } if message == "Bob declined the invite."
        )));
        let effects = roster.accept(2, &world);
        assert!(matches!(effects.as_slice(), [PartyEffect::Error { message }] if message == "You have no pending party invite."));
    }

    #[test]
    fn invite_expires_after_ttl() {
        let world = world_with_players(2);
        let mut roster = PartyRoster::new();
        let _ = roster.invite(1, "Bob", &world, 10);
        roster.expire_invites(10 + INVITE_TTL_TICKS);
        let effects = roster.accept(2, &world);
        assert!(matches!(effects.as_slice(), [PartyEffect::Error { message }] if message == "You have no pending party invite."));
    }

    #[test]
    fn kick_removes_member_leader_only() {
        let world = world_with_players(3);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        let _ = roster.invite(1, "Carol", &world, 0);
        let _ = roster.accept(3, &world);
        let effects = roster.kick(2, "Carol", &world);
        assert!(matches!(effects.as_slice(), [PartyEffect::Error { message }] if message == "You are not the party leader."));
        let effects = roster.kick(1, "Carol", &world);
        assert!(effects.iter().any(|e| matches!(e, PartyEffect::Notice { message } if message == "Carol was removed from the party.")));
        assert_eq!(roster.members_of(1).unwrap().len(), 2);
        assert!(roster.party_id(3).is_none());
    }

    #[test]
    fn promote_transfers_leader() {
        let world = world_with_players(2);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        let effects = roster.promote(1, "Bob", &world);
        assert!(effects.iter().any(|e| matches!(e, PartyEffect::Notice { message } if message == "Bob is now the leader.")));
        assert_eq!(roster.leader_of(1), Some(2));
        assert!(roster.set_loot_mode(1, crate::social::loot::LootMode::NeedGreed) == false);
        assert!(roster.set_loot_mode(2, crate::social::loot::LootMode::NeedGreed));
    }

    #[test]
    fn disband_clears_all() {
        let world = world_with_players(2);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        let effects = roster.disband(1);
        assert!(effects.iter().any(|e| matches!(e, PartyEffect::Update { members } if members.is_empty())));
        assert!(roster.party_id(1).is_none());
        assert!(roster.party_id(2).is_none());
    }

    #[test]
    fn ready_check_all_ready() {
        let world = world_with_players(2);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        let _ = roster.ready_check(1, 0);
        let _ = roster.ready_respond(1, true, &world, &[1, 2]);
        let effects = roster.ready_respond(2, true, &world, &[1, 2]);
        assert!(effects.iter().any(|e| matches!(e, PartyEffect::Notice { message } if message == "Everyone is ready.")));
    }

    #[test]
    fn group_xp_classic_table() {
        assert_eq!(group_xp(100, 1), 100);
        assert_eq!(group_xp(100, 2), 75);
        assert_eq!(group_xp(100, 3), 66);
        assert_eq!(group_xp(100, 4), 62);
        assert_eq!(group_xp(100, 5), 60);
        assert_eq!(group_xp(100, 10), 30);
        assert_eq!(group_xp(50, 2), 37); // 50 * 15 / 20
    }
}
