import { describe, expect, it } from "vitest";
import { runReferenceSimulation } from "../../services/referenceSimulationRunner.js";

const ONE_SECOND = 1e9;
const ZERO_SEQUENCE = { type: "sequence", values: [0], loop: true };

function trigger(dependencyHrid, conditionHrid, comparatorHrid, value = 0) {
    return { dependencyHrid, conditionHrid, comparatorHrid, value };
}

function ability(hrid, level = 1, triggers = []) {
    return { hrid, level, triggers };
}

function consumable(hrid, triggers = []) {
    return { hrid, triggers };
}

function equipment(hrid, enhancementLevel = 0) {
    return { hrid, enhancementLevel };
}

function player({
    hrid = "player1",
    staminaLevel = 500,
    intelligenceLevel = 500,
    attackLevel = 20,
    meleeLevel = 20,
    defenseLevel = 500,
    rangedLevel = 20,
    magicLevel = 20,
    equipment: equipmentSlots = {},
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
        equipment: equipmentSlots,
        abilities,
        food,
        drinks,
        houseRooms: {},
        guildBuffs: {},
        achievements: {},
        debuffOnLevelGap: 0,
    };
}

function traceOptions(maxEntries = 100_000) {
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

function run(request) {
    return runReferenceSimulation(request);
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
        const quickAidTrigger = trigger(
            "/combat_trigger_dependencies/all_allies",
            "/combat_trigger_conditions/lowest_hp_percentage",
            "/combat_trigger_comparators/less_than_equal",
            90,
        );
        const rejuvenateTrigger = trigger(
            "/combat_trigger_dependencies/all_allies",
            "/combat_trigger_conditions/missing_hp",
            "/combat_trigger_comparators/greater_than_equal",
            1,
        );
        const cakeTrigger = trigger(
            "/combat_trigger_dependencies/self",
            "/combat_trigger_conditions/missing_hp",
            "/combat_trigger_comparators/greater_than_equal",
            100,
        );

        const execution = await run({
            contractVersion: 1,
            requestId: "advanced-heal-hot",
            dataVersion: "repository-v1.0.28",
            players: [
                player({
                    hrid: "player1",
                    staminaLevel: 1_000,
                    defenseLevel: 20,
                    food: [consumable("/items/blueberry_cake", [cakeTrigger])],
                }),
                player({ hrid: "player2", staminaLevel: 700, defenseLevel: 100 }),
                player({
                    hrid: "player3",
                    staminaLevel: 2_000,
                    intelligenceLevel: 1_000,
                    defenseLevel: 2_000,
                    abilities: [
                        ability("/abilities/quick_aid", 1, [quickAidTrigger]),
                        ability("/abilities/rejuvenate", 1, [rejuvenateTrigger]),
                    ],
                }),
            ],
            target: {
                kind: "zone",
                zoneHrid: "/actions/combat/sorcerers_tower",
                difficultyTier: 0,
            },
            simulationTimeLimit: 60 * ONE_SECOND,
            random: ZERO_SEQUENCE,
            options: traceOptions(),
        });

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
        const execution = await run({
            contractVersion: 1,
            requestId: "advanced-pierce-multi-target",
            dataVersion: "repository-v1.0.28",
            players: [player({
                staminaLevel: 2_000,
                intelligenceLevel: 1_000,
                defenseLevel: 2_000,
                attackLevel: 10,
                rangedLevel: 10,
                abilities: [
                    ability("/abilities/penetrating_shot"),
                    ability("/abilities/sweep"),
                ],
            })],
            target: {
                kind: "zone",
                zoneHrid: "/actions/combat/sorcerers_tower",
                difficultyTier: 0,
            },
            simulationTimeLimit: 25 * ONE_SECOND,
            random: ZERO_SEQUENCE,
            options: traceOptions(),
        });

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
        const execution = await run({
            contractVersion: 1,
            requestId: "advanced-curse-stacking",
            dataVersion: "repository-v1.0.28",
            players: [player({
                staminaLevel: 2_000,
                defenseLevel: 2_000,
                attackLevel: 10,
                rangedLevel: 10,
                equipment: {
                    "/equipment_types/two_hand": equipment("/items/cursed_bow"),
                },
            })],
            target: {
                kind: "labyrinth",
                labyrinthHrid: "/monsters/crystal_colossus",
                roomLevel: 100,
                crates: [],
            },
            simulationTimeLimit: 25 * ONE_SECOND,
            random: ZERO_SEQUENCE,
            options: traceOptions(),
        });

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
        const reviveTrigger = trigger(
            "/combat_trigger_dependencies/all_allies",
            "/combat_trigger_conditions/number_of_dead_units",
            "/combat_trigger_comparators/greater_than_equal",
            1,
        );
        const execution = await run({
            contractVersion: 1,
            requestId: "advanced-revive-dead-ally",
            dataVersion: "repository-v1.0.28",
            players: [
                player({
                    hrid: "player1",
                    staminaLevel: 1,
                    intelligenceLevel: 1,
                    defenseLevel: 1,
                }),
                player({
                    hrid: "player2",
                    staminaLevel: 4_000,
                    intelligenceLevel: 2_000,
                    defenseLevel: 4_000,
                    abilities: [ability("/abilities/revive", 1, [reviveTrigger])],
                }),
            ],
            target: {
                kind: "labyrinth",
                labyrinthHrid: "/monsters/crystal_colossus",
                roomLevel: 100,
                crates: [],
            },
            simulationTimeLimit: 35 * ONE_SECOND,
            random: ZERO_SEQUENCE,
            options: traceOptions(),
        });

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
        const execution = await run({
            contractVersion: 1,
            requestId: "advanced-same-time-buff-expiration",
            dataVersion: "repository-v1.0.28",
            players: [player({
                staminaLevel: 10_000,
                intelligenceLevel: 2_000,
                defenseLevel: 10_000,
                attackLevel: 1,
                meleeLevel: 1,
                rangedLevel: 1,
                magicLevel: 1,
                abilities: [ability("/abilities/speed_aura")],
            })],
            target: {
                kind: "labyrinth",
                labyrinthHrid: "/monsters/crystal_colossus",
                roomLevel: 100,
                crates: [],
            },
            simulationTimeLimit: 121 * ONE_SECOND,
            random: ZERO_SEQUENCE,
            options: traceOptions(200_000),
        });

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
