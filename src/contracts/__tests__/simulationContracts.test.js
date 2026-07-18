import { describe, expect, it } from "vitest";
import {
    SIMULATION_CONTRACT_VERSION,
    SIMULATION_ENGINE_REFERENCE_JS,
    SIMULATION_STATISTICS_FAST,
    createEngineCapabilitiesV1,
    createGameDataManifestV1,
    createSimulationErrorV1,
    createSimulationProgressV1,
    createSimulationResultV1,
    legacyWorkerMessageToSimulationRequestV1,
    normalizeSimulationRequestV1,
    simulationRequestV1ToLegacyWorkerMessage,
} from "../simulationContracts.js";

function playerFixture() {
    return {
        hrid: "player1",
        staminaLevel: 1,
        intelligenceLevel: 1,
        attackLevel: 1,
        meleeLevel: 1,
        defenseLevel: 1,
        rangedLevel: 1,
        magicLevel: 1,
        equipment: {},
        food: [],
        drinks: [],
        abilities: [],
        houseRooms: {},
        achievements: {},
    };
}

describe("simulationContracts", () => {
    it("normalizes a versioned zone request", () => {
        const request = normalizeSimulationRequestV1({
            requestId: "zone-fixture",
            dataVersion: "data-2026-07-18",
            players: [playerFixture()],
            target: {
                kind: "zone",
                zoneHrid: "/actions/combat/fly",
                difficultyTier: 3.9,
            },
            simulationTimeLimit: 1e12,
            random: { type: "seeded", seed: "fixture" },
            options: {
                statisticsMode: SIMULATION_STATISTICS_FAST,
                enableHpMpVisualization: true,
                trace: { enabled: true, maxEntries: 500 },
                extra: { mooPass: true },
            },
        });

        expect(request).toMatchObject({
            contractVersion: SIMULATION_CONTRACT_VERSION,
            requestId: "zone-fixture",
            dataVersion: "data-2026-07-18",
            target: {
                kind: "zone",
                zoneHrid: "/actions/combat/fly",
                difficultyTier: 3,
            },
            random: { type: "seeded", seed: "fixture" },
            options: {
                statisticsMode: SIMULATION_STATISTICS_FAST,
                enableHpMpVisualization: true,
                trace: { enabled: true, maxEntries: 500 },
                extra: { mooPass: true },
            },
        });
    });

    it("round-trips the legacy labyrinth worker message", () => {
        const legacy = {
            type: "start_simulation",
            requestId: "labyrinth-fixture",
            dataVersion: "data-a",
            players: [playerFixture()],
            labyrinth: {
                labyrinthHrid: "/actions/combat/labyrinth",
                roomLevel: 77,
                crates: 4,
            },
            simulationTimeLimit: 2e12,
            random: { type: "sequence", values: [0.1, 0.2], loop: true },
            trace: { enabled: true, maxEntries: 1000 },
            extra: { enableHpMpVisualization: false, comExp: 2 },
        };

        const request = legacyWorkerMessageToSimulationRequestV1(legacy);
        const roundTripped = simulationRequestV1ToLegacyWorkerMessage(request);

        expect(roundTripped).toMatchObject(legacy);
        expect(roundTripped.zone).toBeNull();
    });

    it("rejects invalid requests before they reach an engine", () => {
        expect(() => normalizeSimulationRequestV1({})).toThrow(/player/i);
        expect(() => normalizeSimulationRequestV1({
            requestId: "bad-duration",
            players: [playerFixture()],
            target: { kind: "zone", zoneHrid: "/zone", difficultyTier: 0 },
            simulationTimeLimit: 0,
        })).toThrow(/simulationTimeLimit/i);
        expect(() => normalizeSimulationRequestV1({
            requestId: "bad-random",
            players: [playerFixture()],
            target: { kind: "zone", zoneHrid: "/zone", difficultyTier: 0 },
            simulationTimeLimit: 1,
            random: { type: "sequence", values: [1] },
        })).toThrow(/sequence random/i);
    });

    it("creates JSON-safe progress, result, and error envelopes", () => {
        const progress = createSimulationProgressV1({
            requestId: "request-a",
            progress: 2,
            target: { kind: "zone", zoneHrid: "/zone", difficultyTier: 1 },
        });
        const result = createSimulationResultV1({
            requestId: "request-a",
            engine: SIMULATION_ENGINE_REFERENCE_JS,
            engineVersion: "1.0.28",
            dataVersion: "data-a",
            result: { encounters: 10 },
        });
        const error = createSimulationErrorV1({
            requestId: "request-a",
            code: "FIXTURE_FAILURE",
            message: "Fixture failed.",
        });

        expect(progress.progress).toBe(1);
        expect(result.result).toEqual({ encounters: 10 });
        expect(error.error.code).toBe("FIXTURE_FAILURE");
        expect(() => JSON.stringify({ progress, result, error })).not.toThrow();
    });

    it("creates game-data manifests and engine capability documents", () => {
        const manifest = createGameDataManifestV1({
            version: "data-a",
            generatedAt: "2026-07-18T00:00:00.000Z",
            sources: [{ name: "init_client_data", sha256: "abc" }],
            files: [{ path: "abilityDetailMap.json", sha256: "def" }],
        });
        const capabilities = createEngineCapabilitiesV1({
            engine: SIMULATION_ENGINE_REFERENCE_JS,
            engineVersion: "1.0.28",
            targets: ["zone", "labyrinth", "zone"],
            statisticsModes: ["full", "fast", "fast"],
            eventTrace: true,
        });

        expect(manifest.type).toBe("game_data_manifest");
        expect(capabilities.targets).toEqual(["zone", "labyrinth"]);
        expect(capabilities.statisticsModes).toEqual(["full", "fast"]);
        expect(capabilities.eventTrace).toBe(true);
    });
});
