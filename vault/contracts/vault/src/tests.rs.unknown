use super::contract::*;
use super::msg::*;
use super::error::*;
use super::state::{STATE, PAYMENT_REQUESTS};

use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{coins, from_binary, Addr, Uint64, Uint128};
use asset::{ Asset, AssetInfo };

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&coins(2, "token"));

    let msg = InstantiateMsg {
        shop: Addr::unchecked("shop"),
    };
    let info = mock_info("creator", &coins(1000, "earth"));

    // we can just call .unwrap() to assert this was a success
    let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let state = STATE.load(&deps.storage).unwrap();
    assert_eq!(state.shop, "shop");
    assert_eq!(state.last_id, Uint64::from(0 as u64));
    // let res = query(deps.as_ref(), mock_env(), QueryMsg:: {host: Addr::unchecked("creator")}).unwrap();

    // let value: GameStatesResponse = from_binary(&res).unwrap();
    // assert_eq!(value.games.len(), 0);
}

#[test]
fn create_payment_request() {
    let mut deps = mock_dependencies(&coins(2, "token"));

    let msg = InstantiateMsg {
        shop: Addr::unchecked("shop"),
    };
    let info = mock_info("creator", &coins(1000, "earth"));

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    let auth_info = mock_info("host", &coins(1000, "paper"));
    let asset: Asset = Asset {
        info: AssetInfo::NativeToken{ denom: String::from("uluna") },
        amount: Uint128::from(1_000_000 as u128),
    };
    let _res = execute(
        deps.as_mut(), 
        mock_env(), 
        auth_info,
        ExecuteMsg::CreatePaymentRequest {asset: asset, order_id: String::from("1")}
    ).unwrap();

    assert_eq!(PAYMENT_REQUESTS.has(&deps.storage, String::from("1")), true);
}

#[test]
fn pay_into_payment_request() {
    let mut deps = mock_dependencies(&coins(2, "token"));

    let msg = InstantiateMsg {
        shop: Addr::unchecked("shop"),
    };
    let info = mock_info("creator", &coins(1000, "earth"));

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    let auth_info = mock_info("host", &coins(1000, "paper"));
    let asset: Asset = Asset {
        info: AssetInfo::NativeToken{ denom: String::from("uluna") },
        amount: Uint128::from(1_000_000 as u128),
    };
    let _res = execute(
        deps.as_mut(), 
        mock_env(), 
        auth_info.clone(),
        ExecuteMsg::CreatePaymentRequest {asset: asset, order_id: String::from("1")}
    ).unwrap();

    assert_eq!(PAYMENT_REQUESTS.has(&deps.storage, String::from("1")), true);

    let pay_info = mock_info("customer", &coins(1_000_000, "uluna"));
    let _res = execute(
        deps.as_mut(),
        mock_env(),
        pay_info,
        ExecuteMsg::PayIntoPaymentRequest {id: String::from("1")}
    );

    let pr = PAYMENT_REQUESTS.load(&deps.storage, String::from("1")).unwrap();
    assert_eq!(pr.customer, "customer");
    assert_eq!(pr.paid_amount, Uint128::from(1_000_000 as u128));
}

#[test]
fn pay_into_payment_request_with_wrong_amount() {
    let mut deps = mock_dependencies(&coins(2, "token"));

    let msg = InstantiateMsg {
        shop: Addr::unchecked("shop"),
    };
    let info = mock_info("creator", &coins(1000, "earth"));

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    let auth_info = mock_info("host", &coins(1000, "paper"));
    let asset: Asset = Asset {
        info: AssetInfo::NativeToken{ denom: String::from("uluna") },
        amount: Uint128::from(1_000_000 as u128),
    };
    let _res = execute(
        deps.as_mut(), 
        mock_env(), 
        auth_info.clone(),
        ExecuteMsg::CreatePaymentRequest {asset: asset, order_id: String::from("1")}
    ).unwrap();

    assert_eq!(PAYMENT_REQUESTS.has(&deps.storage, String::from("1")), true);

    let pay_info = mock_info("customer", &coins(1_000_00, "uluna"));
    let res = execute(
        deps.as_mut(),
        mock_env(),
        pay_info,
        ExecuteMsg::PayIntoPaymentRequest {id: String::from("1")}
    ).unwrap_err();
    match res {
        ContractError::WrongAmount { amount: _ } => {},
        _ => panic!("Must return wrong amount error"),
    }
}

