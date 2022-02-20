use crate::test_utils::TokenQuerier;
use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    from_binary, from_slice, Addr, Coin, Empty, OwnedDeps, Querier, QuerierResult, QueryRequest,
    StdResult, SystemError, WasmQuery,
};
use cw20::Cw20QueryMsg;

pub fn mock_dependencies() -> OwnedDeps<MockStorage, MockApi, CustomMockQuerier> {
    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: CustomMockQuerier::default(),
    }
}

// We do not have any custom query
type CustomQuery = Empty;

pub struct CustomMockQuerier {
    base: MockQuerier<CustomQuery>,
    token_querier: TokenQuerier,
}

impl Default for CustomMockQuerier {
    fn default() -> Self {
        Self {
            base: MockQuerier::<CustomQuery>::new(&[]),
            token_querier: TokenQuerier::default(),
        }
    }
}

impl Querier for CustomMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let request: QueryRequest<CustomQuery> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return Err(SystemError::InvalidRequest {
                    error: format!("[mock]: failed to parse query request {}", e),
                    request: bin_request.into(),
                })
                .into()
            }
        };
        self.handle_query(&request)
    }
}

impl CustomMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<CustomQuery>) -> QuerierResult {
        match request {
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                let contract_addr = Addr::unchecked(contract_addr);

                let parse_pair_query: StdResult<Cw20QueryMsg> = from_binary(msg);
                if let Ok(pair_query) = parse_pair_query {
                    return self.token_querier.handle_query(&contract_addr, pair_query);
                }

                panic!("[mock]: failed to parse wasm query {:?}", msg)
            }

            _ => self.base.handle_query(request),
        }
    }
}
