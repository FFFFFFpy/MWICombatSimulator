import { describe, expect, it } from "vitest";
import CombatSimulator from "../combatSimulator.js";
import Player from "../player.js";
import Zone from "../zone.js";
import { attachCombatTrace } from "../../services/combatTrace.js";
import {
    createSeededRandomSource,
    createTracingRandomSource,
    withPatchedMathRandom,
} from "../../shared/randomSource.js";

const ONE_SECOND = 1e9;

function createPlayer() {
    const player = new Player();
    player.hrid = "player1";
    player.staminaLevel = 20;
    player.intelligenceLevel = 20;
    player.attackLevel = 20;
    player.meleeLevel = 20;
    player.defenseLevel = 20;
    player.rangedLevel = 20;
    player.magicLevel = 20;
    return player;
}

async function runFixture({ seed = "zone-solo-basic", trace = false } = {}) {
    const simulator = new CombatSimulator(
        [createPlayer()],
        new Zone("/actions/combat/fly", 0),
        null,
        { enableHpMpVisualization: false },
    );
    const traceController = trace ? attachCombatTrace(simulator, { maxEntries: 10_000 }) : null;
    let randomSource = createSeededRandomSource(seed);
    if (traceController) {
        randomSource = createTracingRandomSource(
            randomSource,
            (draw) => traceController.recordRandomDraw(draw),
        );
    }
    const result = await withPatchedMathRandom(
        randomSource,
        () => simulator.simulate(60 * ONE_SECOND),
    );
    const serializedResult = JSON.parse(JSON.stringify(result));
    const serializedTrace = traceController ? JSON.parse(JSON.stringify(traceController.getTrace())) : null;
    traceController?.detach();
    return { result: serializedResult, trace: serializedTrace };
}

describe("deterministic reference simulation", () => {
    it("replays the same result with the same seed", async () => {
        const first = await runFixture();
        const second = await runFixture();

        expect(first.result).toEqual(second.result);
        expect(first.result.simulatedTime).toBeGreaterThanOrEqual(60 * ONE_SECOND);
    });

    it("does not change the final result when event trace is enabled", async () => {
        const withoutTrace = await runFixture({ seed: "trace-parity", trace: false });
        const withTrace = await runFixture({ seed: "trace-parity", trace: true });

        expect(withTrace.result).toEqual(withoutTrace.result);
        expect(withTrace.trace.events.length).toBeGreaterThan(0);
        expect(withTrace.trace.events.some((entry) => entry.randomDraws.length > 0)).toBe(true);
        expect(withTrace.trace.truncatedEntries).toBe(0);
    });
});
