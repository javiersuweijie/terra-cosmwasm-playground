use asset::{Asset, AssetInfo};
use astroport::asset as astroport_asset;
use astroport::pair::{
    Cw20HookMsg as AstroportHookMsg, ExecuteMsg as AstroportExecuteMsg, QueryMsg as PairQueryMsg,
};
use astroport::querier::{query_pair_info, query_supply, simulate};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, from_binary, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Decimal, Decimal256,
    Deps, DepsMut, Env, MessageInfo, Order, QuerierWrapper, QueryRequest, Reply, ReplyOn, Response,
    StdError, StdResult, SubMsg, SubMsgExecutionResponse, Uint128, Uint256, Uint64, WasmMsg,
    WasmQuery,
};
use cw2::set_contract_version;
use cw20::{
    BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg, MinterResponse,
    TokenInfoResponse,
};
use std::convert::TryInto;
use std::ops::{Add, AddAssign, Mul, Sub};
use std::str::FromStr;
use vault::msg::{
    Cw20HookMsg as VaultCw20HookMsg, ExecuteMsg as VaultExecuteMsg, PositionResponse,
    QueryMsg as VaultQueryMsg,
};
use vault::state::Vault;

use crate::error::ContractError;
use crate::helpers::unwrap_reply;
use crate::isqrt::Isqrt;
use crate::msg::{Cw20HookMsg, Cw20InstantiateMsg, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::response::MsgInstantiateContractResponse;
use crate::state::{Farm, Position, FARM, POSITIONS, TEMP};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:vault-farm";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const SWAP_REPLY_ID: u64 = 1;
const PROVIDE_LIQUIDITY_ID: u64 = 2;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let vault_addr = deps.api.addr_validate(&msg.vault_addr)?;
    let astroport_factory_addr = deps.api.addr_validate(&msg.astroport_factory_addr)?;
    let farm = Farm {
        base_asset: msg.base_asset,
        other_asset: msg.other_asset,
        claim_asset_addr: deps.api.addr_validate(&msg.claim_asset_addr)?,
        vault_addr: vault_addr,
        astroport_factory_addr: astroport_factory_addr,
        total_shares: Uint128::zero(),
    };
    match FARM.save(deps.storage, &farm) {
        Ok(_) => Ok(Response::new()),
        Err(e) => Err(ContractError::Std(e)),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
    match msg.id {
        SWAP_REPLY_ID => handle_swap_reply(deps, _env, unwrap_reply(msg)?),
        PROVIDE_LIQUIDITY_ID => handle_provide_liquidity_reply(deps, _env, unwrap_reply(msg)?),
        id => Err(StdError::generic_err(format!("Unknown reply id: {}", id))),
    }
}

fn handle_swap_reply(
    deps: DepsMut,
    _env: Env,
    msg: SubMsgExecutionResponse,
) -> StdResult<Response> {
    let farm = FARM.load(deps.storage)?;

    let base_asset_string = farm.base_asset.to_string();
    let other_asset_string = farm.other_asset.to_string();

    let swap_event = msg
        .events
        .iter()
        .find(|e| {
            e.attributes
                .iter()
                .any(|attr| attr.key == "action" && attr.value == "swap")
        })
        .ok_or_else(|| StdError::GenericErr {
            msg: format!("unable to find swap action"),
        })?;

    swap_event
        .attributes
        .iter()
        .find(|attr| attr.key == "offer_asset" && attr.value == base_asset_string)
        .ok_or_else(|| StdError::GenericErr {
            msg: format!("unable to find offer asset"),
        })?;

    swap_event
        .attributes
        .iter()
        .find(|attr| attr.key == "ask_asset" && attr.value == other_asset_string)
        .ok_or_else(|| StdError::GenericErr {
            msg: format!("unable to find ask asset"),
        })?;

    let pair_info = query_pair_info(
        &deps.querier,
        farm.astroport_factory_addr,
        &[
            to_astroport_asset(&farm.base_asset),
            to_astroport_asset(&farm.other_asset),
        ],
    )?;

    let base_amount = query_cw20_balance(
        &deps.querier,
        _env.contract.address.to_string(),
        base_asset_string.clone(),
    )?;
    let other_amount = match farm.other_asset.clone() {
        AssetInfo::NativeToken { denom } => {
            deps.querier
                .query_balance(
                    _env.contract.address.to_string(),
                    other_asset_string.clone(),
                )?
                .amount
        }
        AssetInfo::Token { contract_addr } => query_cw20_balance(
            &deps.querier,
            _env.contract.address.to_string(),
            other_asset_string.clone(),
        )?,
    };

    let mut messages: Vec<SubMsg> = vec![];
    messages.push(SubMsg::new(WasmMsg::Execute {
        contract_addr: base_asset_string.clone(),
        funds: vec![],
        msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
            amount: base_amount,
            spender: pair_info.contract_addr.to_string(),
            expires: None,
        })?,
    }));

    let mut funds: Vec<Coin> = vec![];

    match farm.other_asset.clone() {
        AssetInfo::Token { contract_addr: _ } => messages.push(SubMsg::new(WasmMsg::Execute {
            contract_addr: other_asset_string.clone(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                amount: other_amount,
                spender: pair_info.contract_addr.to_string(),
                expires: None,
            })?,
        })),
        AssetInfo::NativeToken { denom } => {
            funds.push(Coin::new(other_amount.into(), denom));
        }
    };

    messages.push(SubMsg::reply_on_success(
        WasmMsg::Execute {
            contract_addr: pair_info.contract_addr.to_string(),
            funds: funds,
            msg: to_binary(&AstroportExecuteMsg::ProvideLiquidity {
                assets: [
                    astroport_asset::Asset {
                        info: to_astroport_asset(&farm.base_asset),
                        amount: base_amount,
                    },
                    astroport_asset::Asset {
                        info: to_astroport_asset(&farm.other_asset),
                        amount: other_amount,
                    },
                ],
                slippage_tolerance: None,
                receiver: None,
                auto_stake: Some(false),
            })?,
        },
        PROVIDE_LIQUIDITY_ID,
    ));

    Ok(Response::new().add_submessages(messages))
}

