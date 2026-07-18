#![forbid(unsafe_code)]

pub mod basic_combat;
pub mod basic_data;
pub mod contracts;
pub mod error;
pub mod event_queue;
pub mod rng;
pub mod time;

pub use basic_combat::{
    BASIC_COMBAT_COMPATIBILITY_LEVEL, BASIC_COMBAT_ENGINE_ID, BasicCombatResultV1,
    simulate_basic, simulate_basic_json,
};
pub use basic_data::{
    BASIC_DATA_SCHEMA_VERSION, BASIC_DATA_VERSION, BASIC_FLY_MONSTER_HRID, BASIC_FLY_ZONE_HRID,
    BasicGameData, BasicMonsterData, BasicZoneData,
};
pub use contracts::{
    LabyrinthCrates, RandomConfigV1, SimulationOptionsV1, SimulationRequestV1, SimulationTargetV1,
    StatisticsMode, TraceConfigV1, TraceDetailLevel,
};
pub use error::{Result, SimError};
pub use event_queue::{EventHandle, EventKey, ScheduledEvent, StableEventQueue};
pub use rng::{RandomSource, SeededRandom, SequenceRandom};
pub use time::SimTime;

pub const ENGINE_ID: &str = "rust-core-foundation";
pub const CONTRACT_VERSION: u32 = 1;