#[test]
fn pay_into_payment_request_with_wrong_token() {
    let mut deps = mock_dependencies(&coins(2, "token"));

    let msg = InstantiateMsg {
        shop: Addr::unchecked("shop"),
    };
    let info = mock_info("creator", &coins(1000, "earth"));

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    let auth_info = mock_info("host", &coins(1000, "paper"));
    let asset: Asset = Asset {
        info: AssetInfo::NativeToken{ denom: String::from("uluna") },
        amount: Uint128::from(1_000_000 as u128),
    };
    let _res = execute(
        deps.as_mut(), 
        mock_env(), 
        auth_info.clone(),
        ExecuteMsg::CreatePaymentRequest {asset: asset, order_id: String::from("1")}
    ).unwrap();

    assert_eq!(PAYMENT_REQUESTS.has(&deps.storage, String::from("1")), true);

    let pay_info = mock_info("customer", &coins(1_000_000, "paper"));
    let res = execute(
        deps.as_mut(),
        mock_env(),
        pay_info,
        ExecuteMsg::PayIntoPaymentRequest {id: String::from("1")}
    ).unwrap_err();
    match res {
        ContractError::WrongToken {} => {},
        _ => panic!("Must return wrong token error"),
    }
}

// #[test]
// fn opponent_move() {
//     let mut deps = mock_dependencies(&coins(2, "token"));

//     let msg = InstantiateMsg {};
//     let info = mock_info("creator", &coins(1000, "earth"));

//     // we can just call .unwrap() to assert this was a success
//     let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
//     assert_eq!(0, res.messages.len());

//     let auth_info = mock_info("host", &coins(1000, "paper"));
//     execute(
//         deps.as_mut(), 
//         mock_env(), 
//         auth_info,
//         ExecuteMsg::StartGame {opponent: Addr::unchecked("enemy"), action: Move::Paper{} }
//     ).unwrap();

//     let opp_info = mock_info("enemy", &coins(1000, "scissors"));
//     let res = execute(deps.as_mut(), mock_env(), opp_info, ExecuteMsg::OpponentMove {
//         host: Addr::unchecked("host"),
//         action: Move::Scissors{},
//     }).unwrap();

//     assert_eq!("done", res.attributes[0].value);
//     assert_eq!("enemy", res.attributes[1].value);

//     let res = query(deps.as_ref(), mock_env(), QueryMsg::GetGameStatesByHost {host: Addr::unchecked("creator")}).unwrap();
//     let value: GameStatesResponse = from_binary(&res).unwrap();
//     assert_eq!(value.games.len(), 0);
// }

// #[test]
// fn reset() {
//     let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

//     let msg = InstantiateMsg { count: 17 };
//     let info = mock_info("creator", &coins(2, "token"));
//     let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

//     // beneficiary can release it
//     let unauth_info = mock_info("anyone", &coins(2, "token"));
//     let msg = ExecuteMsg::Reset { count: 5 };
//     let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
//     match res {
//         Err(ContractError::Unauthorized {}) => {}
//         _ => panic!("Must return unauthorized error"),
//     }

//     // only the original creator can reset the counter
//     let auth_info = mock_info("creator", &coins(2, "token"));
//     let msg = ExecuteMsg::Reset { count: 5 };
//     let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

//     // should now be 5
//     let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
//     let value: CountResponse = from_binary(&res).unwrap();
//     assert_eq!(5, value.count);
// }

// #[test]
// fn change_owner() {
//     let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

//     let msg = InstantiateMsg { count: 17 };
//     let info = mock_info("creator", &coins(2, "token"));
//     let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

//     // beneficiary can release it
//     let unauth_info = mock_info("anyone", &coins(2, "token"));
//     let msg = ExecuteMsg::ChangeOwner { owner: Addr::unchecked("new_owner") };
//     let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
//     match res {
//         Err(ContractError::Unauthorized {}) => {}
//         _ => panic!("Must return unauthorized error"),
//     }

//     // only the original creator can reset the counter
//     let auth_info = mock_info("creator", &coins(2, "token"));
//     let msg = ExecuteMsg::ChangeOwner { owner: Addr::unchecked("new_owner") };
//     let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();
//     let owner = &_res.attributes[1].value;
//     assert_eq!("new_owner", owner);

//     // should now be 5
//     let res = query(deps.as_ref(), mock_env(), QueryMsg::GetOwner{}).unwrap();
//     let value: OwnerResponse= from_binary(&res).unwrap();
//     assert_eq!("new_owner", value.owner);
// }