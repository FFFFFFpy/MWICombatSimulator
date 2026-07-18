export const SIMULATION_CONTRACT_VERSION = 1;
export const SIMULATION_ENGINE_REFERENCE_JS = "reference-js";
export const SIMULATION_ENGINE_RUST_WASM = "rust-wasm";
export const SIMULATION_ENGINE_RUST_NATIVE = "rust-native";
export const SIMULATION_STATISTICS_FULL = "full";
export const SIMULATION_STATISTICS_FAST = "fast";

function isPlainObject(value) {
    return value != null && typeof value === "object" && !Array.isArray(value);
}

function finiteNumber(value, fallback = 0) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : fallback;
}

function nonEmptyString(value, field) {
    const normalized = String(value || "").trim();
    if (!normalized) {
        throw new Error(`${field} is required.`);
    }
    return normalized;
}

function cloneJsonValue(value, fallback = null) {
    try {
        return value == null ? value : JSON.parse(JSON.stringify(value));
    } catch (error) {
        return fallback;
    }
}

function normalizeStatisticsMode(value) {
    return value === SIMULATION_STATISTICS_FAST
        ? SIMULATION_STATISTICS_FAST
        : SIMULATION_STATISTICS_FULL;
}

export function normalizeSimulationTargetV1(target) {
    if (!isPlainObject(target)) {
        throw new Error("Simulation target is required.");
    }

    if (target.kind === "zone") {
        return {
            kind: "zone",
            zoneHrid: nonEmptyString(target.zoneHrid, "target.zoneHrid"),
            difficultyTier: Math.max(0, Math.trunc(finiteNumber(target.difficultyTier, 0))),
        };
    }

    if (target.kind === "labyrinth") {
        return {
            kind: "labyrinth",
            labyrinthHrid: nonEmptyString(target.labyrinthHrid, "target.labyrinthHrid"),
            roomLevel: Math.max(1, Math.trunc(finiteNumber(target.roomLevel, 1))),
            crates: Math.max(0, Math.trunc(finiteNumber(target.crates, 0))),
        };
    }

    throw new Error(`Unsupported simulation target kind: ${String(target.kind)}`);
}

export function normalizeSimulationRandomV1(random) {
    if (random == null || random.type === "native") {
        return null;
    }
    if (random.type === "seeded") {
        return {
            type: "seeded",
            seed: cloneJsonValue(random.seed, 0),
        };
    }
    if (random.type === "sequence") {
        const values = Array.isArray(random.values) ? random.values.map(Number) : [];
        if (values.length === 0 || values.some((value) => !Number.isFinite(value) || value < 0 || value >= 1)) {
            throw new Error("Sequence random configuration requires values in [0, 1).");
        }
        return {
            type: "sequence",
            values,
            loop: random.loop === true,
        };
    }
    throw new Error(`Unsupported random source type: ${String(random.type)}`);
}

export function normalizeSimulationRequestV1(input) {
    if (!isPlainObject(input)) {
        throw new TypeError("Simulation request must be an object.");
    }

    const players = Array.isArray(input.players) ? cloneJsonValue(input.players, []) : [];
    if (players.length === 0) {
        throw new Error("Simulation request requires at least one player.");
    }

    const simulationTimeLimit = finiteNumber(input.simulationTimeLimit, 0);
    if (simulationTimeLimit <= 0) {
        throw new Error("simulationTimeLimit must be greater than zero.");
    }

    const traceInput = isPlainObject(input.options?.trace) ? input.options.trace : {};
    return {
        contractVersion: SIMULATION_CONTRACT_VERSION,
        requestId: nonEmptyString(input.requestId || `simulation-${Date.now()}`, "requestId"),
        dataVersion: String(input.dataVersion || "unversioned"),
        players,
        target: normalizeSimulationTargetV1(input.target),
        simulationTimeLimit,
        random: normalizeSimulationRandomV1(input.random),
        options: {
            statisticsMode: normalizeStatisticsMode(input.options?.statisticsMode),
            enableHpMpVisualization: input.options?.enableHpMpVisualization === true,
            trace: {
                enabled: traceInput.enabled === true,
                maxEntries: Math.max(1, Math.trunc(finiteNumber(traceInput.maxEntries, 100_000))),
            },
            extra: cloneJsonValue(input.options?.extra, {}),
        },
    };
}