fn handle_provide_liquidity_reply(
    deps: DepsMut,
    _env: Env,
    msg: SubMsgExecutionResponse,
) -> StdResult<Response> {
    let farm = FARM.load(deps.storage)?;
    let swap_event = msg
        .events
        .iter()
        .find(|e| {
            e.attributes
                .iter()
                .any(|attr| attr.key == "action" && attr.value == "provide_liquidity")
        })
        .ok_or_else(|| StdError::generic_err(format!("unable to find provide_liquidity action")))?;

    let lp_tokens = Uint128::from_str(
        &swap_event
            .attributes
            .iter()
            .find(|attr| attr.key == "share")
            .ok_or_else(|| StdError::GenericErr {
                msg: format!("unable to find LP share"),
            })?
            .value,
    )?;

    let mut position = TEMP.load(deps.storage)?;
    let total_lp_balance = query_cw20_balance(
        &deps.querier,
        _env.contract.address.to_string(),
        farm.claim_asset_addr.to_string(),
    )?;
    let shares = share_from_value(
        farm.total_shares.into(),
        (total_lp_balance - lp_tokens).into(),
        lp_tokens.into(),
    );

    position.shares = shares.try_into()?;
    let vault_position_id = position.vault_position_id.clone();
    POSITIONS.save(deps.storage, vault_position_id.clone(), &position)?;
    TEMP.remove(deps.storage);
    Ok(Response::new().add_attributes(vec![
        ("vault_position_id", vault_position_id),
        ("shares", shares.into()),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => Ok(Response::new()),
        ExecuteMsg::Open {
            borrow_amount,
            base_asset_amount,
        } => open_position(deps, env, info, base_asset_amount, borrow_amount),
        ExecuteMsg::Close {} => Ok(Response::new()),
    }
}

pub fn open_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    base_asset_amount: Uint128,
    borrow_amount: Uint128,
) -> Result<Response, ContractError> {
    let farm = FARM.load(deps.storage)?;
    // TODO: Validate tokens
    // 1. Borrow amount from vault (append message)
    let vault: Vault = deps
        .querier
        .query_wasm_smart(farm.vault_addr.clone(), &VaultQueryMsg::GetVaultConfig {})
        .or_else(|_| {
            Err(StdError::generic_err(format!(
                "vault not found in {}",
                farm.vault_addr.clone()
            )))
        })?;

    let position_id = vault.last_id + Uint128::from(1u128);
    let sender = info.sender;

    let position = Position {
        vault_position_id: position_id.to_string(),
        owner: sender.clone(),
        shares: Uint128::zero(),
    };

    TEMP.save(deps.storage, &position)?;

    let mut messages: Vec<SubMsg> = vec![];
    match farm.base_asset.clone() {
        AssetInfo::NativeToken { denom } => {
            let amount = info
                .funds
                .iter()
                .find(|f| f.denom == denom)
                .map(|c| c.amount)
                .ok_or_else(|| {
                    StdError::generic_err(format!("error getting token deposit for {}", denom))
                })?;
            if base_asset_amount != amount {
                return Err(ContractError::Std(StdError::generic_err(format!(
                    "deposit amount different from expected {} vs {}",
                    amount, base_asset_amount
                ))));
            }
        }
        AssetInfo::Token {
            contract_addr: token_contract_addr,
        } => messages.push(SubMsg::new(WasmMsg::Execute {
            contract_addr: token_contract_addr,
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                amount: base_asset_amount,
                owner: sender.to_string().clone(),
                recipient: env.contract.address.to_string(),
            })?,
        })),
    }
    messages.push(SubMsg::new(WasmMsg::Execute {
        contract_addr: farm.vault_addr.into(),
        msg: to_binary(&VaultExecuteMsg::Borrow {
            borrow_amount: borrow_amount,
        })?,
        funds: vec![],
    }));

    // 2. Calculate how much to swap and final amounts using simulation
    // Taking 0.3% fees into account
    let total_base_amount = base_asset_amount + borrow_amount;
    // Fancy way of dividing base amount into half with fees included
    let base_amount_to_swap = total_base_amount.multiply_ratio(1000u128, 1997u128);
    // let base_amount_to_provide = total_base_amount - base_amount_to_swap;

    let astro_base_asset = to_astroport_asset(&farm.base_asset);
    let astro_other_asset = to_astroport_asset(&farm.other_asset);

    let pair_info = query_pair_info(
        &deps.querier,
        farm.astroport_factory_addr,
        &[astro_base_asset.clone(), astro_other_asset.clone()],
    )?;

    // How much of the other token we are getting back
    let other_token = simulate(
        &deps.querier,
        pair_info.contract_addr.clone(),
        &astroport_asset::Asset {
            info: astro_base_asset.clone(),
            amount: base_amount_to_swap,
        },
    )?;

    match farm.base_asset.clone() {
        AssetInfo::NativeToken { denom } => Err(ContractError::Std(StdError::GenericErr {
            msg: format!("{} not supported", denom),
        })),
        AssetInfo::Token {
            contract_addr: base_asset_addr,
        } => {
            // Execute swap
            messages.push(SubMsg::reply_on_success(
                WasmMsg::Execute {
                    contract_addr: base_asset_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Send {
                        amount: base_amount_to_swap,
                        contract: pair_info.contract_addr.to_string(),
                        msg: to_binary(&AstroportHookMsg::Swap {
                            max_spread: None,
                            belief_price: None,
                            to: None,
                        })?,
                    })?,
                    funds: vec![],
                },
                SWAP_REPLY_ID,
            ));
            return Ok(Response::new().add_submessages(messages));
        }
    }
}

