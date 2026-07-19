#![forbid(unsafe_code)]

use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use mwi_sim_core::{
    AQUA_ARROW_HRID, AbilityDataSnapshotV1, BASIC_COMBAT_COMPATIBILITY_LEVEL,
    BASIC_FLY_ZONE_HRID, CONTRACT_VERSION, CombatZoneDataSnapshotV1,
    DIRECT_DAMAGE_COMPATIBILITY_LEVEL, ENGINE_ID, RandomConfigV1, RandomSource, SeededRandom,
    SimulationRequestV1, SimulationTargetV1, classify_auto_attack_zones,
    classify_direct_damage_abilities, simulate_basic, simulate_direct_damage,
};
use serde_json::{Value, json};

const RESTRICTED_COMBAT_ENGINE_ID: &str = "rust-restricted-combat";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("capabilities") => print_json(&capabilities()),
        Some("inspect-auto-attack-zones") => {
            let snapshot = CombatZoneDataSnapshotV1::embedded()?;
            print_json(&serde_json::to_value(classify_auto_attack_zones(
                &snapshot,
            )?)?)
        }
        Some("inspect-direct-damage-abilities") => {
            let snapshot = AbilityDataSnapshotV1::embedded()?;
            print_json(&serde_json::to_value(classify_direct_damage_abilities(
                &snapshot,
            )?)?)
        }
        Some("validate-request") => {
            let path = required_path(args.next(), "validate-request <request.json>")?;
            let request = load_request(&path)?;
            print_json(&request_summary(&path, &request))
        }
        Some("validate-fixtures") => {
            let root = required_path(args.next(), "validate-fixtures <fixtures/parity>")?;
            let paths = discover_requests(&root)?;
            if paths.is_empty() {
                return Err(format!("no request.json files found under {}", root.display()).into());
            }
            let summaries = paths
                .iter()
                .map(|path| load_request(path).map(|request| request_summary(path, &request)))
                .collect::<Result<Vec<_>, _>>()?;
            print_json(&json!({
                "engine": ENGINE_ID,
                "validated": summaries.len(),
                "requests": summaries,
            }))
        }
        Some("simulate-basic") => {
            let path = required_path(args.next(), "simulate-basic <request.json>")?;
            let request = load_request(&path)?;
            print_json(&serde_json::to_value(simulate_basic(&request)?)?)
        }
        Some("simulate-direct-damage") => {
            let path = required_path(args.next(), "simulate-direct-damage <request.json>")?;
            let request = load_request(&path)?;
            print_json(&serde_json::to_value(simulate_direct_damage(&request)?)?)
        }
        Some("rng-seeded") => {
            let seed = args.next().ok_or("usage: rng-seeded <seed> <count>")?;
            let count = args
                .next()
                .ok_or("usage: rng-seeded <seed> <count>")?
                .parse::<usize>()?;
            let mut rng = SeededRandom::from_seed(&Value::String(seed.clone()))?;
            let values = (0..count)
                .map(|_| rng.next_unit_f64())
                .collect::<Result<Vec<_>, _>>()?;
            print_json(&json!({
                "seed": seed,
                "drawCount": rng.draw_count(),
                "values": values,
            }))
        }
        Some("help") | Some("--help") | Some("-h") | None => {
            print_help();
            Ok(())
        }
        Some(command) => Err(format!("unknown command: {command}").into()),
    }
}

