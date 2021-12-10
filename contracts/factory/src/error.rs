use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
    #[error("Unknown Reply Id")]
    UnknownReplyId {id: u64},
    #[error("token1 must be juno")]
    Token1MustBeJuno {},
    #[error("Swap for this token already exists")]
    SwapAlreadyExists{},
    #[error("Insantiate swap error")]
    InstatiateSwapError{},
}