fn to_astroport_asset(asset_info: &AssetInfo) -> astroport_asset::AssetInfo {
    match asset_info {
        AssetInfo::NativeToken { denom } => astroport_asset::AssetInfo::NativeToken {
            denom: denom.into(),
        },
        AssetInfo::Token { contract_addr } => astroport_asset::AssetInfo::Token {
            contract_addr: Addr::unchecked(contract_addr),
        },
    }
}

pub fn close_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    position_id: String,
) -> Result<Response, ContractError> {
    let farm = FARM.load(deps.storage)?;
    let position = POSITIONS.load(deps.storage, position_id)?;

    // Validations
    if position.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Calculate how much LP is owned by the position
    let total_lp_balance = query_cw20_balance(
        &deps.querier,
        env.contract.address.to_string(),
        farm.claim_asset_addr.to_string(),
    )
    .map_err(|_| ContractError::QueryError {
        kind: "query for lp balance".into(),
    })?;
    let position_lp_balance: Uint128 = value_from_share(
        farm.total_shares.into(),
        total_lp_balance.into(),
        position.shares.into(),
    )
    .try_into()
    .map_err(|_| ContractError::MathError {})?;

    // Check how much to repay
    let vault_position: PositionResponse = deps.querier.query_wasm_smart(
        farm.vault_addr.clone(),
        &VaultQueryMsg::GetPosition {
            position_id: position.vault_position_id.clone(),
        },
    )?;

    let repay_amount = vault_position.debt_value;

    let astro_base_asset = to_astroport_asset(&farm.base_asset);
    let astro_other_asset = to_astroport_asset(&farm.other_asset);

    let pair_info = query_pair_info(
        &deps.querier,
        farm.astroport_factory_addr,
        &[astro_base_asset.clone(), astro_other_asset.clone()],
    )?;
    let assets_in_pool: Vec<astroport_asset::Asset> = deps.querier.query_wasm_smart(
        pair_info.contract_addr.clone(),
        &PairQueryMsg::Share {
            amount: position_lp_balance,
        },
    )?;

    let base_asset_amount = assets_in_pool
        .iter()
        .find(|a| a.info == astro_base_asset)
        .ok_or_else(|| ContractError::WrongToken {})?
        .amount;

    let other_asset_amount = assets_in_pool
        .iter()
        .find(|a| a.info == astro_other_asset)
        .ok_or_else(|| ContractError::WrongToken {})?
        .amount;

    let simluated_swap_amount = simulate(
        &deps.querier,
        pair_info.contract_addr.clone(),
        &astroport_asset::Asset {
            info: astro_other_asset,
            amount: other_asset_amount,
        },
    )?
    .return_amount;

    let total_base_amount = simluated_swap_amount + base_asset_amount;
    if total_base_amount < repay_amount {
        return Err(ContractError::WrongAmount {
            amount: repay_amount - total_base_amount,
        });
    }

    /*
     */

    let mut msgs: Vec<CosmosMsg> = vec![];

    // Close LP and get base token + other token (message)
    msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: farm.claim_asset_addr.to_string(),
        funds: vec![],
        msg: to_binary(&Cw20ExecuteMsg::Send {
            amount: position_lp_balance,
            contract: pair_info.contract_addr.to_string(),
            msg: to_binary(&AstroportHookMsg::WithdrawLiquidity {})?,
        })?,
    }));

    // Swap to get repay amount
    match astro_other_asset {
        astroport_asset::AssetInfo::NativeToken { denom } => {
            msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: pair_info.contract_addr.to_string(),
                funds: coins(other_asset_amount.into(), denom),
                msg: to_binary(&AstroportExecuteMsg::Swap {
                    offer_asset: astroport_asset::Asset {
                        info: astro_other_asset,
                        amount: other_asset_amount,
                    },
                    belief_price: None,
                    max_spread: None,
                    to: None,
                })?,
            }))
        }
        astroport_asset::AssetInfo::Token { contract_addr } => {
            msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    amount: other_asset_amount,
                    contract: pair_info.contract_addr.to_string(),
                    msg: to_binary(&AstroportHookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: None,
                    })?,
                })?,
            }))
        }
    }

    match astro_base_asset {
        astroport_asset::AssetInfo::NativeToken { denom: _ } => {
            return Err(ContractError::WrongToken {});
        }
        astroport_asset::AssetInfo::Token { contract_addr } => {
            // Repay debt (message)
            msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    amount: repay_amount,
                    contract: farm.vault_addr.to_string(),
                    msg: to_binary(&VaultCw20HookMsg::Repay {
                        position_id: position.vault_position_id,
                    })?,
                })?,
            }));
            //  Send the rest back to sender (message)
            msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    amount: total_base_amount - repay_amount,
                    recipient: position.owner.to_string(),
                })?,
            }))
        }
    }

    Ok(Response::new().add_messages(msgs).add_attributes(vec![
        ("owner", position.owner.to_string()),
        ("repay_amount", repay_amount.to_string()),
        (
            "return_amount",
            (total_base_amount - repay_amount).to_string(),
        ),
    ]))
}

fn share_from_value(total_share: Uint256, total_value: Uint256, value: Uint256) -> Uint256 {
    if total_share == Uint256::zero() {
        return value;
    }
    value.multiply_ratio(total_share, total_value)
}

fn value_from_share(total_share: Uint256, total_value: Uint256, share: Uint256) -> Uint256 {
    if total_share == Uint256::zero() {
        return Uint256::zero();
    }
    share.multiply_ratio(total_value, total_share)
}

pub fn kill(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    position_id: String,
) -> Result<Response, ContractError> {
    let positon = POSITIONS.load(deps.storage, position_id)?;
    Ok(Response::default())
}

fn query_total_vault_shares(
    querier: &QuerierWrapper,
    token_contract_addr: &Addr,
) -> StdResult<Uint128> {
    let token_info: TokenInfoResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: token_contract_addr.to_string(),
        msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
    }))?;
    Ok(token_info.total_supply)
}

fn query_cw20_balance(
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

fn total_balance(
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

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetPosition { vault_position_id } => {
            to_binary(&(POSITIONS.load(deps.storage, vault_position_id))?)
        }
        QueryMsg::GetFarm {} => to_binary(&(FARM.load(deps.storage))?),
    }
}
