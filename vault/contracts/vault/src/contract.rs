use asset::{Asset, AssetInfo};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, from_binary, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Order, QuerierWrapper, QueryRequest, Reply, ReplyOn, Response, StdError,
    StdResult, Storage, SubMsg, SubMsgExecutionResponse, Timestamp, Uint128, Uint256, Uint64,
    WasmMsg, WasmQuery,
};
use cw2::set_contract_version;
use cw20::{
    BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg, MinterResponse,
    TokenInfoResponse,
};
use protobuf::Message;
use std::collections::HashMap;
use std::convert::{From, TryInto};
use std::ops::{Add, AddAssign, Sub};

use crate::error::ContractError;
use crate::msg::{Cw20HookMsg, Cw20InstantiateMsg, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::response::MsgInstantiateContractResponse;
use crate::state::{Position, Vault, POSITIONS, VAULT};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:sps";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const INSTANTIATE_REPLY_ID: u64 = 1;
const SECONDS_IN_A_YEAR_10000: u128 = 315569520000u128;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let whitelisted_farms: StdResult<Vec<Addr>> = msg
        .whitelisted_farms
        .iter()
        .map(|a| deps.api.addr_validate(a))
        .collect();

    let vault = Vault {
        // Todo: validate the shop contract
        last_id: Uint128::zero(),
        asset_info: msg.asset_info,
        reserve_pool_bps: msg.reserve_pool_bps,
        total_debt: Uint128::zero(),
        total_debt_shares: Uint128::zero(),
        reserve_pool: Uint128::zero(),
        whitelisted_farms: whitelisted_farms?,
        vault_token_addr: Addr::unchecked(String::default()),
        admin: info.sender,
        last_accrue_timestamp: env.block.time,
    };
    match VAULT.save(deps.storage, &vault) {
        Ok(_) => Ok(Response::new().add_submessage(SubMsg {
            // Create LP token
            msg: WasmMsg::Instantiate {
                admin: None,
                code_id: msg.cw20_code_id,
                msg: to_binary(&Cw20InstantiateMsg {
                    name: "vault token".to_string(),
                    symbol: "vtT".to_string(),
                    decimals: 6,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: env.contract.address.to_string(),
                        cap: None,
                    }),
                })?,
                funds: vec![],
                label: "".to_string(),
            }
            .into(),
            gas_limit: None,
            id: INSTANTIATE_REPLY_ID,
            reply_on: ReplyOn::Success,
        })),
        Err(e) => Err(ContractError::Std(e)),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
    let result: SubMsgExecutionResponse =
        msg.result.into_result().map_err(StdError::generic_err)?;
    let data = result
        .data
        .ok_or(StdError::generic_err(format!("Invalid data")))?;

    let res: MsgInstantiateContractResponse =
        Message::parse_from_bytes(data.as_slice()).map_err(|_| {
            StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
        })?;
    let vault_token_addr = res.contract_address;
    let api = deps.api;
    VAULT.update(deps.storage, |mut vault| -> StdResult<_> {
        vault.vault_token_addr = api.addr_validate(vault_token_addr.as_str())?;
        Ok(vault)
    })?;
    Ok(Response::new().add_attribute("vault_token_addr", vault_token_addr))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::Borrow { borrow_amount } => borrow(deps, env, info.sender, borrow_amount),
        ExecuteMsg::AddWhitelist { address } => add_whitelist(deps, env, info, address),
        ExecuteMsg::Accrue {} => accrue_interests(deps, env),
    }
}

fn add_whitelist(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let vault = VAULT.load(deps.storage)?;
    if info.sender != vault.admin {
        return Err(ContractError::Unauthorized {});
    }
    let whitelisted_address = deps.api.addr_validate(&address)?;
    VAULT.update(deps.storage, |mut v| -> StdResult<_> {
        v.whitelisted_farms.push(whitelisted_address);
        Ok(v)
    })?;

    Ok(Response::new())
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    match from_binary::<Cw20HookMsg>(&msg.msg) {
        Ok(e) => match e {
            Cw20HookMsg::Deposit {} => cw20_deposit(deps, env, info, msg),
            Cw20HookMsg::Withdraw {} => withdraw(deps, env, info, msg),
            _ => Err(ContractError::Std(StdError::GenericErr {
                msg: String::from("unknown hook"),
            })),
            Cw20HookMsg::Repay { position_id } => repay(deps, env, info, msg, position_id),
        },
        Err(e) => Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("hook error"),
        })),
    }
}

