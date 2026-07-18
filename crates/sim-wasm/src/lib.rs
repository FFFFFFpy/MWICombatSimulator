#![forbid(unsafe_code)]

use mwi_sim_core::{
    BASIC_COMBAT_COMPATIBILITY_LEVEL, BASIC_COMBAT_ENGINE_ID, BASIC_FLY_ZONE_HRID,
    CONTRACT_VERSION, ENGINE_ID, SimulationRequestV1, simulate_basic_json,
};

pub fn capabilities_json() -> String {
    serde_json::json!({
        "engine": BASIC_COMBAT_ENGINE_ID,
        "foundationEngine": ENGINE_ID,
        "engineVersion": env!("CARGO_PKG_VERSION"),
        "contractVersion": CONTRACT_VERSION,
        "implemented": [
            "contracts",
            "sequence-rng",
            "seeded-rng-string",
            "stable-event-queue",
            "basic-auto-attack"
        ],
        "combatSimulation": true,
        "fullSimulationResult": false,
        "resultTypes": ["basic_combat_result"],
        "compatibilityLevels": [BASIC_COMBAT_COMPATIBILITY_LEVEL],
        "targets": [{
            "kind": "zone",
            "zoneHrids": [BASIC_FLY_ZONE_HRID],
            "difficultyTiers": [0],
            "players": { "minimum": 1, "maximum": 1 }
        }],
        "statisticsModes": [],
        "eventTrace": false,
        "nativeBatch": false
    })
    .to_string()
}

pub fn validate_request_json(input: &str) -> Result<String, String> {
    let request = SimulationRequestV1::from_json(input).map_err(|error| error.to_string())?;
    request.to_json().map_err(|error| error.to_string())
}

pub fn simulate_basic_request_json(input: &str) -> Result<String, String> {
    simulate_basic_json(input).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
mod wasm_exports {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(js_name = capabilitiesJson)]
    pub fn capabilities_json() -> String {
        super::capabilities_json()
    }

    #[wasm_bindgen(js_name = validateRequestJson)]
    pub fn validate_request_json(input: &str) -> Result<String, JsValue> {
        super::validate_request_json(input).map_err(|message| JsValue::from_str(&message))
    }

    #[wasm_bindgen(js_name = simulateBasicJson)]
    pub fn simulate_basic_json(input: &str) -> Result<String, JsValue> {
        super::simulate_basic_request_json(input).map_err(|message| JsValue::from_str(&message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_only_the_restricted_basic_combat_capability() {
        let capabilities: serde_json::Value = serde_json::from_str(&capabilities_json()).unwrap();
        assert_eq!(capabilities["combatSimulation"], true);
        assert_eq!(capabilities["fullSimulationResult"], false);
        assert_eq!(capabilities["contractVersion"], CONTRACT_VERSION);
        assert_eq!(capabilities["targets"][0]["zoneHrids"][0], BASIC_FLY_ZONE_HRID);
    }

    #[test]
    fn validates_and_simulates_the_basic_fixture() {
        let request = include_str!("../../../fixtures/parity/zone-solo-basic/request.json");
        let normalized = validate_request_json(request).unwrap();
        let value: serde_json::Value = serde_json::from_str(&normalized).unwrap();
        assert_eq!(value["requestId"], "parity-zone-solo-basic-v1");

        let result = simulate_basic_request_json(request).unwrap();
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value["type"], "basic_combat_result");
        assert_eq!(value["compatibilityLevel"], BASIC_COMBAT_COMPATIBILITY_LEVEL);
    }
}
