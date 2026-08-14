//! Multi-room solo delve definitions.

#[derive(Debug, Clone, Copy)]
pub struct DelveRoomDef {
    pub mob_template: &'static str,
    pub count: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct DelveReward {
    pub copper: u32,
    pub item_id: Option<&'static str>,
    pub item_count: u32,
}

#[derive(Debug, Clone)]
pub struct DelveDef {
    pub id: &'static str,
    pub name: &'static str,
    /// Overworld zone restored after the final room.
    pub zone_id: &'static str,
    pub min_level: u32,
    pub entrance_x: f32,
    pub entrance_z: f32,
    pub rooms: &'static [DelveRoomDef],
    pub reward: DelveReward,
}

pub static DELVES: &[DelveDef] = &[DelveDef {
    id: "eastbrook_hollow",
    name: "Eastbrook Hollow",
    zone_id: "eastbrook",
    min_level: 1,
    entrance_x: 8.0,
    entrance_z: -6.0,
    rooms: &[
        DelveRoomDef {
            mob_template: "young_wolf",
            count: 2,
        },
        DelveRoomDef {
            mob_template: "young_boar",
            count: 3,
        },
        DelveRoomDef {
            mob_template: "scarred_wolf",
            count: 1,
        },
    ],
    reward: DelveReward {
        copper: 75,
        item_id: Some("eastbrook_greaves"),
        item_count: 1,
    },
}];

pub fn delve(id: &str) -> Option<&'static DelveDef> {
    DELVES.iter().find(|delve| delve.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{item, mob};

    #[test]
    fn eastbrook_hollow_has_three_valid_rooms_and_final_rewards() {
        let hollow = delve("eastbrook_hollow").expect("eastbrook hollow delve");

        assert_eq!(hollow.zone_id, "eastbrook");
        assert!(hollow.rooms.len() >= 3);
        for room in hollow.rooms {
            assert!(room.count > 0);
            assert!(
                mob(room.mob_template).is_some(),
                "missing mob template {}",
                room.mob_template
            );
        }
        assert!(hollow.reward.copper > 0);
        let item_id = hollow.reward.item_id.expect("item reward");
        assert!(item(item_id).is_some(), "missing reward item {item_id}");
        assert!(hollow.reward.item_count > 0);
    }

    #[test]
    fn hollow_entrance_is_away_from_eastbrook_spawn() {
        let hollow = delve("eastbrook_hollow").unwrap();
        let dx = hollow.entrance_x - 2.0;
        let dz = hollow.entrance_z - 4.0;
        assert!((dx * dx + dz * dz).sqrt() > 5.0);
        assert!((hollow.entrance_x - 8.0).abs() < 1e-3);
        assert!((hollow.entrance_z + 6.0).abs() < 1e-3);
    }
}
