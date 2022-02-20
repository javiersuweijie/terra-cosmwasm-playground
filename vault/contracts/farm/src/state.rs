use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use asset::AssetInfo;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    pub vault_position_id: String,
    pub owner: Addr,
    pub shares: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Farm {
    pub vault_addr: Addr,
    pub base_asset: AssetInfo,
    pub other_asset: AssetInfo,
    pub claim_asset_addr: Addr,
    pub total_shares: Uint128,
    pub astroport_factory_addr: Addr,
}

pub const POSITIONS: Map<String, Position> = Map::new("positions");
pub const FARM: Item<Farm> = Item::new("farm");
pub const TEMP: Item<Position> = Item::new("temp");