export function legacyWorkerMessageToSimulationRequestV1(message, metadata = {}) {
    if (!isPlainObject(message) || message.type !== "start_simulation") {
        throw new Error("Expected a start_simulation worker message.");
    }

    const target = message.zone
        ? {
            kind: "zone",
            zoneHrid: message.zone.zoneHrid,
            difficultyTier: message.zone.difficultyTier,
        }
        : message.labyrinth
            ? {
                kind: "labyrinth",
                labyrinthHrid: message.labyrinth.labyrinthHrid,
                roomLevel: message.labyrinth.roomLevel,
                crates: message.labyrinth.crates,
            }
            : null;

    return normalizeSimulationRequestV1({
        requestId: metadata.requestId || message.requestId,
        dataVersion: metadata.dataVersion || message.dataVersion,
        players: message.players,
        target,
        simulationTimeLimit: message.simulationTimeLimit,
        random: message.random,
        options: {
            statisticsMode: message.statisticsMode,
            enableHpMpVisualization: message.extra?.enableHpMpVisualization,
            trace: message.trace,
            extra: message.extra,
        },
    });
}

export function simulationRequestV1ToLegacyWorkerMessage(request) {
    const normalized = normalizeSimulationRequestV1(request);
    const message = {
        type: "start_simulation",
        requestId: normalized.requestId,
        dataVersion: normalized.dataVersion,
        players: normalized.players,
        simulationTimeLimit: normalized.simulationTimeLimit,
        statisticsMode: normalized.options.statisticsMode,
        random: normalized.random,
        trace: normalized.options.trace,
        extra: {
            ...normalized.options.extra,
            enableHpMpVisualization: normalized.options.enableHpMpVisualization,
        },
        zone: null,
        labyrinth: null,
    };

    if (normalized.target.kind === "zone") {
        message.zone = {
            zoneHrid: normalized.target.zoneHrid,
            difficultyTier: normalized.target.difficultyTier,
        };
    } else {
        message.labyrinth = {
            labyrinthHrid: normalized.target.labyrinthHrid,
            roomLevel: normalized.target.roomLevel,
            crates: normalized.target.crates,
        };
    }

    return message;
}

export function createSimulationProgressV1({ requestId, progress, target = null, detail = null }) {
    return {
        contractVersion: SIMULATION_CONTRACT_VERSION,
        type: "simulation_progress",
        requestId: nonEmptyString(requestId, "requestId"),
        progress: Math.min(1, Math.max(0, finiteNumber(progress, 0))),
        target: target ? normalizeSimulationTargetV1(target) : null,
        detail: cloneJsonValue(detail, null),
    };
}

export function createSimulationResultV1({
    requestId,
    engine = SIMULATION_ENGINE_REFERENCE_JS,
    engineVersion = "unversioned",
    dataVersion = "unversioned",
    result,
    trace = null,
}) {
    return {
        contractVersion: SIMULATION_CONTRACT_VERSION,
        type: "simulation_result",
        requestId: nonEmptyString(requestId, "requestId"),
        engine: nonEmptyString(engine, "engine"),
        engineVersion: String(engineVersion || "unversioned"),
        dataVersion: String(dataVersion || "unversioned"),
        result: cloneJsonValue(result, {}),
        trace: cloneJsonValue(trace, null),
    };
}

export function createSimulationErrorV1({ requestId, code = "SIMULATION_FAILED", message, detail = null }) {
    return {
        contractVersion: SIMULATION_CONTRACT_VERSION,
        type: "simulation_error",
        requestId: nonEmptyString(requestId, "requestId"),
        error: {
            code: nonEmptyString(code, "error.code"),
            message: nonEmptyString(message, "error.message"),
            detail: cloneJsonValue(detail, null),
        },
    };
}

export function createGameDataManifestV1({ version, generatedAt, sources = [], files = [] }) {
    return {
        contractVersion: SIMULATION_CONTRACT_VERSION,
        type: "game_data_manifest",
        version: nonEmptyString(version, "version"),
        generatedAt: nonEmptyString(generatedAt, "generatedAt"),
        sources: cloneJsonValue(sources, []),
        files: cloneJsonValue(files, []),
    };
}

export function createEngineCapabilitiesV1({
    engine,
    engineVersion,
    targets = ["zone", "labyrinth"],
    statisticsModes = [SIMULATION_STATISTICS_FULL],
    deterministicRandom = true,
    eventTrace = false,
    nativeBatch = false,
}) {
    return {
        contractVersion: SIMULATION_CONTRACT_VERSION,
        type: "engine_capabilities",
        engine: nonEmptyString(engine, "engine"),
        engineVersion: String(engineVersion || "unversioned"),
        targets: [...new Set(targets.map(String))],
        statisticsModes: [...new Set(statisticsModes.map(normalizeStatisticsMode))],
        deterministicRandom: deterministicRandom === true,
        eventTrace: eventTrace === true,
        nativeBatch: nativeBatch === true,
    };
}
