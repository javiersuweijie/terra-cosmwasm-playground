use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use asset::AssetInfo;
use cosmwasm_std::{Addr, Timestamp, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    pub id: String,
    pub farm_addr: Addr,
    pub debt_share: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Vault {
    pub last_id: Uint128,
    pub asset_info: AssetInfo,
    pub vault_token_addr: Addr,
    pub total_debt_shares: Uint128,
    pub total_debt: Uint128,
    pub reserve_pool: Uint128,
    pub reserve_pool_bps: u64,
    pub whitelisted_farms: Vec<Addr>,
    pub admin: Addr,
    pub last_accrue_timestamp: Timestamp,
}

pub const POSITIONS: Map<String, Position> = Map::new("positions");
pub const VAULT: Item<Vault> = Item::new("vault");
