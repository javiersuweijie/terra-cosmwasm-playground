use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use asset::AssetInfo;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    pub vault_position: String,
    pub vault_addr: Addr,
    pub owner: Addr,
    pub shares: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Farm {
    pub base_asset_info: AssetInfo,
    pub other_asset_info: AssetInfo,
    pub lp_token_addr: Addr,
    pub total_shares: Uint128,
}

pub const POSITIONS: Map<String, Position> = Map::new("positions");
pub const FARM: Item<Farm> = Item::new("farm");
