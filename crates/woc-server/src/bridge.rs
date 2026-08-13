//! Convert between `woc_persist` DTOs and `woc_sim::PlayerPersistentState`.

use woc_content::ItemQuality;
use woc_persist::{
    Character, CharacterSave, EquipmentDto, GuildDto, GuildMemberDto, InvStackDto, MailDto,
    MarketListingDto, ProfessionSkillDto, QuestProgressDto, RealmEconomy, ReputationDto,
    TalentRankDto,
};
use woc_sim::ecs::components::{
    Equipment, EquipmentQualities, EquipmentWear, InvStack, QuestProgress,
};
use woc_sim::mail::MailItem;
use woc_sim::market::Listing;
use woc_sim::persist_state::{quest_state_from_str, quest_state_to_str, PlayerPersistentState};
use woc_sim::social::guild::{Guild, GuildMember, GuildRank};
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
        equipment_wear: wear_from_dto(&character.equipment),
        equipment_enchants: enchants_from_dto(&character.equipment),
        equipment_qualities: qualities_from_dto(&character.equipment),
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
        hearth_zone_id: character.hearth_zone_id.clone(),
        hearth_x: character.hearth_x,
        hearth_z: character.hearth_z,
        hearth_ready_tick: character.hearth_ready_tick,
        stance_id: character.stance_id.clone(),
        reputation: character
            .reputation
            .iter()
            .map(|r| (r.faction_id.clone(), r.value))
            .collect(),
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
        equipment: equip_to_dto(
            &state.equipment,
            &state.equipment_wear,
            &state.equipment_enchants,
            &state.equipment_qualities,
        ),
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
        hearth_zone_id: state.hearth_zone_id.clone(),
        hearth_x: state.hearth_x,
        hearth_z: state.hearth_z,
        hearth_ready_tick: state.hearth_ready_tick,
        stance_id: state.stance_id.clone(),
        reputation: state
            .reputation
            .iter()
            .map(|(id, value)| ReputationDto {
                faction_id: id.clone(),
                value: *value,
            })
            .collect(),
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

    let guilds: Vec<Guild> = economy.guilds.iter().map(guild_from_dto).collect();
    sim.guilds.load_guilds(guilds, economy.next_guild_id);
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
        guilds: sim
            .guilds
            .all_guilds()
            .into_iter()
            .map(guild_to_dto)
            .collect(),
        next_guild_id: sim.guilds.next_id(),
    }
}

fn guild_from_dto(dto: &GuildDto) -> Guild {
    Guild {
        id: dto.id,
        name: dto.name.clone(),
        motd: dto.motd.clone(),
        motd_set_by: dto.motd_set_by.clone(),
        members: dto.members.iter().map(guild_member_from_dto).collect(),
    }
}

fn guild_member_from_dto(dto: &GuildMemberDto) -> GuildMember {
    GuildMember {
        durable_id: dto.durable_id.clone(),
        name: dto.name.clone(),
        class_id: dto.class_id.clone(),
        level: dto.level,
        rank: GuildRank::parse(&dto.rank).unwrap_or(GuildRank::Member),
    }
}

fn guild_to_dto(g: Guild) -> GuildDto {
    GuildDto {
        id: g.id,
        name: g.name,
        motd: g.motd,
        motd_set_by: g.motd_set_by,
        members: g
            .members
            .into_iter()
            .map(|m| GuildMemberDto {
                durable_id: m.durable_id,
                name: m.name,
                class_id: m.class_id,
                level: m.level,
                rank: m.rank.as_str().to_string(),
            })
            .collect(),
    }
}

fn quality_from_dto(s: &Option<String>) -> Option<ItemQuality> {
    s.as_deref().and_then(ItemQuality::parse)
}

fn quality_to_dto(q: Option<ItemQuality>) -> Option<String> {
    q.map(|q| q.as_str().to_string())
}

