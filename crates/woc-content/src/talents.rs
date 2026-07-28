//! Talent definitions (stub for later completion waves).

#[derive(Debug, Clone)]
pub struct TalentDef {
    pub id: &'static str,
    pub name: &'static str,
    pub class_id: &'static str,
    pub tier: u32,
    pub max_rank: u32,
}

pub static TALENTS: &[TalentDef] = &[];

pub fn talent(id: &str) -> Option<&'static TalentDef> {
    TALENTS.iter().find(|t| t.id == id)
}