fn capabilities() -> Value {
    json!({
        "engine": RESTRICTED_COMBAT_ENGINE_ID,
        "foundationEngine": ENGINE_ID,
        "engineVersion": env!("CARGO_PKG_VERSION"),
        "contractVersion": CONTRACT_VERSION,
        "implemented": [
            "contracts",
            "sequence-rng",
            "seeded-rng-string",
            "stable-event-queue",
            "basic-auto-attack",
            "single-target-direct-damage-cast"
        ],
        "combatSimulation": true,
        "fullSimulationResult": false,
        "resultTypes": [
            "basic_combat_result",
            "direct_damage_combat_result"
        ],
        "compatibilityLevels": [
            BASIC_COMBAT_COMPATIBILITY_LEVEL,
            DIRECT_DAMAGE_COMPATIBILITY_LEVEL
        ],
        "targets": [{
            "kind": "zone",
            "zoneHrids": [BASIC_FLY_ZONE_HRID],
            "difficultyTiers": [0],
            "players": { "minimum": 1, "maximum": 1 }
        }],
        "abilityCapabilities": [{
            "hrid": AQUA_ARROW_HRID,
            "levels": [1],
            "customTriggers": false,
            "resultType": "direct_damage_combat_result",
            "compatibilityLevel": DIRECT_DAMAGE_COMPATIBILITY_LEVEL
        }],
        "statisticsModes": [],
        "eventTrace": false,
        "nativeBatch": false,
    })
}

fn required_path(value: Option<String>, usage: &str) -> Result<PathBuf, Box<dyn Error>> {
    value
        .map(PathBuf::from)
        .ok_or_else(|| usage.to_owned().into())
}

fn load_request(path: &Path) -> Result<SimulationRequestV1, Box<dyn Error>> {
    let input = fs::read_to_string(path)?;
    Ok(SimulationRequestV1::from_json(&input)?)
}

fn discover_requests(root: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut requests = Vec::new();
    discover_requests_recursive(root, &mut requests)?;
    requests.sort();
    Ok(requests)
}

fn discover_requests_recursive(
    directory: &Path,
    requests: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            discover_requests_recursive(&path, requests)?;
        } else if path.file_name().is_some_and(|name| name == "request.json") {
            requests.push(path);
        }
    }
    Ok(())
}

fn request_summary(path: &Path, request: &SimulationRequestV1) -> Value {
    let target = match &request.target {
        SimulationTargetV1::Zone {
            zone_hrid,
            difficulty_tier,
            ..
        } => json!({
            "kind": "zone",
            "hrid": zone_hrid,
            "difficultyTier": difficulty_tier,
        }),
        SimulationTargetV1::Labyrinth {
            labyrinth_hrid,
            room_level,
            ..
        } => json!({
            "kind": "labyrinth",
            "hrid": labyrinth_hrid,
            "roomLevel": room_level,
        }),
    };
    let random = match &request.random {
        None | Some(RandomConfigV1::Native) => "native",
        Some(RandomConfigV1::Seeded { .. }) => "seeded",
        Some(RandomConfigV1::Sequence { .. }) => "sequence",
    };

    json!({
        "path": path.to_string_lossy(),
        "requestId": request.request_id,
        "dataVersion": request.data_version,
        "players": request.players.len(),
        "simulationTimeLimit": request.simulation_time_limit.get(),
        "target": target,
        "random": random,
        "traceDetailLevel": format!("{:?}", request.options.trace.detail_level).to_lowercase(),
    })
}

fn print_json(value: &Value) -> Result<(), Box<dyn Error>> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn print_help() {
    println!(
        "mwi-sim-cli\n\n\
         Commands:\n\
           capabilities\n\
           inspect-auto-attack-zones\n\
           inspect-direct-damage-abilities\n\
           validate-request <request.json>\n\
           validate-fixtures <fixtures/parity>\n\
           simulate-basic <request.json>\n\
           simulate-direct-damage <request.json>\n\
           rng-seeded <seed> <count>\n\n\
         inspect commands report data-derived candidates only.\n\
         They do not expand the formal capabilities target list.\n\n\
         simulate-basic is restricted to the M2 single-player tier-0 Fly\n\
         auto-attack capability. simulate-direct-damage is restricted to\n\
         level-1 Aqua Arrow with empty custom Triggers. Neither returns\n\
         SimulationResultV1."
    );
}
