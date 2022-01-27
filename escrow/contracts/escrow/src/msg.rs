use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr,Uint64,Uint128, Binary};
use cw20::Cw20ReceiveMsg;
use asset::{Asset};

use crate::state::PaymentRequest;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub shop: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    CreatePaymentRequest { asset: Asset, order_id: String },
    PayIntoPaymentRequest { id: String },
    ApproveRefund { id: String, asset: Asset },
    SettlePaymentRequest { id: String },
    Receive(Cw20ReceiveMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetPaymentRequestById { id: String }
} 

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    PayIntoPaymentRequest { id: String }
} 

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct PaymentRequestResponse {
    pub payment_request: PaymentRequest,
}