fn inv_from_dto(slots: &[Option<InvStackDto>]) -> Vec<Option<InvStack>> {
    slots
        .iter()
        .map(|s| {
            s.as_ref().map(|st| InvStack {
                item_id: st.item_id.clone(),
                count: st.count,
                durability: st.durability,
                enchant_id: st.enchant_id.clone(),
                quality: quality_from_dto(&st.quality),
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
                durability: st.durability,
                enchant_id: st.enchant_id.clone(),
                quality: quality_to_dto(st.quality),
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
        neck: e.neck.clone(),
        finger: e.finger.clone(),
        finger2: e.finger2.clone(),
        shoulder: e.shoulder.clone(),
        back: e.back.clone(),
        wrist: e.wrist.clone(),
        hands: e.hands.clone(),
        waist: e.waist.clone(),
        trinket: e.trinket.clone(),
        trinket2: e.trinket2.clone(),
    }
}

fn wear_from_dto(e: &EquipmentDto) -> EquipmentWear {
    EquipmentWear {
        main_hand: e.main_hand_durability,
        off_hand: e.off_hand_durability,
        head: e.head_durability,
        chest: e.chest_durability,
        legs: e.legs_durability,
        feet: e.feet_durability,
        shoulder: e.shoulder_durability,
        back: e.back_durability,
        wrist: e.wrist_durability,
        hands: e.hands_durability,
        waist: e.waist_durability,
    }
}

fn enchants_from_dto(e: &EquipmentDto) -> woc_sim::ecs::components::EquipmentEnchants {
    woc_sim::ecs::components::EquipmentEnchants {
        main_hand: e.main_hand_enchant.clone(),
        off_hand: e.off_hand_enchant.clone(),
    }
}

fn qualities_from_dto(e: &EquipmentDto) -> EquipmentQualities {
    EquipmentQualities {
        main_hand: quality_from_dto(&e.main_hand_quality),
        off_hand: quality_from_dto(&e.off_hand_quality),
        head: quality_from_dto(&e.head_quality),
        chest: quality_from_dto(&e.chest_quality),
        legs: quality_from_dto(&e.legs_quality),
        feet: quality_from_dto(&e.feet_quality),
        neck: quality_from_dto(&e.neck_quality),
        finger: quality_from_dto(&e.finger_quality),
        finger2: quality_from_dto(&e.finger2_quality),
        shoulder: quality_from_dto(&e.shoulder_quality),
        back: quality_from_dto(&e.back_quality),
        wrist: quality_from_dto(&e.wrist_quality),
        hands: quality_from_dto(&e.hands_quality),
        waist: quality_from_dto(&e.waist_quality),
        trinket: quality_from_dto(&e.trinket_quality),
        trinket2: quality_from_dto(&e.trinket2_quality),
    }
}

fn equip_to_dto(
    e: &Equipment,
    wear: &EquipmentWear,
    enchants: &woc_sim::ecs::components::EquipmentEnchants,
    qualities: &EquipmentQualities,
) -> EquipmentDto {
    EquipmentDto {
        main_hand: e.main_hand.clone(),
        off_hand: e.off_hand.clone(),
        head: e.head.clone(),
        chest: e.chest.clone(),
        legs: e.legs.clone(),
        feet: e.feet.clone(),
        neck: e.neck.clone(),
        finger: e.finger.clone(),
        finger2: e.finger2.clone(),
        shoulder: e.shoulder.clone(),
        back: e.back.clone(),
        wrist: e.wrist.clone(),
        hands: e.hands.clone(),
        waist: e.waist.clone(),
        trinket: e.trinket.clone(),
        trinket2: e.trinket2.clone(),
        main_hand_enchant: enchants.main_hand.clone(),
        off_hand_enchant: enchants.off_hand.clone(),
        main_hand_durability: wear.main_hand,
        off_hand_durability: wear.off_hand,
        head_durability: wear.head,
        chest_durability: wear.chest,
        legs_durability: wear.legs,
        feet_durability: wear.feet,
        shoulder_durability: wear.shoulder,
        back_durability: wear.back,
        wrist_durability: wear.wrist,
        hands_durability: wear.hands,
        waist_durability: wear.waist,
        main_hand_quality: quality_to_dto(qualities.main_hand),
        off_hand_quality: quality_to_dto(qualities.off_hand),
        head_quality: quality_to_dto(qualities.head),
        chest_quality: quality_to_dto(qualities.chest),
        legs_quality: quality_to_dto(qualities.legs),
        feet_quality: quality_to_dto(qualities.feet),
        neck_quality: quality_to_dto(qualities.neck),
        finger_quality: quality_to_dto(qualities.finger),
        finger2_quality: quality_to_dto(qualities.finger2),
        shoulder_quality: quality_to_dto(qualities.shoulder),
        back_quality: quality_to_dto(qualities.back),
        wrist_quality: quality_to_dto(qualities.wrist),
        hands_quality: quality_to_dto(qualities.hands),
        waist_quality: quality_to_dto(qualities.waist),
        trinket_quality: quality_to_dto(qualities.trinket),
        trinket2_quality: quality_to_dto(qualities.trinket2),
    }
}

fn quests_from_dto(quests: &[QuestProgressDto]) -> Vec<QuestProgress> {
    quests
        .iter()
        .map(|q| QuestProgress {
            quest_id: q.quest_id.clone(),
            state: quest_state_from_str(&q.state),
            counts: q.counts.clone(),
            completed_tick: q.completed_tick,
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
            completed_tick: q.completed_tick,
        })
        .collect()
}
