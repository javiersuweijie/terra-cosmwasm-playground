use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Addr;
use cw_storage_plus::Map;

use crate::msg::{Move};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GameState {
    pub host: Addr,
    pub host_move: Move,
    pub opponent: Addr,
}

pub const STATE: Map<(Addr, Addr), GameState> = Map::new("state");
