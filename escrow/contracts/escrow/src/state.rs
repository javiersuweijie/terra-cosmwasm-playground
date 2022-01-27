use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128, Uint64};
use cw_storage_plus::{Map, Item};
use asset::Asset;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PaymentRequest {
        pub id: String,
        pub asset: Asset,
        pub order_id: String,
        pub paid_amount: Uint128,
        pub refund_amount: Uint128,
        pub merchant: Addr,
        pub customer: Addr
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub last_id: Uint64,
    pub shop: Addr,
}

pub const PAYMENT_REQUESTS: Map<String, PaymentRequest> = Map::new("payment_requests");
pub const STATE: Item<State> = Item::new("state");
