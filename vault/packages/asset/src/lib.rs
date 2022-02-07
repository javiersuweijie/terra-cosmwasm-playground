use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

use cosmwasm_std::{
    to_binary, Addr, Api, BankMsg, CanonicalAddr, Coin, CosmosMsg, Decimal, MessageInfo,
    QuerierWrapper, StdError, StdResult, SubMsg, Uint128, WasmMsg,
};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Asset {
    pub info: AssetInfo,
    pub amount: Uint128,
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.amount, self.info)
    }
}

static DECIMAL_FRACTION: Uint128 = Uint128::new(1_000_000_000_000_000_000u128);

impl Asset {
    pub fn is_native_token(&self) -> bool {
        self.info.is_native_token()
    }

    // pub fn compute_tax(&self, querier: &QuerierWrapper) -> StdResult<Uint128> {
    //     let amount = self.amount;
    //     if let AssetInfo::NativeToken { denom } = &self.info {
    //         if denom == "uluna" {
    //             Ok(Uint128::zero())
    //         } else {
    //             let terra_querier = TerraQuerier::new(querier);
    //             let tax_rate: Decimal = (terra_querier.query_tax_rate()?).rate;
    //             let tax_cap: Uint128 = (terra_querier.query_tax_cap(denom.to_string())?).cap;
    //             Ok(std::cmp::min(
    //                 amount.checked_sub(amount.multiply_ratio(
    //                     DECIMAL_FRACTION,
    //                     DECIMAL_FRACTION * tax_rate + DECIMAL_FRACTION,
    //                 ))?,
    //                 tax_cap,
    //             ))
    //         }
    //     } else {
    //         Ok(Uint128::zero())
    //     }
    // }

    // pub fn deduct_tax(&self, querier: &QuerierWrapper) -> StdResult<Coin> {
    //     let amount = self.amount;
    //     if let AssetInfo::NativeToken { denom } = &self.info {
    //         Ok(Coin {
    //             denom: denom.to_string(),
    //             amount: amount.checked_sub(self.compute_tax(querier)?)?,
    //         })
    //     } else {
    //         Err(StdError::generic_err("cannot deduct tax from token asset"))
    //     }
    // }

    // pub fn into_msg(self, querier: &QuerierWrapper, recipient: Addr) -> StdResult<CosmosMsg> {
    //     let amount = self.amount;

    //     match &self.info {
    //         AssetInfo::Token { contract_addr } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
    //             contract_addr: contract_addr.to_string(),
    //             msg: to_binary(&Cw20ExecuteMsg::Transfer {
    //                 recipient: recipient.to_string(),
    //                 amount,
    //             })?,
    //             funds: vec![],
    //         })),
    //         AssetInfo::NativeToken { .. } => Ok(CosmosMsg::Bank(BankMsg::Send {
    //             to_address: recipient.to_string(),
    //             amount: vec![self.deduct_tax(querier)?],
    //         })),
    //     }
    // }

    // pub fn into_submsg(self, querier: &QuerierWrapper, recipient: Addr) -> StdResult<SubMsg> {
    //     Ok(SubMsg::new(self.into_msg(querier, recipient)?))
    // }

    // pub fn assert_sent_native_token_balance(&self, message_info: &MessageInfo) -> StdResult<()> {
    //     if let AssetInfo::NativeToken { denom } = &self.info {
    //         match message_info.funds.iter().find(|x| x.denom == *denom) {
    //             Some(coin) => {
    //                 if self.amount == coin.amount {
    //                     Ok(())
    //                 } else {
    //                     Err(StdError::generic_err("Native token balance mismatch between the argument and the transferred"))
    //                 }
    //             }
    //             None => {
    //                 if self.amount.is_zero() {
    //                     Ok(())
    //                 } else {
    //                     Err(StdError::generic_err("Native token balance mismatch between the argument and the transferred"))
    //                 }
    //             }
    //         }
    //     } else {
    //         Ok(())
    //     }
    // }
}

/// AssetInfo contract_addr is usually passed from the cw20 hook
/// so we can trust the contract_addr is properly validated.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfo {
    Token { contract_addr: String },
    NativeToken { denom: String },
}

impl fmt::Display for AssetInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AssetInfo::NativeToken { denom } => write!(f, "{}", denom),
            AssetInfo::Token { contract_addr } => write!(f, "{}", contract_addr),
        }
    }
}

impl AssetInfo {

    pub fn is_native_token(&self) -> bool {
        match self {
            AssetInfo::NativeToken { .. } => true,
            AssetInfo::Token { .. } => false,
        }
    }
    // pub fn query_pool(
    //     &self,
    //     querier: &QuerierWrapper,
    //     api: &dyn Api,
    //     pool_addr: Addr,
    // ) -> StdResult<Uint128> {
    //     match self {
    //         AssetInfo::Token { contract_addr, .. } => query_token_balance(
    //             querier,
    //             api.addr_validate(contract_addr.as_str())?,
    //             pool_addr,
    //         ),
    //         AssetInfo::NativeToken { denom, .. } => {
    //             query_balance(querier, pool_addr, denom.to_string())
    //         }
    //     }
    // }

    pub fn equal(&self, asset: &AssetInfo) -> bool {
        match self {
            AssetInfo::Token { contract_addr, .. } => {
                let self_contract_addr = contract_addr;
                match asset {
                    AssetInfo::Token { contract_addr, .. } => self_contract_addr == contract_addr,
                    AssetInfo::NativeToken { .. } => false,
                }
            }
            AssetInfo::NativeToken { denom, .. } => {
                let self_denom = denom;
                match asset {
                    AssetInfo::Token { .. } => false,
                    AssetInfo::NativeToken { denom, .. } => self_denom == denom,
                }
            }
        }
    }
}