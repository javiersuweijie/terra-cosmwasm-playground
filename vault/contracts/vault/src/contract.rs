use asset::{Asset, AssetInfo};
use cosmwasm_bignumber::Uint256;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, from_binary, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Order, QuerierWrapper, QueryRequest, Reply, ReplyOn, Response, StdError,
    StdResult, SubMsg, Uint128, Uint64, WasmMsg, WasmQuery,
};
use cw2::set_contract_version;
use cw20::{
    BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg, MinterResponse,
    TokenInfoResponse,
};
use protobuf::Message;
use std::ops::{Add, AddAssign};

use crate::error::ContractError;
use crate::msg::{Cw20HookMsg, Cw20InstantiateMsg, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::response::MsgInstantiateContractResponse;
use crate::state::{Position, Vault, POSITIONS, VAULT};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:sps";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const INSTANTIATE_REPLY_ID: u64 = 1;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let vault = Vault {
        // Todo: validate the shop contract
        last_id: Uint128::zero(),
        asset_info: msg.asset_info,
        reserve_pool_bps: msg.reserve_pool_bps,
        total_debt: Uint128::zero(),
        total_debt_shares: Uint128::zero(),
        reserve_pool: Uint128::zero(),
        whitelisted_workers: vec![],
        vault_token_addr: Addr::unchecked(String::default()),
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
    let data = msg.result.unwrap().data.unwrap();
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
        ExecuteMsg::Kill { position_id } => Err(ContractError::Unauthorized {}),
    }
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
            Cw20HookMsg::Borrow {
                worker_addr,
                borrow_amount,
            } => cw20_borrow(deps, env, info, msg, worker_addr, borrow_amount),
            _ => Err(ContractError::Std(StdError::GenericErr {
                msg: String::from("unknown hook"),
            })),
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

pub fn cw20_borrow(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: Cw20ReceiveMsg,
    worker_addr: Addr,
    borrow_amount: Uint128,
) -> Result<Response, ContractError> {
    let asset = Asset {
        info: AssetInfo::Token {
            contract_addr: info.sender.to_string(),
        },
        amount: msg.amount,
    };
    let sender: Addr = deps.api.addr_validate(msg.sender.as_str())?;
    borrow(deps, env, asset, sender, worker_addr, borrow_amount)
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

    let total_vault_shares: Uint256 = query_total_vault_shares(&deps.querier, &vault_token)?.into();
    let total_balance: Uint256 = total_balance(&deps.querier, env.contract.address, &vault)?.into();

    let tokens_to_withdraw: Uint128 = amount
        .multiply_ratio(total_balance, total_vault_shares)
        .into();
    let burn_vault_token_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: vault.vault_token_addr.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Burn {
            amount: amount.into(),
        })?,
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
    // TODO: accure interest

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
        let total_token_balance: Uint256 =
            total_balance(&deps.querier, env.contract.address, &vault)?
                .checked_sub(asset.amount)?
                .into();
        let amount_256: Uint256 = asset.amount.into();
        vault_tokens_to_mint = amount_256
            .multiply_ratio(total_vault_shares, total_token_balance)
            .into();
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
    asset: Asset,
    sender: Addr,
    farm_addr: Addr,
    borrow_amount: Uint128,
) -> Result<Response, ContractError> {
    let vault = VAULT.load(deps.storage)?;
    if vault.asset_info != asset.info {
        return Err(ContractError::WrongToken {});
    }

    // Hardcoded such that you can only borrow 2x your principal amount
    if asset.amount.checked_mul(Uint128::from(2 as u128))? < borrow_amount {
        return Err(ContractError::WrongAmount {
            amount: borrow_amount,
        });
    }

    let debt_share: Uint128 = debt_share_from_value(
        vault.total_debt_shares.into(),
        vault.total_debt.into(),
        borrow_amount.into(),
    )
    .into();

    let position = Position {
        id: vault.last_id.to_string(),
        owner: sender.clone(),
        farm: farm_addr.clone(),
        debt_share: debt_share,
    };

    VAULT.update(deps.storage, |mut v| -> StdResult<_> {
        v.last_id.add_assign(Uint128::from(1 as u128));
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
        AssetInfo::Token { contract_addr } => Ok(Response::new()
            .add_message(CosmosMsg::Wasm(
                // Deposit funds into another contract
                // TODO: handle refunds
                WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Send {
                        amount: borrow_amount.checked_add(asset.amount)?,
                        contract: farm_addr.to_string(),
                        msg: to_binary(&Cw20HookMsg::Deposit {})?,
                    })?,
                    funds: vec![],
                },
            ))
            .add_attribute("position_id", position.id.clone())),
    }
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
        QueryMsg::GetPosition { position_id } => {
            to_binary(&(POSITIONS.load(deps.storage, position_id))?)
        }
    }
}
