#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{CosmosMsg, BankMsg, WasmMsg, coins, from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Addr, StdError, Order, Uint64, Uint128, Coin};
use cw2::set_contract_version;
use cw20::{Cw20ReceiveMsg, Cw20ExecuteMsg};
use asset::{Asset, AssetInfo};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, PaymentRequestResponse, Cw20HookMsg};
use crate::state::{PaymentRequest, STATE, State, PAYMENT_REQUESTS};


// version info for migration info
const CONTRACT_NAME: &str = "crates.io:sps";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let state = State {
        // Todo: validate the shop contract
        shop: msg.shop.clone(),
        last_id: Uint64::from(0 as u64),
    };
    match STATE.save(deps.storage, &state) {
        Ok(_) => Ok(Response::new()
                    .add_attribute("method", "instantiate")),
        Err(e) => Err(ContractError::Std(e)),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CreatePaymentRequest { asset, order_id } => create_payment_request(deps, info, asset, order_id),
        ExecuteMsg::PayIntoPaymentRequest { id } => pay_into_payment_request(deps, info, id),
        ExecuteMsg::ApproveRefund { asset, id } => Ok(Response::default()),
        ExecuteMsg::SettlePaymentRequest { id } => settle_payment_request(deps, info, id),
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
    }
}

pub fn receive_cw20(deps: DepsMut, _: Env, info: MessageInfo, msg: Cw20ReceiveMsg) -> Result<Response, ContractError> {
    let contract_addr = info.sender.clone();
    match from_binary::<Cw20HookMsg>(&msg.msg) {
        Ok(Cw20HookMsg::PayIntoPaymentRequest {id}) => {
            let customer = deps.api.addr_validate(&msg.sender).unwrap(); // TODO: how to properly handle this?
            match PAYMENT_REQUESTS.may_load(deps.storage, id.clone())? {
                None => Err(ContractError::Std(StdError::NotFound {kind: String::from("payment_request")})),
                Some(p) => { 
                    match p.asset.info {
                        AssetInfo::NativeToken { denom: _ } => Err(ContractError::WrongToken {}),
                        AssetInfo::Token { contract_addr: pr_contract_addr } => {
                            if contract_addr != pr_contract_addr {
                                return Err(ContractError::WrongToken {});
                            }
                            if p.asset.amount != msg.amount {
                                return Err(ContractError::WrongAmount {amount: msg.amount});
                            }
                            PAYMENT_REQUESTS.update(deps.storage, id, |p_new_| -> StdResult<_> {
                                match p_new_ {
                                    None => Err(StdError::NotFound {kind: String::from("payment_request")}),
                                    Some(mut p_new) => {
                                        p_new.paid_amount = msg.amount;
                                        p_new.customer = customer.clone();
                                        Ok(p_new)
                                    }
                                }
                            })?;
                            Ok(Response::default())
                        }
                    }
                }
            }
        },
        _ => Err(ContractError::Std(StdError::GenericErr {msg: String::from("unknown hook")}))
    }
}

pub fn create_payment_request(deps: DepsMut, info: MessageInfo, asset: Asset, order_id: String) -> Result<Response, ContractError> {
    let merchant = info.sender;
    let state_ = STATE.may_load(deps.storage)?;
    match state_ {
        None => Err(ContractError::Std(StdError::GenericErr {msg: String::from("contract state invalid")})),
        Some(state) => {
            let id = state.last_id + Uint64::from(1 as u64);
            let id_string = id.to_string();
            let payment_request = PaymentRequest {
                 merchant: merchant.clone(),
                 customer: Addr::unchecked( "0"),
                 asset: asset.clone(),
                 order_id: order_id.clone(),
                 id: id_string.clone(),
                 paid_amount: Uint128::from(0 as u64),
                 refund_amount: Uint128::from(0 as u64),
            };
            match PAYMENT_REQUESTS.save(deps.storage, id_string.clone(), &payment_request) {
                Ok(_) => Ok(Response::new().add_attribute("id", id_string)),
                Err(err) => Err(ContractError::Std(err)),
            }
        }
    }
}

pub fn pay_into_payment_request(deps: DepsMut, info: MessageInfo, id: String) -> Result<Response, ContractError> {
    let customer = info.sender;
    match PAYMENT_REQUESTS.may_load(deps.storage, id.clone())? {
        None => Err(ContractError::Std(StdError::NotFound {kind: String::from("payment_request")})),
        Some(p) => { 
            /*
             * TODO: handle refunding tokens that are:
             * 1. Not the token that we are interested in
             * 2. Tokens amount remaining after paying
             */
            
            let coin = find_matching_fund(&info.funds, p.asset)?;
            PAYMENT_REQUESTS.update(deps.storage, id, |p_new_| -> StdResult<_> {
                match p_new_ {
                    None => Err(StdError::NotFound {kind: String::from("payment_request")}),
                    Some(mut p_new) => {
                        p_new.paid_amount = coin.amount;
                        p_new.customer = customer.clone();
                        Ok(p_new)
                    }
                }
            })?;
            Ok(Response::new())
        }
    }
}

fn find_matching_fund(coins: &Vec<Coin>, asset: Asset) -> Result<&Coin, ContractError> {
    match asset.info {
        AssetInfo::NativeToken { denom } => {
            match coins.iter().find(|c| c.denom == denom) {
                None => Err(ContractError::WrongToken {}),
                Some(coin) => {
                    if coin.amount != asset.amount {
                        return Err(ContractError::WrongAmount {amount: coin.amount});
                    } else {
                        return Ok(coin);
                    }
                }
            }
        },
        AssetInfo::Token { contract_addr: _ } => Err(ContractError::WrongToken {})
    }
}

pub fn settle_payment_request(deps: DepsMut, info: MessageInfo, id: String) -> Result<Response, ContractError> {
    let customer = info.sender;
    match PAYMENT_REQUESTS.may_load(deps.storage, id.clone())? {
        None => Err(ContractError::Std(StdError::NotFound {kind: String::from("payment_request")})),
        Some(p) => { 
            if p.customer != customer {
                return Err(ContractError::Unauthorized {});
            }
            if p.paid_amount != p.asset.amount {
                return Err(ContractError::Unpaid {}); 
            }
            PAYMENT_REQUESTS.remove(deps.storage, id.clone());

            match p.asset.info {
                AssetInfo::NativeToken {denom} => Ok(Response::new().add_message(CosmosMsg::Bank(BankMsg::Send {
                        amount: coins(p.paid_amount.u128(), denom),
                        to_address: p.merchant.to_string(),
                }))),
                AssetInfo::Token {contract_addr } => Ok(Response::new().add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr,
                    funds: vec!(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        amount: p.paid_amount,
                        recipient: p.merchant.to_string(),
                    })?
                })))
            }
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetPaymentRequestById { id } => to_binary(&get_payment_request_by_id(deps, id)?),
    }
}

pub fn get_payment_request_by_id(deps: Deps, id: String) -> StdResult<PaymentRequestResponse> {
    match PAYMENT_REQUESTS.may_load(deps.storage, id.clone())? {
        None => Err(StdError::NotFound {kind: String::from("payment request")}),
        Some(p) => {
            Ok(PaymentRequestResponse {
                payment_request: p
            })
        }
    }
}