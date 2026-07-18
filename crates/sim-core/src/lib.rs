#![forbid(unsafe_code)]

pub mod ability_data;
pub mod auto_attack_capabilities;
pub mod basic_combat;
pub mod basic_data;
pub mod combat_data;
pub mod contracts;
pub mod error;
pub mod event_queue;
pub mod rng;
pub mod time;

pub use ability_data::{
    ABILITY_DATA_SCHEMA_VERSION, ABILITY_DATA_VERSION, AQUA_ARROW_HRID,
    DIRECT_DAMAGE_REPORT_VERSION, AbilityDataSnapshotV1, AbilityDataSourceV1, AbilityDataV1,
    AbilityEffectDataV1, DirectDamageAbilityCandidateV1, DirectDamageAbilityCountsV1,
    DirectDamageAbilityIssueV1, DirectDamageAbilityRejectionV1, DirectDamageAbilityReportV1,
    classify_direct_damage_abilities,
};
pub use auto_attack_capabilities::{
    AUTO_ATTACK_CAPABILITY_REPORT_VERSION, AutoAttackCapabilityCountsV1,
    AutoAttackCapabilityIssueV1, AutoAttackCapabilityReportV1, AutoAttackZoneCandidateV1,
    AutoAttackZoneRejectionV1, classify_auto_attack_zones,
};
pub use basic_combat::{
    BASIC_COMBAT_COMPATIBILITY_LEVEL, BASIC_COMBAT_ENGINE_ID, BasicCombatResultV1, simulate_basic,
    simulate_basic_json,
};
pub use basic_data::{
    BASIC_DATA_SCHEMA_VERSION, BASIC_DATA_VERSION, BASIC_FLY_MONSTER_HRID, BASIC_FLY_ZONE_HRID,
    BasicGameData, BasicMonsterData, BasicZoneData,
};
pub use combat_data::{
    COMBAT_ZONE_DATA_SCHEMA_VERSION, COMBAT_ZONE_DATA_VERSION, CombatBossSpawnV1,
    CombatDataCountsV1, CombatDataSourceV1, CombatDataSourcesV1, CombatMonsterDataV1,
    CombatMonsterLevelsV1, CombatRandomSpawnInfoV1, CombatSpawnRuleV1, CombatZoneDataSnapshotV1,
    CombatZoneDataV1,
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
