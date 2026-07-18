use thiserror::Error;

pub type Result<T> = std::result::Result<T, SimError>;

#[derive(Debug, Error, PartialEq)]
pub enum SimError {
    #[error("unsupported simulation contract version: {received}; expected {expected}")]
    UnsupportedContractVersion { received: u32, expected: u32 },

    #[error("simulation request requires at least one player")]
    MissingPlayers,

    #[error("requestId must not be empty")]
    EmptyRequestId,

    #[error("dataVersion must not be empty")]
    EmptyDataVersion,

    #[error("simulation time must be finite and non-negative; received {0}")]
    InvalidTime(f64),

    #[error("simulationTimeLimit must be greater than zero")]
    NonPositiveSimulationLimit,

    #[error("random values must be finite numbers in [0, 1); received {0}")]
    InvalidRandomValue(f64),

    #[error("random sequence requires at least one value")]
    EmptyRandomSequence,

    #[error("random sequence exhausted after {draws} draws")]
    RandomSequenceExhausted { draws: u64 },

    #[error("event handle is stale or already cancelled")]
    StaleEventHandle,

    #[error("invalid JSON request: {0}")]
    InvalidRequestJson(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