pub fn cw20_deposit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let asset = Asset {
        info: AssetInfo::Token {
            contract_addr: info.sender.to_string(),
        },
        amount: msg.amount,
    };
    let sender: Addr = deps.api.addr_validate(msg.sender.as_str())?;
    deposit(deps, env, asset, sender)
}

fn withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    // TODO: accure interest

    let vault_token = info.sender;
    let amount: Uint256 = msg.amount.into();
    let sender = msg.sender;

    let vault = VAULT.load(deps.storage)?;
    if vault.vault_token_addr != vault_token {
        return Err(ContractError::WrongToken {});
    }

    let total_vault_shares = query_total_vault_shares(&deps.querier, &vault_token)?;
    let total_balance = total_balance(&deps.querier, env.contract.address, &vault)?;

    _accrue_interests(
        deps.storage,
        &vault.last_accrue_timestamp,
        &env.block.time,
        total_balance,
        vault.total_debt,
    )?;

    let tokens_to_withdraw: Uint128 = amount
        .multiply_ratio(total_balance, total_vault_shares)
        .try_into()
        .map_err(|_| ContractError::Std(StdError::generic_err(format!("conversion error"))))?;

    let burn_vault_token_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: vault.vault_token_addr.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Burn { amount: msg.amount })?,
        funds: vec![],
    });

    match vault.asset_info {
        AssetInfo::NativeToken { denom: _ } => {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: "not supported".into(),
            }));
        }
        AssetInfo::Token { contract_addr } => {
            let transfer_token_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr,
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    amount: tokens_to_withdraw,
                    recipient: sender,
                })?,
                funds: vec![],
            });
            return Ok(Response::new()
                .add_messages(vec![burn_vault_token_msg, transfer_token_msg])
                .add_attributes(vec![
                    ("burnt", amount.to_string()),
                    ("withdrew", tokens_to_withdraw.to_string()),
                ]));
        }
    }
}

pub fn query_total_vault_shares(
    querier: &QuerierWrapper,
    token_contract_addr: &Addr,
) -> StdResult<Uint128> {
    let token_info: TokenInfoResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: token_contract_addr.to_string(),
        msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
    }))?;
    Ok(token_info.total_supply)
}

pub fn query_cw20_balance(
    querier: &QuerierWrapper,
    account_address: String,
    token_contract_addr: String,
) -> StdResult<Uint128> {
    let token_balance: BalanceResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: token_contract_addr,
        msg: to_binary(&Cw20QueryMsg::Balance {
            address: account_address,
        })?,
    }))?;
    Ok(token_balance.balance)
}

pub fn total_balance(
    querier: &QuerierWrapper,
    account_address: Addr,
    vault: &Vault,
) -> StdResult<Uint128> {
    match &vault.asset_info {
        AssetInfo::Token { contract_addr } => {
            let balance: Uint128 = query_cw20_balance(
                querier,
                account_address.to_string(),
                contract_addr.to_string(),
            )?;
            Ok(balance
                .checked_add(vault.total_debt)?
                .checked_sub(vault.reserve_pool)?)
        }
        AssetInfo::NativeToken { denom: _ } => Ok(Uint128::zero()),
    }
}

pub fn deposit(
    deps: DepsMut,
    env: Env,
    asset: Asset,
    sender: Addr,
) -> Result<Response, ContractError> {
    let vault = VAULT.load(deps.storage)?;
    if vault.asset_info != asset.info {
        return Err(ContractError::WrongToken {});
    }
    let total_vault_shares: Uint256 =
        query_total_vault_shares(&deps.querier, &vault.vault_token_addr.clone())?.into();
    let vault_tokens_to_mint: Uint128;
    if total_vault_shares.is_zero() {
        vault_tokens_to_mint = asset.amount;
    } else {
        // Total token balance before deposit
        let total_token_balance_before_deposit =
            total_balance(&deps.querier, env.contract.address, &vault)?
                .checked_sub(asset.amount)?;
        vault_tokens_to_mint = Uint256::from(asset.amount)
            .multiply_ratio(total_vault_shares, total_token_balance_before_deposit)
            .try_into()
            .map_err(|_| ContractError::Std(StdError::generic_err(format!("conversion error"))))?;
        _accrue_interests(
            deps.storage,
            &vault.last_accrue_timestamp,
            &env.block.time,
            total_token_balance_before_deposit,
            vault.total_debt,
        )?;
    }

    Ok(
        Response::new().add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: vault.vault_token_addr.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: sender.to_string(),
                amount: vault_tokens_to_mint,
            })?,
            funds: vec![],
        })),
    )
}

