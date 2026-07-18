#![forbid(unsafe_code)]

pub mod contracts;
pub mod error;
pub mod event_queue;
pub mod rng;
pub mod time;

pub use contracts::{
    LabyrinthCrates, RandomConfigV1, SimulationOptionsV1, SimulationRequestV1,
    SimulationTargetV1, StatisticsMode, TraceConfigV1, TraceDetailLevel,
};
pub use error::{Result, SimError};
pub use event_queue::{EventHandle, EventKey, ScheduledEvent, StableEventQueue};
pub use rng::{RandomSource, SeededRandom, SequenceRandom};
pub use time::SimTime;

pub const ENGINE_ID: &str = "rust-core-foundation";
pub const CONTRACT_VERSION: u32 = 1;
