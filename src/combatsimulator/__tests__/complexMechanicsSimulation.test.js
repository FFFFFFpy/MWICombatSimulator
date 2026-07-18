import { describe, expect, it } from "vitest";
import { runReferenceSimulation } from "../../services/referenceSimulationRunner.js";

const ONE_SECOND = 1e9;
const DURABLE_LABYRINTH_TARGET = {
    kind: "labyrinth",
    labyrinthHrid: "/monsters/crystal_colossus",
    roomLevel: 100,
    crates: [],
};

function trigger(dependencyHrid, conditionHrid, comparatorHrid, value = 0) {
    return { dependencyHrid, conditionHrid, comparatorHrid, value };
}

function playerDto({
    hrid = "player1",
    staminaLevel = 100,
    intelligenceLevel = 100,
    attackLevel = 50,
    meleeLevel = 50,
    defenseLevel = 100,
    rangedLevel = 50,
    magicLevel = 50,
    equipment = {},
    abilities = [],
    food = [],
    drinks = [],
} = {}) {
    return {
        hrid,
        staminaLevel,
        intelligenceLevel,
        attackLevel,
        meleeLevel,
        defenseLevel,
        rangedLevel,
        magicLevel,
        equipment,
        food,
        drinks,
        abilities,
        houseRooms: {},
        guildBuffs: {},
        achievements: {},
        debuffOnLevelGap: 0,
    };
}

function ability(hrid, level = 1, triggers = []) {
    return { hrid, level, triggers };
}

function consumable(hrid, triggers) {
    return { hrid, triggers };
}

function equipment(hrid, enhancementLevel = 0) {
    return { hrid, enhancementLevel };
}

function detailedTraceOptions(maxEntries = 20_000) {
    return {
        statisticsMode: "full",
        enableHpMpVisualization: false,
        trace: {
            enabled: true,
            maxEntries,
            detailLevel: "combat",
        },
        extra: {},
    };
}

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

describe("complex deterministic combat mechanics", () => {
    it("covers DOT plus blind, silence, and stun with a fixed random sequence", async () => {
        const execution = await runReferenceSimulation({
            contractVersion: 1,
            requestId: "complex-dot-control-sequence",
            dataVersion: "repository-v1.0.28",
            players: [playerDto({
                staminaLevel: 2_000,
                intelligenceLevel: 300,
                defenseLevel: 2_000,
                attackLevel: 10,
                meleeLevel: 10,
                rangedLevel: 10,
                magicLevel: 10,
                abilities: [
                    ability("/abilities/firestorm"),
                    ability("/abilities/natures_veil"),
                    ability("/abilities/silencing_shot"),
                    ability("/abilities/stunning_blow"),
                ],
            })],
            target: DURABLE_LABYRINTH_TARGET,
            simulationTimeLimit: 30 * ONE_SECOND,
            random: { type: "sequence", values: [0], loop: true },
            options: detailedTraceOptions(),
        });

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
        const missingMpTrigger = trigger(
            "/combat_trigger_dependencies/self",
            "/combat_trigger_conditions/missing_mp",
            "/combat_trigger_comparators/greater_than_equal",
            50,
        );
        const execution = await runReferenceSimulation({
            contractVersion: 1,
            requestId: "complex-oom-yogurt-recovery",
            dataVersion: "repository-v1.0.28",
            players: [playerDto({
                staminaLevel: 2_000,
                intelligenceLevel: 2,
                defenseLevel: 2_000,
                attackLevel: 10,
                meleeLevel: 10,
                rangedLevel: 10,
                magicLevel: 10,
                abilities: [
                    ability("/abilities/firestorm"),
                    ability("/abilities/natures_veil"),
                ],
                food: [consumable("/items/yogurt", [missingMpTrigger])],
            })],
            target: DURABLE_LABYRINTH_TARGET,
            simulationTimeLimit: 50 * ONE_SECOND,
            random: { type: "sequence", values: [0], loop: true },
            options: detailedTraceOptions(),
        });

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
        const execution = await runReferenceSimulation({
            contractVersion: 1,
            requestId: "complex-equipment-passives",
            dataVersion: "repository-v1.0.28",
            players: [
                playerDto({
                    hrid: "player1",
                    staminaLevel: 500,
                    intelligenceLevel: 300,
                    defenseLevel: 500,
                    equipment: {
                        "/equipment_types/two_hand": equipment("/items/griffin_bulwark"),
                    },
                    abilities: [
                        ability("/abilities/spike_shell"),
                        ability("/abilities/retribution"),
                    ],
                }),
                playerDto({
                    hrid: "player2",
                    staminaLevel: 500,
                    intelligenceLevel: 300,
                    defenseLevel: 500,
                    equipment: {
                        "/equipment_types/main_hand": equipment("/items/furious_spear"),
                    },
                }),
                playerDto({
                    hrid: "player3",
                    staminaLevel: 500,
                    intelligenceLevel: 300,
                    defenseLevel: 500,
                }),
            ],
            target: {
                kind: "zone",
                zoneHrid: "/actions/combat/sorcerers_tower",
                difficultyTier: 0,
            },
            simulationTimeLimit: 45 * ONE_SECOND,
            random: { type: "sequence", values: [0], loop: true },
            options: detailedTraceOptions(50_000),
        });

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
        const execution = await runReferenceSimulation({
            contractVersion: 1,
            requestId: "complex-regal-sword-parry",
            dataVersion: "repository-v1.0.28",
            players: [playerDto({
                staminaLevel: 2_000,
                intelligenceLevel: 300,
                defenseLevel: 2_000,
                attackLevel: 10,
                meleeLevel: 10,
                rangedLevel: 10,
                magicLevel: 10,
                equipment: {
                    "/equipment_types/main_hand": equipment("/items/regal_sword"),
                },
            })],
            target: DURABLE_LABYRINTH_TARGET,
            simulationTimeLimit: 15 * ONE_SECOND,
            random: { type: "sequence", values: [0], loop: true },
            options: detailedTraceOptions(),
        });

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
