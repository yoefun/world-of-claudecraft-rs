//! Convert between `woc_persist` DTOs and `woc_sim::PlayerPersistentState`.

use woc_persist::{
    Character, CharacterSave, EquipmentDto, InvStackDto, MailDto, MarketListingDto,
    ProfessionSkillDto, QuestProgressDto, RealmEconomy, TalentRankDto,
};
use woc_sim::entity::{Equipment, InvStack, QuestProgress};
use woc_sim::mail::MailItem;
use woc_sim::market::Listing;
use woc_sim::persist_state::{quest_state_from_str, quest_state_to_str, PlayerPersistentState};
use woc_sim::Sim;

pub fn character_to_state(character: &Character) -> PlayerPersistentState {
    PlayerPersistentState {
        durable_id: Some(character.id.to_string()),
        level: character.level,
        xp: character.xp,
        copper: character.copper,
        pos_x: character.pos_x,
        pos_z: character.pos_z,
        inventory: inv_from_dto(&character.inventory),
        equipment: equip_from_dto(&character.equipment),
        quests: quests_from_dto(&character.quests),
        zone_id: character.zone_id.clone(),
        talent_points: character.talent_points,
        talents: character
            .talents
            .iter()
            .map(|t| (t.talent_id.clone(), t.rank))
            .collect(),
        bank: inv_from_dto(&character.bank),
        bank_copper: character.bank_copper,
        honor: character.honor,
        professions: character
            .professions
            .iter()
            .map(|p| (p.id.clone(), p.skill))
            .collect(),
        pvp_flagged: character.pvp_flagged,
        completed_deeds: character.completed_deeds.iter().cloned().collect(),
    }
}

pub fn state_to_save(state: &PlayerPersistentState) -> CharacterSave {
    CharacterSave {
        level: state.level,
        xp: state.xp,
        copper: state.copper,
        pos_x: state.pos_x,
        pos_z: state.pos_z,
        inventory: inv_to_dto(&state.inventory),
        equipment: equip_to_dto(&state.equipment),
        quests: quests_to_dto(&state.quests),
        zone_id: state.zone_id.clone(),
        talent_points: state.talent_points,
        talents: state
            .talents
            .iter()
            .map(|(id, rank)| TalentRankDto {
                talent_id: id.clone(),
                rank: *rank,
            })
            .collect(),
        bank: inv_to_dto(&state.bank),
        bank_copper: state.bank_copper,
        honor: state.honor,
        professions: state
            .professions
            .iter()
            .map(|(id, skill)| ProfessionSkillDto {
                id: id.clone(),
                skill: *skill,
            })
            .collect(),
        pvp_flagged: state.pvp_flagged,
        completed_deeds: state.completed_deeds.iter().cloned().collect(),
    }
}

pub fn apply_economy_to_sim(sim: &mut Sim, economy: &RealmEconomy) {
    let mails: Vec<MailItem> = economy
        .mail
        .iter()
        .map(|m| MailItem {
            id: m.id,
            from: m.from.clone(),
            to_durable: m.to_durable.clone(),
            subject: m.subject.clone(),
            copper: m.copper,
            item_id: m.item_id.clone(),
            item_count: m.item_count,
        })
        .collect();
    sim.mail.load_mails(mails, economy.next_mail_id);

    let listings: Vec<Listing> = economy
        .market
        .iter()
        .map(|l| Listing {
            id: l.id,
            seller_id: 0,
            seller_durable: l.seller_durable.clone(),
            seller_name: l.seller_name.clone(),
            item_id: l.item_id.clone(),
            count: l.count,
            price: l.price,
            expires_tick: l.expires_tick,
        })
        .collect();
    sim.market.load_listings(listings, economy.next_listing_id);
}

pub fn export_economy_from_sim(sim: &Sim) -> RealmEconomy {
    RealmEconomy {
        mail: sim
            .mail
            .all_mails()
            .into_iter()
            .map(|m| MailDto {
                id: m.id,
                from: m.from,
                to_durable: m.to_durable,
                subject: m.subject,
                copper: m.copper,
                item_id: m.item_id,
                item_count: m.item_count,
            })
            .collect(),
        market: sim
            .market
            .listings
            .iter()
            .map(|l| MarketListingDto {
                id: l.id,
                seller_durable: l.seller_durable.clone(),
                seller_name: l.seller_name.clone(),
                item_id: l.item_id.clone(),
                count: l.count,
                price: l.price,
                expires_tick: l.expires_tick,
            })
            .collect(),
        next_mail_id: sim.mail.next_id(),
        next_listing_id: sim.market.next_id(),
    }
}

fn inv_from_dto(slots: &[Option<InvStackDto>]) -> Vec<Option<InvStack>> {
    slots
        .iter()
        .map(|s| {
            s.as_ref().map(|st| InvStack {
                item_id: st.item_id.clone(),
                count: st.count,
            })
        })
        .collect()
}

fn inv_to_dto(slots: &[Option<InvStack>]) -> Vec<Option<InvStackDto>> {
    slots
        .iter()
        .map(|s| {
            s.as_ref().map(|st| InvStackDto {
                item_id: st.item_id.clone(),
                count: st.count,
            })
        })
        .collect()
}

fn equip_from_dto(e: &EquipmentDto) -> Equipment {
    Equipment {
        main_hand: e.main_hand.clone(),
        off_hand: e.off_hand.clone(),
        head: e.head.clone(),
        chest: e.chest.clone(),
        legs: e.legs.clone(),
        feet: e.feet.clone(),
    }
}

fn equip_to_dto(e: &Equipment) -> EquipmentDto {
    EquipmentDto {
        main_hand: e.main_hand.clone(),
        off_hand: e.off_hand.clone(),
        head: e.head.clone(),
        chest: e.chest.clone(),
        legs: e.legs.clone(),
        feet: e.feet.clone(),
    }
}

fn quests_from_dto(quests: &[QuestProgressDto]) -> Vec<QuestProgress> {
    quests
        .iter()
        .map(|q| QuestProgress {
            quest_id: q.quest_id.clone(),
            state: quest_state_from_str(&q.state),
            counts: q.counts.clone(),
        })
        .collect()
}

fn quests_to_dto(quests: &[QuestProgress]) -> Vec<QuestProgressDto> {
    quests
        .iter()
        .map(|q| QuestProgressDto {
            quest_id: q.quest_id.clone(),
            state: quest_state_to_str(q.state).to_string(),
            counts: q.counts.clone(),
        })
        .collect()
}
