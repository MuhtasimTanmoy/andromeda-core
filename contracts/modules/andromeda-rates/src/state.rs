use andromeda_modules::rates::RateInfo;
use cosmwasm_schema::cw_serde;
use cw_storage_plus::{Item, Map};

pub const CONFIG: Item<Config> = Item::new("config");
pub const EXEMPT_ADDRESSES: Map<&str, bool> = Map::new("exemptions");

#[cw_serde]
pub struct Config {
    pub rates: Vec<RateInfo>,
}
