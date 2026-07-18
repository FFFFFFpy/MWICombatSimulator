#![forbid(unsafe_code)]

use mwi_sim_core::{CONTRACT_VERSION, ENGINE_ID, SimulationRequestV1};

pub fn capabilities_json() -> String {
    serde_json::json!({
        "engine": ENGINE_ID,
        "engineVersion": env!("CARGO_PKG_VERSION"),
        "contractVersion": CONTRACT_VERSION,
        "implemented": ["contracts", "sequence-rng", "seeded-rng", "stable-event-queue"],
        "combatSimulation": false,
        "targets": [],
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_foundation_capabilities_without_claiming_combat_support() {
        let capabilities: serde_json::Value = serde_json::from_str(&capabilities_json()).unwrap();
        assert_eq!(capabilities["combatSimulation"], false);
        assert_eq!(capabilities["contractVersion"], CONTRACT_VERSION);
    }

    #[test]
    fn validates_a_committed_fixture() {
        let request = include_str!("../../../fixtures/parity/zone-solo-basic/request.json");
        let normalized = validate_request_json(request).unwrap();
        let value: serde_json::Value = serde_json::from_str(&normalized).unwrap();
        assert_eq!(value["requestId"], "parity-zone-solo-basic-v1");
    }
}
