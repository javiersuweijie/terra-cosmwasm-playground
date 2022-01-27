#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Addr, StdError, Order};
use cw_storage_plus::Item;
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, Move, GameStateResponse, GameStatesResponse};
use crate::state::{GameState, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:sps";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::new()
        .add_attribute("method", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::StartGame { action, opponent } => start_game(deps, info, action, opponent),
        ExecuteMsg::OpponentMove { action, host } => opponent_move(deps, info, action, host),
    }
}

pub fn start_game(deps: DepsMut, info: MessageInfo, action: Move, opponent: Addr) -> Result<Response, ContractError> {
    let host = info.sender;
    let game_state = STATE.may_load(deps.storage, (host.clone(), opponent.clone()))?;
    if game_state != None {
        return Err(ContractError::GameAlreadyExists {});
    }

    let new_game_state = GameState {
        host: host.clone(),
        opponent: opponent.clone(),
        host_move: action,
    };
    STATE.save(deps.storage, (host.clone(), opponent.clone()), &new_game_state)?;
    let res = Response::new()
    .add_attribute("host", host)
    .add_attribute("opponent", opponent);
    Ok(res)
}

pub fn opponent_move(deps: DepsMut, info: MessageInfo, action: Move, host: Addr) -> Result<Response, ContractError> {
    let opponent = info.sender;
    let state_path = STATE.key((host.clone(), opponent.clone()));
    let game_state = state_path.may_load(deps.storage)?;
    match game_state {
        None => Err(ContractError::GameNotFound{}),
        Some(game_state) => {
            let game_result = resolve_game(game_state.host_move, action);
            let res = Response::new();
            match game_result {
                0 => {
                    state_path.remove(deps.storage);
                    Ok(res.add_attribute("result", "done").add_attribute("winner", game_state.host))
                },
                1 => {
                    state_path.remove(deps.storage);
                    Ok(res.add_attribute("result", "done").add_attribute("winner", opponent))
                },
                _ => Ok(res.add_attribute("result", "draw")),
            }
        }
    }
}

fn resolve_game(move0: Move, move1: Move) -> i8 {
    match move0 {
        Move::Scissors {} => {
            match move1 {
                Move::Scissors {} => -1,
                Move::Paper {} => 0,
                Move::Stone {} => 1,
            }
        },
        Move::Paper {} => {
            match move1 {
                Move::Scissors {} => 1,
                Move::Paper {} => -1,
                Move::Stone {} => 0,
            }
        },
        Move::Stone {} => {
            match move1 {
                Move::Scissors {} => 0,
                Move::Paper {} => 1,
                Move::Stone {} => -1,
            }
        },
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetGameStatesByHost {host} => to_binary(&query_game_states_by_host(deps, host)?),
    }
}

fn query_game_states_by_host(deps: Deps, host: Addr) -> StdResult<GameStatesResponse> {
    let games: StdResult<Vec<_>> = STATE
        .prefix(host)
        .range(deps.storage, None, None, Order::Ascending)
        .collect();
    match games {
        Err(_) => Err(StdError::GenericErr{
            msg: String::from("Unknown error when querying for games")
        }),
        Ok(gs) => {
            let games: Vec<GameStateResponse> = gs.iter().map(|g| GameStateResponse{
                host: g.1.host.clone(),
                opponent: g.1.opponent.clone(),
            }).collect();
            Ok(GameStatesResponse {
                games: games
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetGameStatesByHost {host: Addr::unchecked("creator")}).unwrap();

        let value: GameStatesResponse = from_binary(&res).unwrap();
        assert_eq!(value.games.len(), 0);
    }

    #[test]
    fn start_game() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let auth_info = mock_info("host", &coins(1000, "paper"));
        let res = execute(
            deps.as_mut(), 
            mock_env(), 
            auth_info,
            ExecuteMsg::StartGame {opponent: Addr::unchecked("enemy"), action: Move::Paper{} }
        ).unwrap();

        assert_eq!("host", res.attributes[0].value);
        assert_eq!("enemy", res.attributes[1].value);

        let auth_info_2 = mock_info("host", &coins(1000, "paper"));
        let res = execute(
            deps.as_mut(), 
            mock_env(), 
            auth_info_2,
            ExecuteMsg::StartGame {opponent: Addr::unchecked("enemy2"), action: Move::Paper{} }
        ).unwrap();

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetGameStatesByHost {
            host: Addr::unchecked("host")
        }).unwrap();

        let value: GameStatesResponse = from_binary(&res).unwrap();
        assert_eq!(2, value.games.len());
        let first = value.games.get(0).unwrap();
        assert_eq!("host", first.host);
        assert_eq!("enemy", first.opponent);
    }

    #[test]
    fn opponent_move() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let auth_info = mock_info("host", &coins(1000, "paper"));
        execute(
            deps.as_mut(), 
            mock_env(), 
            auth_info,
            ExecuteMsg::StartGame {opponent: Addr::unchecked("enemy"), action: Move::Paper{} }
        ).unwrap();

        let opp_info = mock_info("enemy", &coins(1000, "scissors"));
        let res = execute(deps.as_mut(), mock_env(), opp_info, ExecuteMsg::OpponentMove {
            host: Addr::unchecked("host"),
            action: Move::Scissors{},
        }).unwrap();

        assert_eq!("done", res.attributes[0].value);
        assert_eq!("enemy", res.attributes[1].value);

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetGameStatesByHost {host: Addr::unchecked("creator")}).unwrap();
        let value: GameStatesResponse = from_binary(&res).unwrap();
        assert_eq!(value.games.len(), 0);
    }

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
}
