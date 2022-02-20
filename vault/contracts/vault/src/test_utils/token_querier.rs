use cosmwasm_std::{to_binary, Addr, QuerierResult, SystemError, Uint128};
use cw20::{
    BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg, MinterResponse,
    TokenInfoResponse,
};
use std::collections::HashMap;

#[derive(Default)]
pub struct TokenQuerier {
    token_balance: HashMap<String, BalanceResponse>,
}

impl TokenQuerier {
    pub fn handle_query(&self, contract_addr: &Addr, query: Cw20QueryMsg) -> QuerierResult {
        match query {
            Cw20QueryMsg::Balance { address } => self.query_balance(address),
            _ => Err(SystemError::UnsupportedRequest {
                kind: format!("unknown request"),
            })
            .into(),
        }
    }

    fn query_balance(&self, address: String) -> QuerierResult {
        let balance = self.token_balance.get(&address);
        match balance {
            Some(b) => Ok(to_binary(&balance).into()).into(),
            None => Err(SystemError::InvalidRequest {
                error: format!("no balance found for {}", address),
                request: Default::default(),
            })
            .into(),
        }
    }

    fn set_balance(&mut self, address: String, amount: Uint128) -> () {
        self.token_balance
            .insert(address, BalanceResponse { balance: amount });
    }
}
