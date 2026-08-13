//! Durable realm economy (mail + auction house).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MailDto {
    pub id: u32,
    pub from: String,
    pub to_durable: String,
    pub subject: String,
    pub copper: u32,
    pub item_id: Option<String>,
    pub item_count: u32,
    #[serde(default)]
    pub durability: Option<u32>,
    #[serde(default)]
    pub enchant_id: Option<String>,
    #[serde(default)]
    pub bound: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MarketListingDto {
    pub id: u32,
    pub seller_durable: String,
    pub seller_name: String,
    pub item_id: String,
    pub count: u32,
    pub price: u32,
    pub expires_tick: u64,
    #[serde(default)]
    pub durability: Option<u32>,
    #[serde(default)]
    pub enchant_id: Option<String>,
    #[serde(default)]
    pub bound: bool,
    #[serde(default)]
    pub start_bid: u32,
    #[serde(default)]
    pub current_bid: u32,
    #[serde(default)]
    pub bidder_durable: Option<String>,
    #[serde(default)]
    pub bidder_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RealmEconomy {
    #[serde(default)]
    pub mail: Vec<MailDto>,
    #[serde(default)]
    pub market: Vec<MarketListingDto>,
    #[serde(default = "default_next_id")]
    pub next_mail_id: u32,
    #[serde(default = "default_next_id")]
    pub next_listing_id: u32,
}

fn default_next_id() -> u32 {
    1
}

pub fn economy_to_json(economy: &RealmEconomy) -> Result<String, serde_json::Error> {
    serde_json::to_string(economy)
}

pub fn economy_from_json(s: &str) -> Result<RealmEconomy, serde_json::Error> {
    serde_json::from_str(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn economy_roundtrip() {
        let eco = RealmEconomy {
            mail: vec![MailDto {
                id: 1,
                from: "AH".into(),
                to_durable: "ada".into(),
                subject: "Sold".into(),
                copper: 40,
                item_id: None,
                item_count: 0,
                durability: None,
                enchant_id: None,
                bound: false,
            }],
            market: vec![MarketListingDto {
                id: 2,
                seller_durable: "bob".into(),
                seller_name: "Bob".into(),
                item_id: "silverleaf".into(),
                count: 1,
                price: 12,
                expires_tick: 100,
                durability: None,
                enchant_id: None,
                bound: false,
                start_bid: 0,
                current_bid: 0,
                bidder_durable: None,
                bidder_name: None,
            }],
            next_mail_id: 3,
            next_listing_id: 4,
        };
        let back = economy_from_json(&economy_to_json(&eco).unwrap()).unwrap();
        assert_eq!(back, eco);
    }

    #[test]
    fn economy_omitted_instance_fields_default() {
        let eco: RealmEconomy = serde_json::from_str(
            r#"{"mail":[{"id":1,"from":"AH","to_durable":"ada","subject":"Sold","copper":40,"item_id":null,"item_count":0}],"market":[{"id":2,"seller_durable":"bob","seller_name":"Bob","item_id":"worn_sword","count":1,"price":12,"expires_tick":100}],"next_mail_id":3,"next_listing_id":4}"#,
        )
        .unwrap();
        assert!(eco.mail[0].durability.is_none());
        assert!(eco.market[0].enchant_id.is_none());
        assert!(!eco.mail[0].bound);
        assert_eq!(eco.market[0].start_bid, 0);
        assert!(eco.market[0].bidder_durable.is_none());
    }
}
