use super::contract::*;
use super::error::*;
use super::msg::*;
use super::state::{POSITIONS, VAULT};

use asset::{Asset, AssetInfo};
use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{coins, from_binary, Addr, DepsMut, Env, StdResult, Timestamp, Uint128, Uint64};

use crate::test_utils::mock_dependencies;

fn init_contract(deps: DepsMut, env: Env) {
    let msg = InstantiateMsg {
        asset_info: AssetInfo::Token {
            contract_addr: "testtoken".into(),
        },
        cw20_code_id: 1,
        reserve_pool_bps: 1000,
        whitelisted_farms: vec![],
    };
    let info = mock_info("creator", &coins(1000, "earth"));

    // we can just call .unwrap() to assert this was a success
    instantiate(deps, env.clone(), info, msg).unwrap();
}

#[test]
fn proper_initialization() {
    let mock_env = mock_env();
    let mut deps = mock_dependencies();
    init_contract(deps.as_mut(), mock_env.clone());
    // it worked, let's query the state
    let vault = VAULT.load(&deps.storage).unwrap();
    assert_eq!(vault.last_accrue_timestamp, mock_env.block.time);
    assert_eq!(vault.last_id, 0u128.into());
}

#[test]
fn accrue_interest() {
    let mock_env = mock_env();
    let mut deps = mock_dependencies();
    init_contract(deps.as_mut(), mock_env);

    VAULT
        .update(deps.as_mut().storage, |mut v| -> StdResult<_> {
            v.total_debt = 500000000u128.into();
            Ok(v)
        })
        .unwrap();
    let new_vault = _accrue_interests(
        deps.as_mut().storage,
        &Timestamp::from_seconds(0),
        &Timestamp::from_seconds(300),
        1000000000u128.into(),
        500000000u128.into(),
    )
    .unwrap();

    assert_eq!(new_vault.total_debt, (500000000u128 + 2376u128).into());
}

#[test]
fn partial_repayment() {
    let mock_env = mock_env();
    let mut deps = mock_dependencies();
    init_contract(deps.as_mut(), mock_env);
    VAULT
        .update(deps.as_mut().storage, |mut v| -> StdResult<_> {
            v.total_debt = 500000000u128.into();
            v.total_debt_shares = 500000000u128.into();
            Ok(v)
        })
        .unwrap();
}
