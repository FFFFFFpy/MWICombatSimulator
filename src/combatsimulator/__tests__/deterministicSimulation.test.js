import { describe, expect, it } from "vitest";
import requestFixture from "../../../fixtures/parity/zone-solo-basic/request.json";
import { normalizeSimulationRequestV1 } from "../../contracts/simulationContracts.js";
import { runReferenceSimulation } from "../../services/referenceSimulationRunner.js";

function firstDifference(first, second, path = "result") {
    if (Object.is(first, second)) return null;
    if (typeof first !== typeof second || first == null || second == null) {
        return { path, first, second };
    }
    if (typeof first !== "object") {
        return { path, first, second };
    }
    if (Array.isArray(first) !== Array.isArray(second)) {
        return { path, firstType: Array.isArray(first) ? "array" : "object", secondType: Array.isArray(second) ? "array" : "object" };
    }
    if (Array.isArray(first)) {
        if (first.length !== second.length) {
            return { path: `${path}.length`, first: first.length, second: second.length };
        }
        for (let index = 0; index < first.length; index++) {
            const difference = firstDifference(first[index], second[index], `${path}[${index}]`);
            if (difference) return difference;
        }
        return null;
    }

    const firstKeys = Object.keys(first).sort();
    const secondKeys = Object.keys(second).sort();
    if (JSON.stringify(firstKeys) !== JSON.stringify(secondKeys)) {
        return { path: `${path}.__keys`, first: firstKeys, second: secondKeys };
    }
    for (const key of firstKeys) {
        const difference = firstDifference(first[key], second[key], `${path}.${key}`);
        if (difference) return difference;
    }
    return null;
}

function assertSameRun(first, second, label) {
    const difference = firstDifference(first.result, second.result);
    if (difference || first.randomDraws !== second.randomDraws) {
        throw new Error(JSON.stringify({
            label,
            difference,
            firstRandomDraws: first.randomDraws,
            secondRandomDraws: second.randomDraws,
            firstSimulatedTime: first.result.simulatedTime,
            secondSimulatedTime: second.result.simulatedTime,
            firstEncounters: first.result.encounters,
            secondEncounters: second.result.encounters,
        }, null, 2));
    }
}

async function runFixture({ seed = requestFixture.random.seed, trace = false } = {}) {
    const request = normalizeSimulationRequestV1({
        ...requestFixture,
        random: { ...requestFixture.random, seed },
        options: {
            ...requestFixture.options,
            trace: { ...requestFixture.options.trace, enabled: trace },
        },
    });
    const execution = await runReferenceSimulation(request);
    return {
        request,
        result: JSON.parse(JSON.stringify(execution.result)),
        trace: execution.trace ? JSON.parse(JSON.stringify(execution.trace)) : null,
        randomDraws: execution.randomDraws,
    };
}

describe("deterministic reference simulation", () => {
    it("replays the same result with the same versioned request and seed", async () => {
        const first = await runFixture();
        const second = await runFixture();

        assertSameRun(first, second, "same request and seed");
        expect(first.randomDraws).toBeGreaterThan(0);
        expect(first.result.simulatedTime).toBeGreaterThanOrEqual(first.request.simulationTimeLimit);
    });

    it("does not change the final result when event trace is enabled", async () => {
        const withoutTrace = await runFixture({ seed: "trace-parity", trace: false });
        const withTrace = await runFixture({ seed: "trace-parity", trace: true });

        assertSameRun(withTrace, withoutTrace, "trace enabled versus disabled");
        expect(withTrace.trace.events.length).toBeGreaterThan(0);
        expect(withTrace.trace.events.some((entry) => entry.randomDraws.length > 0)).toBe(true);
        expect(withTrace.trace.truncatedEntries).toBe(0);
    });
});
