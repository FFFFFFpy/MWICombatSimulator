use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CONTRACT_VERSION, Result, SimError, SimTime};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatisticsMode {
    #[default]
    Full,
    Fast,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TraceDetailLevel {
    #[default]
    Basic,
    Combat,
}

fn is_basic_trace_detail(value: &TraceDetailLevel) -> bool {
    *value == TraceDetailLevel::Basic
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TraceConfigV1 {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_trace_max_entries")]
    pub max_entries: u64,
    #[serde(default, skip_serializing_if = "is_basic_trace_detail")]
    pub detail_level: TraceDetailLevel,
}

const fn default_trace_max_entries() -> u64 {
    100_000
}

impl Default for TraceConfigV1 {
    fn default() -> Self {
        Self {
            enabled: false,
            max_entries: default_trace_max_entries(),
            detail_level: TraceDetailLevel::Basic,
        }
    }
}

fn empty_object() -> Value {
    Value::Object(Default::default())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationOptionsV1 {
    #[serde(default)]
    pub statistics_mode: StatisticsMode,
    #[serde(default)]
    pub enable_hp_mp_visualization: bool,
    #[serde(default)]
    pub trace: TraceConfigV1,
    #[serde(default = "empty_object")]
    pub extra: Value,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

impl Default for SimulationOptionsV1 {
    fn default() -> Self {
        Self {
            statistics_mode: StatisticsMode::Full,
            enable_hp_mp_visualization: false,
            trace: TraceConfigV1::default(),
            extra: empty_object(),
            extensions: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LabyrinthCrates {
    Count(u32),
    Hrids(Vec<String>),
}

impl Default for LabyrinthCrates {
    fn default() -> Self {
        Self::Hrids(Vec::new())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum SimulationTargetV1 {
    #[serde(rename = "zone")]
    Zone {
        #[serde(rename = "zoneHrid")]
        zone_hrid: String,
        #[serde(rename = "difficultyTier", default)]
        difficulty_tier: u32,
        #[serde(flatten)]
        extensions: BTreeMap<String, Value>,
    },
    #[serde(rename = "labyrinth")]
    Labyrinth {
        #[serde(rename = "labyrinthHrid")]
        labyrinth_hrid: String,
        #[serde(rename = "roomLevel")]
        room_level: u32,
        #[serde(default)]
        crates: LabyrinthCrates,
        #[serde(flatten)]
        extensions: BTreeMap<String, Value>,
    },
}

impl SimulationTargetV1 {
    fn validate(&self) -> Result<()> {
        match self {
            Self::Zone { zone_hrid, .. } if zone_hrid.trim().is_empty() => {
                Err(SimError::InvalidTarget("zoneHrid must not be empty".into()))
            }
            Self::Labyrinth { labyrinth_hrid, .. } if labyrinth_hrid.trim().is_empty() => Err(
                SimError::InvalidTarget("labyrinthHrid must not be empty".into()),
            ),
            Self::Labyrinth { room_level: 0, .. } => Err(SimError::InvalidTarget(
                "roomLevel must be greater than zero".into(),
            )),
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RandomConfigV1 {
    Native,
    Seeded {
        seed: Value,
    },
    Sequence {
        values: Vec<f64>,
        #[serde(rename = "loop", default)]
        loop_values: bool,
    },
}

impl RandomConfigV1 {
    fn validate(&self) -> Result<()> {
        if let Self::Sequence { values, .. } = self {
            if values.is_empty() {
                return Err(SimError::EmptyRandomSequence);
            }
            for value in values {
                if !value.is_finite() || *value < 0.0 || *value >= 1.0 {
                    return Err(SimError::InvalidRandomValue(*value));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationRequestV1 {
    pub contract_version: u32,
    pub request_id: String,
    pub data_version: String,
    pub players: Vec<Value>,
    pub target: SimulationTargetV1,
    pub simulation_time_limit: SimTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub random: Option<RandomConfigV1>,
    #[serde(default)]
    pub options: SimulationOptionsV1,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

impl SimulationRequestV1 {
    pub fn from_json(input: &str) -> Result<Self> {
        let request: Self = serde_json::from_str(input)?;
        request.validate()?;
        Ok(request)
    }

    pub fn to_json(&self) -> Result<String> {
        self.validate()?;
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn validate(&self) -> Result<()> {
        if self.contract_version != CONTRACT_VERSION {
            return Err(SimError::UnsupportedContractVersion {
                received: self.contract_version,
                expected: CONTRACT_VERSION,
            });
        }
        if self.request_id.trim().is_empty() {
            return Err(SimError::EmptyRequestId);
        }
        if self.data_version.trim().is_empty() {
            return Err(SimError::EmptyDataVersion);
        }
        if self.players.is_empty() {
            return Err(SimError::MissingPlayers);
        }
        if self.simulation_time_limit == SimTime::ZERO {
            return Err(SimError::NonPositiveSimulationLimit);
        }
        if self.options.trace.max_entries == 0 {
            return Err(SimError::InvalidTraceConfig(
                "trace.maxEntries must be greater than zero".into(),
            ));
        }
        self.target.validate()?;
        if let Some(random) = &self.random {
            random.validate()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZONE_FIXTURE: &str =
        include_str!("../../../fixtures/parity/zone-solo-basic/request.json");
    const LABYRINTH_FIXTURE: &str =
        include_str!("../../../fixtures/parity/ability-dot-control-sequence/request.json");

    #[test]
    fn parses_zone_and_labyrinth_fixtures_without_parsing_player_internals() {
        let zone = SimulationRequestV1::from_json(ZONE_FIXTURE).unwrap();
        assert_eq!(zone.players.len(), 1);
        assert!(matches!(zone.target, SimulationTargetV1::Zone { .. }));

        let labyrinth = SimulationRequestV1::from_json(LABYRINTH_FIXTURE).unwrap();
        assert!(matches!(
            labyrinth.target,
            SimulationTargetV1::Labyrinth {
                crates: LabyrinthCrates::Hrids(_),
                ..
            }
        ));
        assert!(matches!(
            labyrinth.random,
            Some(RandomConfigV1::Sequence {
                loop_values: true,
                ..
            })
        ));
        assert_eq!(
            labyrinth.options.trace.detail_level,
            TraceDetailLevel::Combat
        );
    }

    #[test]
    fn preserves_unknown_top_level_and_player_fields() {
        let request = SimulationRequestV1::from_json(
            r#"{
                "contractVersion": 1,
                "requestId": "extensions",
                "dataVersion": "test",
                "players": [{"hrid":"player1","futureField":{"x":1}}],
                "target": {"kind":"zone","zoneHrid":"/zone","difficultyTier":0},
                "simulationTimeLimit": 1,
                "options": {},
                "futureTopLevel": [1,2,3]
            }"#,
        )
        .unwrap();

        assert_eq!(request.players[0]["futureField"]["x"], 1);
        assert_eq!(request.extensions["futureTopLevel"][2], 3);

        let encoded = request.to_json().unwrap();
        let round_trip = SimulationRequestV1::from_json(&encoded).unwrap();
        assert_eq!(round_trip.players, request.players);
        assert_eq!(round_trip.extensions, request.extensions);
    }

    #[test]
    fn rejects_invalid_contracts_before_any_engine_exists() {
        let mut request = SimulationRequestV1::from_json(ZONE_FIXTURE).unwrap();
        request.contract_version = 2;
        assert!(matches!(
            request.validate(),
            Err(SimError::UnsupportedContractVersion { received: 2, .. })
        ));

        request.contract_version = CONTRACT_VERSION;
        request.players.clear();
        assert!(matches!(request.validate(), Err(SimError::MissingPlayers)));
    }
}