pub fn borrow(
    deps: DepsMut,
    env: Env,
    farm_addr: Addr,
    borrow_amount: Uint128,
) -> Result<Response, ContractError> {
    let mut vault = VAULT.load(deps.storage)?;
    let total_balance = total_balance(&deps.querier, env.contract.address, &vault)?;
    vault = _accrue_interests(
        deps.storage,
        &vault.last_accrue_timestamp,
        &env.block.time,
        total_balance,
        vault.total_debt,
    )?;

    // farm_addr must be whitelisted in order to borrow from vault
    if !vault.whitelisted_farms.iter().any(|a| farm_addr.eq(a)) {
        return Err(ContractError::Std(StdError::generic_err(format!(
            "address not whitelisted {}",
            farm_addr
        ))));
    }

    let debt_share: Uint128 = debt_share_from_value(
        vault.total_debt_shares.into(),
        vault.total_debt.into(),
        borrow_amount.into(),
    )
    .try_into()
    .map_err(|_| ContractError::Std(StdError::generic_err(format!("conversion error"))))?;

    let position_id = vault.last_id + Uint128::from(1u128);

    let position = Position {
        id: position_id.to_string(),
        farm_addr: farm_addr.clone(),
        debt_share: debt_share,
    };

    VAULT.update(deps.storage, |mut v| -> StdResult<_> {
        v.last_id = position_id;
        v.total_debt_shares.add_assign(debt_share);
        v.total_debt.add_assign(borrow_amount);
        Ok(v)
    })?;
    POSITIONS.save(deps.storage, position.id.clone(), &position)?;

    match vault.asset_info {
        AssetInfo::NativeToken { denom: _ } => {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: "not implemented".into(),
            }))
        }
        AssetInfo::Token {
            contract_addr: token_contract_addr,
        } => Ok(Response::new()
            .add_message(CosmosMsg::Wasm(
                // Deposit funds into another contract
                WasmMsg::Execute {
                    contract_addr: token_contract_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        amount: borrow_amount,
                        recipient: farm_addr.to_string(),
                    })?,
                    funds: vec![],
                },
            ))
            .add_attribute("position_id", position.id.clone())),
    }
}

fn repay(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: Cw20ReceiveMsg,
    position_id: String,
) -> Result<Response, ContractError> {
    let mut vault = VAULT.load(deps.storage)?;
    let total_balance = total_balance(&deps.querier, env.contract.address, &vault)?;
    vault = _accrue_interests(
        deps.storage,
        &vault.last_accrue_timestamp,
        &env.block.time,
        total_balance,
        vault.total_debt,
    )?;

    let position = POSITIONS.load(deps.storage, position_id.clone())?;

    let owner = msg.sender;
    let token_addr = info.sender;
    let token_amount = msg.amount;

    match vault.asset_info {
        AssetInfo::NativeToken { denom: _ } => {
            return Err(ContractError::WrongToken {});
        }
        AssetInfo::Token { contract_addr } => {
            if contract_addr != token_addr.to_string() {
                return Err(ContractError::WrongToken {});
            }
        }
    }

    let position_debt_value: Uint128 = debt_value_from_share(
        vault.total_debt_shares.into(),
        vault.total_debt.into(),
        position.debt_share.into(),
    )
    .try_into()
    .map_err(|_| {
        ContractError::Std(StdError::GenericErr {
            msg: format!("conversion error"),
        })
    })?;

    let (refund, debt_left) = if token_amount > position_debt_value {
        (
            token_amount.checked_sub(position_debt_value)?,
            Uint128::zero(),
        )
    } else {
        (Uint128::zero(), position_debt_value - token_amount)
    };

    let mut msgs: Vec<CosmosMsg> = vec![];
    if refund > Uint128::zero() {
        msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: token_addr.into(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                amount: refund,
                recipient: owner,
            })?,
        }))
    }

    let final_position_debt_share: Uint128 = debt_share_from_value(
        (vault.total_debt_shares - position.debt_share).into(),
        (vault.total_debt - position_debt_value).into(),
        debt_left.into(),
    )
    .try_into()
    .map_err(|_| {
        ContractError::Std(StdError::GenericErr {
            msg: format!("conversion error"),
        })
    })?;

    if final_position_debt_share == Uint128::zero() {
        POSITIONS.remove(deps.storage, position_id.clone());
    } else {
        POSITIONS.update(deps.storage, position_id.clone(), |p| -> StdResult<_> {
            match p {
                None => Err(StdError::NotFound {
                    kind: "position".into(),
                }),
                Some(mut pp) => {
                    pp.debt_share = final_position_debt_share;
                    Ok(pp)
                }
            }
        })?;
    }

    Ok(Response::new().add_messages(msgs).add_attributes(vec![
        ("position_id", position_id),
        ("debt_share", final_position_debt_share.into()),
        ("refund_amount", refund.into()),
    ]))
}

