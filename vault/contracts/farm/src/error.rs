use cosmwasm_std::{OverflowError, StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
    #[error("Wrong token sent")]
    WrongToken {},

    #[error("Wrong amount sent: {amount}")]
    WrongAmount { amount: Uint128 },

    #[error("Payment request is not yet paid")]
    Unpaid {},

    #[error("Math error")]
    MathError {},

    #[error("Query error {}")]
    QueryError { kind: String },
}

impl From<OverflowError> for ContractError {
    fn from(o: OverflowError) -> Self {
        StdError::from(o).into()
    }
}
