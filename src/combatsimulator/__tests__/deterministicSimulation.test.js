import { describe, expect, it } from "vitest";
import requestFixture from "../../../fixtures/parity/zone-solo-basic/request.json";
import CombatSimulator from "../combatSimulator.js";
import Player from "../player.js";
import Zone from "../zone.js";
import { normalizeSimulationRequestV1 } from "../../contracts/simulationContracts.js";
import { attachCombatTrace } from "../../services/combatTrace.js";
import {
    createRandomSourceFromConfig,
    createTracingRandomSource,
    withPatchedMathRandom,
} from "../../shared/randomSource.js";

async function runFixture({ seed = requestFixture.random.seed, trace = false } = {}) {
    const request = normalizeSimulationRequestV1({
        ...requestFixture,
        random: { ...requestFixture.random, seed },
        options: {
            ...requestFixture.options,
            trace: { ...requestFixture.options.trace, enabled: trace },
        },
    });
    const players = request.players.map((player) => Player.createFromDTO(structuredClone(player)));
    const simulator = new CombatSimulator(
        players,
        new Zone(request.target.zoneHrid, request.target.difficultyTier),
        null,
        { enableHpMpVisualization: request.options.enableHpMpVisualization },
    );
    const traceController = trace
        ? attachCombatTrace(simulator, { maxEntries: request.options.trace.maxEntries })
        : null;
    let randomSource = createRandomSourceFromConfig(request.random);
    if (traceController) {
        randomSource = createTracingRandomSource(
            randomSource,
            (draw) => traceController.recordRandomDraw(draw),
        );
    }
    const result = await withPatchedMathRandom(
        randomSource,
        () => simulator.simulate(request.simulationTimeLimit),
    );
    const serializedResult = JSON.parse(JSON.stringify(result));
    const serializedTrace = traceController ? JSON.parse(JSON.stringify(traceController.getTrace())) : null;
    const randomDraws = randomSource?.drawCount || 0;
    traceController?.detach();
    return { request, result: serializedResult, trace: serializedTrace, randomDraws };
}

describe("deterministic reference simulation", () => {
    it("replays the same result with the same versioned request and seed", async () => {
        const first = await runFixture();
        const second = await runFixture();

        expect(first.result).toEqual(second.result);
        expect(first.randomDraws).toBe(second.randomDraws);
        expect(first.randomDraws).toBeGreaterThan(0);
        expect(first.result.simulatedTime).toBeGreaterThanOrEqual(first.request.simulationTimeLimit);
    });

    it("does not change the final result when event trace is enabled", async () => {
        const withoutTrace = await runFixture({ seed: "trace-parity", trace: false });
        const withTrace = await runFixture({ seed: "trace-parity", trace: true });

        expect(withTrace.result).toEqual(withoutTrace.result);
        expect(withTrace.randomDraws).toBe(withoutTrace.randomDraws);
        expect(withTrace.trace.events.length).toBeGreaterThan(0);
        expect(withTrace.trace.events.some((entry) => entry.randomDraws.length > 0)).toBe(true);
        expect(withTrace.trace.truncatedEntries).toBe(0);
    });
});