fn accrue_interests(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let vault = VAULT.load(deps.storage)?;
    let total_balance = total_balance(&deps.querier, env.contract.address, &vault)?;
    _accrue_interests(
        deps.storage,
        &vault.last_accrue_timestamp,
        &env.block.time,
        total_balance,
        vault.total_debt,
    )?;
    Ok(Response::new())
}

pub fn _accrue_interests(
    storage: &mut dyn Storage,
    last_accrue_timestamp: &Timestamp,
    current_timestamp: &Timestamp,
    total_balance: Uint128,
    total_debt: Uint128,
) -> StdResult<Vault> {
    let apy = calculate_interest_rate(total_balance, total_debt)?;
    let seconds_since_last: Uint128 =
        (current_timestamp.seconds() - last_accrue_timestamp.seconds()).into();
    let interests = seconds_since_last.multiply_ratio(apy * total_debt, SECONDS_IN_A_YEAR_10000);
    VAULT.update(storage, |mut s| -> StdResult<_> {
        s.last_accrue_timestamp = current_timestamp.clone();
        s.total_debt += interests;
        Ok(s)
    })
}

/**
 *  Simple linear interest rate for simplicity
 *  Returns interest rate in bps (1e4)
 */
fn calculate_interest_rate(total_balance: Uint128, total_debt: Uint128) -> StdResult<Uint128> {
    Ok(total_debt.multiply_ratio(Uint128::from(10000u128), total_balance))
}

fn debt_share_from_value(
    total_debt_share: Uint256,
    total_debt: Uint256,
    value: Uint256,
) -> Uint256 {
    if total_debt_share == Uint256::zero() {
        return value;
    }
    value.multiply_ratio(total_debt_share, total_debt)
}

fn debt_value_from_share(
    total_debt_share: Uint256,
    total_debt: Uint256,
    share: Uint256,
) -> Uint256 {
    if total_debt_share == Uint256::zero() {
        return Uint256::zero();
    }
    share.multiply_ratio(total_debt, total_debt_share)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetVaultConfig {} => to_binary(&(VAULT.load(deps.storage))?),
        QueryMsg::GetPosition { position_id } => to_binary(&query_position(deps, position_id)?),
    }
}

fn query_position(deps: Deps, position_id: String) -> StdResult<Binary> {
    let vault = VAULT.load(deps.storage)?;
    let position = POSITIONS.load(deps.storage, position_id)?;

    let position_debt_value = debt_value_from_share(
        vault.total_debt_shares.into(),
        vault.total_debt.into(),
        position.debt_share.into(),
    );

    let mut result: HashMap<String, String> = HashMap::new();
    result.insert("id".into(), position.id);
    result.insert("debt_share".into(), position.debt_share.into());
    result.insert("farm_addr".into(), position.farm_addr.to_string());
    result.insert("debt_value".into(), position_debt_value.to_string());

    to_binary(&result)
}
