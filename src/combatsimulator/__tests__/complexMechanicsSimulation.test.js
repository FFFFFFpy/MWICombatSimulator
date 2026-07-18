import { describe, expect, it } from "vitest";
import dotControlRequest from "../../../fixtures/parity/ability-dot-control-sequence/request.json";
import equipmentPassivesRequest from "../../../fixtures/parity/equipment-passives-reflect/request.json";
import oomRecoveryRequest from "../../../fixtures/parity/oom-yogurt-recovery/request.json";
import regalParryRequest from "../../../fixtures/parity/regal-sword-parry/request.json";
import { runReferenceSimulation } from "../../services/referenceSimulationRunner.js";

function allTraceUnits(trace) {
    return trace.events.flatMap((entry) => [
        ...(entry.before?.players || []),
        ...(entry.before?.enemies || []),
        ...(entry.after?.players || []),
        ...(entry.after?.enemies || []),
    ]);
}

function eventTypeCount(trace, type) {
    return trace.events.filter((entry) => entry.event?.type === type).length;
}

function abilityEventCount(trace, hrid) {
    return trace.events.filter((entry) => entry.event?.ability === hrid).length;
}

function consumableEventCount(trace, hrid) {
    return trace.events.filter((entry) => entry.event?.consumable === hrid).length;
}

function observedBuffHrids(trace) {
    return new Set(
        allTraceUnits(trace)
            .flatMap((unit) => unit?.combatState?.buffs || [])
            .map((buff) => buff.uniqueHrid),
    );
}

function recursiveHasKey(value, expectedKey) {
    if (!value || typeof value !== "object") return false;
    if (Object.prototype.hasOwnProperty.call(value, expectedKey)) return true;
    return Object.values(value).some((entry) => recursiveHasKey(entry, expectedKey));
}

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

describe("complex deterministic combat mechanics", () => {
    it("covers DOT plus blind, silence, and stun with a fixed random sequence", async () => {
        const execution = await runFixture(dotControlRequest);

        expect(execution.trace.detailLevel).toBe("combat");
        expect(execution.trace.truncatedEntries).toBe(0);
        expect(abilityEventCount(execution.trace, "/abilities/firestorm")).toBeGreaterThan(0);
        expect(abilityEventCount(execution.trace, "/abilities/natures_veil")).toBeGreaterThan(0);
        expect(abilityEventCount(execution.trace, "/abilities/silencing_shot")).toBeGreaterThan(0);
        expect(abilityEventCount(execution.trace, "/abilities/stunning_blow")).toBeGreaterThan(0);
        expect(eventTypeCount(execution.trace, "damageOverTime")).toBeGreaterThan(0);
        expect(eventTypeCount(execution.trace, "blindExpiration")).toBeGreaterThan(0);
        expect(eventTypeCount(execution.trace, "silenceExpiration")).toBeGreaterThan(0);
        expect(eventTypeCount(execution.trace, "stunExpiration")).toBeGreaterThan(0);

        const units = allTraceUnits(execution.trace);
        expect(units.some((unit) => unit?.blinded === true)).toBe(true);
        expect(units.some((unit) => unit?.silenced === true)).toBe(true);
        expect(units.some((unit) => unit?.stunned === true)).toBe(true);
        expect(recursiveHasKey(execution.result.attacks, "damageOverTime")).toBe(true);
    });

    it("records OOM, recovery-over-time food, and later ability reuse", async () => {
        const execution = await runFixture(oomRecoveryRequest);

        expect(execution.result.playerRanOutOfMana.player1).toBe(true);
        expect(execution.result.consumablesUsed.player1?.["/items/yogurt"]).toBeGreaterThan(0);
        expect(execution.result.manapointsGained.player1?.["/items/yogurt"] ?? 0).toBeGreaterThan(0);
        expect(eventTypeCount(execution.trace, "consumableTick")).toBeGreaterThan(0);
        expect(consumableEventCount(execution.trace, "/items/yogurt")).toBeGreaterThan(0);
        expect(abilityEventCount(execution.trace, "/abilities/firestorm")).toBeGreaterThan(1);

        const manaValues = allTraceUnits(execution.trace)
            .filter((unit) => unit?.hrid === "player1")
            .map((unit) => unit.manapoints);
        expect(Math.min(...manaValues)).toBeLessThan(75);
        expect(Math.max(...manaValues)).toBeGreaterThan(Math.min(...manaValues));
    });

    it("covers fury, weaken, thorns, and retaliation without parry stealing the hit", async () => {
        const execution = await runFixture(equipmentPassivesRequest);

        const buffs = observedBuffHrids(execution.trace);
        expect(buffs.has("/buff_uniques/spike_shell_physical_thorns")).toBe(true);
        expect(buffs.has("/buff_uniques/spike_shell_elemental_thorns")).toBe(true);
        expect(buffs.has("/buff_uniques/retribution")).toBe(true);
        expect(buffs.has("/buff_uniques/fury_accuracy")).toBe(true);
        expect(buffs.has("/buff_uniques/fury_damage")).toBe(true);
        expect(buffs.has("/buff_uniques/weaken")).toBe(true);
        const dealtThorns = recursiveHasKey(execution.result.attacks, "physicalThorns")
            || recursiveHasKey(execution.result.attacks, "elementalThorns");
        expect(dealtThorns).toBe(true);
        expect(recursiveHasKey(execution.result.attacks, "retaliation")).toBe(true);
    });

    it("records a successful parry as enemy damage during an enemy auto-attack event", async () => {
        const execution = await runFixture(regalParryRequest);
        const parriedEnemyAttack = execution.trace.events.find((entry) => {
            if (entry.event?.type !== "autoAttack" || !entry.event?.source?.startsWith("/monsters/")) {
                return false;
            }
            const enemyLostHp = entry.changes?.enemies?.some((change) => change.changes?.hitpoints);
            const playerLostHp = entry.changes?.players?.some((change) => change.changes?.hitpoints);
            return enemyLostHp && !playerLostHp;
        });

        expect(parriedEnemyAttack).toBeTruthy();
    });
});
