import { describe, expect, it } from "vitest";
import pierceRequest from "../../../fixtures/parity/ability-pierce-multi-target/request.json";
import curseRequest from "../../../fixtures/parity/cursed-bow-stacking/request.json";
import healingRequest from "../../../fixtures/parity/healing-hot-party/request.json";
import reviveRequest from "../../../fixtures/parity/revive-dead-ally/request.json";
import speedAuraRequest from "../../../fixtures/parity/speed-aura-simultaneous-expiration/request.json";
import { runReferenceSimulation } from "../../services/referenceSimulationRunner.js";

function runFixture(request) {
    return runReferenceSimulation({
        ...request,
        options: {
            ...request.options,
            trace: {
                ...request.options.trace,
                enabled: true,
            },
        },
    });
}

function abilityEvents(trace, hrid) {
    return trace.events.filter((entry) => entry.event?.ability === hrid);
}

function traceUnits(trace) {
    return trace.events.flatMap((entry) => [
        ...(entry.before?.players || []),
        ...(entry.before?.enemies || []),
        ...(entry.after?.players || []),
        ...(entry.after?.enemies || []),
    ]);
}

function observedBuffs(trace, uniqueHrid) {
    return traceUnits(trace)
        .flatMap((unit) => unit?.combatState?.buffs || [])
        .filter((buff) => buff.uniqueHrid === uniqueHrid);
}

function recursiveHasKey(value, expectedKey) {
    if (!value || typeof value !== "object") return false;
    if (Object.prototype.hasOwnProperty.call(value, expectedKey)) return true;
    return Object.values(value).some((entry) => recursiveHasKey(entry, expectedKey));
}

function speedBuffs(snapshot) {
    return (snapshot?.players?.[0]?.combatState?.buffs || [])
        .filter((buff) => buff.uniqueHrid.startsWith("/buff_uniques/speed_aura"));
}

describe("advanced deterministic combat mechanics", () => {
    it("covers lowest-HP healing, all-party healing, and HP recovery ticks", async () => {
        const execution = await runFixture(healingRequest);

        expect(execution.trace.truncatedEntries).toBe(0);
        expect(abilityEvents(execution.trace, "/abilities/quick_aid").length).toBeGreaterThan(0);
        expect(abilityEvents(execution.trace, "/abilities/rejuvenate").length).toBeGreaterThan(0);
        expect(execution.trace.events.some((entry) => entry.event?.type === "consumableTick")).toBe(true);
        expect(execution.result.consumablesUsed.player1?.["/items/blueberry_cake"] ?? 0).toBeGreaterThan(0);
        expect(execution.result.hitpointsGained.player1?.["/items/blueberry_cake"] ?? 0).toBeGreaterThan(0);
        expect(execution.result.hitpointsGained.player1?.["/abilities/quick_aid"] ?? 0).toBeGreaterThan(0);
        expect(
            Object.values(execution.result.hitpointsGained)
                .some((sources) => Number(sources?.["/abilities/rejuvenate"] || 0) > 0),
        ).toBe(true);
    });

    it("covers guaranteed ability pierce and all-enemy damage in one multi-enemy encounter", async () => {
        const execution = await runFixture(pierceRequest);
        const penetratingEvent = abilityEvents(execution.trace, "/abilities/penetrating_shot")
            .find((entry) => (entry.changes?.enemies || []).filter((change) => change.changes?.hitpoints).length >= 2);
        const sweepEvent = abilityEvents(execution.trace, "/abilities/sweep")
            .find((entry) => (entry.changes?.enemies || []).filter((change) => change.changes?.hitpoints).length >= 2);

        expect(penetratingEvent).toBeTruthy();
        expect(sweepEvent).toBeTruthy();
        expect(recursiveHasKey(execution.result.attacks.player1, "/abilities/penetrating_shot")).toBe(true);
        expect(recursiveHasKey(execution.result.attacks.player1, "/abilities/sweep")).toBe(true);
    });

    it("covers curse stacking plus expiration-event replacement from Cursed Bow attacks", async () => {
        const execution = await runFixture(curseRequest);
        const curseBuffs = observedBuffs(execution.trace, "/buff_uniques/curse");

        expect(curseBuffs.length).toBeGreaterThan(0);
        expect(Math.max(...curseBuffs.map((buff) => Number(buff.flatBoost || 0)))).toBeGreaterThanOrEqual(0.04);
        expect(
            execution.trace.events.some((entry) =>
                (entry.queueOperations || []).some((operation) =>
                    operation.operation === "add" && operation.event?.type === "curseExpiration"),
            ),
        ).toBe(true);
        expect(
            execution.trace.events.some((entry) =>
                (entry.queueOperations || []).some((operation) =>
                    operation.operation === "cancel" && operation.event?.type === "curseExpiration"),
            ),
        ).toBe(true);
    });

    it("revives a dead ally before the normal non-dungeon respawn path exists", async () => {
        const execution = await runFixture(reviveRequest);
        const reviveEvent = abilityEvents(execution.trace, "/abilities/revive")
            .find((entry) => (entry.changes?.players || []).some((change) =>
                change.hrid === "player1"
                && Number(change.changes?.hitpoints?.before) === 0
                && Number(change.changes?.hitpoints?.after) > 0,
            ));

        expect(execution.result.deaths.player1 ?? 0).toBeGreaterThan(0);
        expect(execution.result.hitpointsGained.player1?.["/abilities/revive"] ?? 0).toBeGreaterThan(0);
        expect(reviveEvent).toBeTruthy();
    });

    it("locks stable ordering for simultaneous Speed Aura buff expirations", async () => {
        const execution = await runFixture(speedAuraRequest);
        const expirationEvents = execution.trace.events.filter((entry) =>
            entry.event?.type === "checkBuffExpiration" && entry.event?.source === "player1",
        );
        const groups = new Map();
        for (const entry of expirationEvents) {
            const group = groups.get(entry.event.time) || [];
            group.push(entry);
            groups.set(entry.event.time, group);
        }
        const simultaneous = [...groups.values()].find((entries) => entries.length >= 2);

        expect(simultaneous).toBeTruthy();
        expect(simultaneous[0].sequence + 1).toBe(simultaneous[1].sequence);
        expect(
            simultaneous.some((entry) => speedBuffs(entry.before).length === 2 && speedBuffs(entry.after).length === 0),
        ).toBe(true);
    });
});
