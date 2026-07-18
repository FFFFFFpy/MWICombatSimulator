import { describe, expect, it } from "vitest";
import { attachCombatTrace } from "../combatTrace.js";

function createQueue() {
    const events = [];
    return {
        minHeap: {
            get length() {
                return events.length;
            },
            toArray() {
                return [...events];
            },
        },
        addEvent(event) {
            events.push(event);
        },
        clearMatching(predicate) {
            let cleared = false;
            for (let index = events.length - 1; index >= 0; index--) {
                if (predicate(events[index])) {
                    events.splice(index, 1);
                    cleared = true;
                }
            }
            return cleared;
        },
    };
}

function createUnit(hrid, isPlayer, hitpoints) {
    return {
        hrid,
        isPlayer,
        isOutOfMana: false,
        isStunned: false,
        isBlinded: false,
        isSilenced: false,
        stunExpireTime: null,
        blindExpireTime: null,
        silenceExpireTime: null,
        combatBuffs: {},
        abilities: [],
        food: [],
        drinks: [],
        combatDetails: {
            currentHitpoints: hitpoints,
            maxHitpoints: hitpoints,
            currentManapoints: isPlayer ? 50 : 0,
            maxManapoints: isPlayer ? 50 : 0,
            totalArmor: 10,
            combatStats: {
                attackInterval: 3_000_000_000,
                fury: 0,
                damageTaken: 0,
            },
        },
    };
}

function createSimulator() {
    const player = createUnit("player1", true, 100);
    const enemy = createUnit("/monsters/test", false, 80);
    const eventQueue = createQueue();
    const simulator = {
        players: [player],
        enemies: [enemy],
        eventQueue,
        simulationTime: 0,
        zone: { encountersKilled: 0, dungeonsCompleted: 0, dungeonsFailed: 0 },
        async processEvent(event) {
            this.simulationTime = event.time;
            enemy.combatDetails.currentHitpoints -= 12;
            this.traceController?.recordRandomDraw({ index: 0, value: 0.25 });
            eventQueue.addEvent({ type: "follow_up", time: event.time + 1, source: player, target: enemy });
            eventQueue.clearMatching((queued) => queued.type === "cancel_me");
        },
    };
    eventQueue.addEvent({ type: "cancel_me", time: 99, source: enemy, target: player });
    return simulator;
}

describe("combatTrace", () => {
    it("records event state changes, random draws, and queue operations", async () => {
        const simulator = createSimulator();
        const controller = attachCombatTrace(simulator);
        simulator.traceController = controller;

        await simulator.processEvent({
            type: "auto_attack",
            time: 10,
            source: simulator.players[0],
            target: simulator.enemies[0],
        });

        const trace = controller.getTrace();
        expect(trace.version).toBe(1);
        expect(trace.detailLevel).toBeUndefined();
        expect(trace.events).toHaveLength(1);
        expect(trace.events[0].before.players[0].combatState).toBeUndefined();
        expect(trace.events[0].event).toMatchObject({
            type: "auto_attack",
            time: 10,
            source: "player1",
            target: "/monsters/test",
        });
        expect(trace.events[0].changes.enemies).toEqual([
            {
                key: "enemy:0:/monsters/test",
                hrid: "/monsters/test",
                changes: { hitpoints: { before: 80, after: 68 } },
            },
        ]);
        expect(trace.events[0].randomDraws).toEqual([{ index: 0, value: 0.25 }]);
        expect(trace.events[0].queueOperations.map((entry) => entry.operation)).toEqual(["add", "cancel"]);
    });

    it("records buffs, cooldowns, OOM, and derived stats in combat detail mode", async () => {
        const simulator = createSimulator();
        const player = simulator.players[0];
        player.abilities = [{ hrid: "/abilities/test", lastUsed: -1, manaCost: 25 }];
        simulator.processEvent = async function processDetailedEvent(event) {
            this.simulationTime = event.time;
            player.isOutOfMana = true;
            player.combatDetails.currentManapoints = 0;
            player.combatDetails.combatStats.fury = 0.03;
            player.abilities[0].lastUsed = event.time;
            player.combatBuffs["/buff_uniques/test"] = {
                uniqueHrid: "/buff_uniques/test",
                typeHrid: "/buff_types/damage",
                ratioBoost: 0.2,
                flatBoost: 0,
                startTime: event.time,
                duration: 20,
            };
        };

        const controller = attachCombatTrace(simulator, { detailLevel: "combat" });
        await simulator.processEvent({ type: "abilityCastEnd", time: 10, source: player, ability: player.abilities[0] });

        const trace = controller.getTrace();
        const event = trace.events[0];
        expect(trace.detailLevel).toBe("combat");
        expect(event.before.players[0].combatState).toMatchObject({
            outOfMana: false,
            buffs: [],
            abilities: [{ hrid: "/abilities/test", lastUsed: -1, manaCost: 25 }],
        });
        expect(event.after.players[0].combatState).toMatchObject({
            outOfMana: true,
            stats: { fury: 0.03 },
            buffs: [{ uniqueHrid: "/buff_uniques/test", typeHrid: "/buff_types/damage" }],
            abilities: [{ hrid: "/abilities/test", lastUsed: 10, manaCost: 25 }],
        });
        expect(event.changes.players[0].changes.combatState).toBeTruthy();
    });

    it("keeps duplicate monster HRIDs as separate trace units", async () => {
        const simulator = createSimulator();
        simulator.enemies.push(createUnit("/monsters/test", false, 60));
        const controller = attachCombatTrace(simulator);

        await simulator.processEvent({ type: "auto_attack", time: 10 });

        const event = controller.getTrace().events[0];
        expect(event.before.enemies.map((unit) => unit.key)).toEqual([
            "enemy:0:/monsters/test",
            "enemy:1:/monsters/test",
        ]);
        expect(event.changes.enemies).toHaveLength(1);
        expect(event.changes.enemies[0].key).toBe("enemy:0:/monsters/test");
    });

    it("limits retained entries and reports truncation", async () => {
        const simulator = createSimulator();
        const controller = attachCombatTrace(simulator, { maxEntries: 1 });

        await simulator.processEvent({ type: "first", time: 1 });
        await simulator.processEvent({ type: "second", time: 2 });

        expect(controller.getTrace().events).toHaveLength(1);
        expect(controller.getTrace().truncatedEntries).toBe(1);
    });

    it("returns the existing controller when attached twice and can detach", () => {
        const simulator = createSimulator();
        const original = simulator.processEvent;
        const controller = attachCombatTrace(simulator);

        expect(attachCombatTrace(simulator)).toBe(controller);
        expect(simulator.processEvent).not.toBe(original);

        controller.detach();
        expect(simulator.__combatTraceController).toBeUndefined();
    });
});
