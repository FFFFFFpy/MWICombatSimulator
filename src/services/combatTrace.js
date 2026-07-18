function unitSnapshot(unit) {
    if (!unit) return null;
    return {
        hrid: String(unit.hrid || ""),
        isPlayer: unit.isPlayer === true,
        hitpoints: Number(unit.combatDetails?.currentHitpoints || 0),
        maxHitpoints: Number(unit.combatDetails?.maxHitpoints || 0),
        manapoints: Number(unit.combatDetails?.currentManapoints || 0),
        maxManapoints: Number(unit.combatDetails?.maxManapoints || 0),
        stunned: unit.isStunned === true,
        blinded: unit.isBlinded === true,
        silenced: unit.isSilenced === true,
    };
}

function eventSnapshot(event) {
    if (!event) return null;
    return {
        type: String(event.type || event.constructor?.type || event.constructor?.name || "unknown"),
        time: Number(event.time || 0),
        hrid: event.hrid ? String(event.hrid) : null,
        source: event.source?.hrid ? String(event.source.hrid) : null,
        target: event.target?.hrid ? String(event.target.hrid) : null,
        ability: event.ability?.hrid ? String(event.ability.hrid) : null,
        consumable: event.consumable?.hrid ? String(event.consumable.hrid) : null,
    };
}

function simulationSnapshot(simulator) {
    const players = Array.isArray(simulator?.players) ? simulator.players : [];
    const enemies = Array.isArray(simulator?.enemies) ? simulator.enemies : [];
    return {
        simulationTime: Number(simulator?.simulationTime || 0),
        players: players.map(unitSnapshot),
        enemies: enemies.map(unitSnapshot),
        eventQueueSize: Number(simulator?.eventQueue?.minHeap?.length || simulator?.eventQueue?.minHeap?.size || 0),
        encountersKilled: Number(simulator?.zone?.encountersKilled || 0),
        dungeonsCompleted: Number(simulator?.zone?.dungeonsCompleted || 0),
        dungeonsFailed: Number(simulator?.zone?.dungeonsFailed || 0),
    };
}

function diffUnits(beforeUnits, afterUnits) {
    const beforeMap = new Map((beforeUnits || []).map((unit) => [unit.hrid, unit]));
    const afterMap = new Map((afterUnits || []).map((unit) => [unit.hrid, unit]));
    const changes = [];

    for (const hrid of new Set([...beforeMap.keys(), ...afterMap.keys()])) {
        const before = beforeMap.get(hrid) || null;
        const after = afterMap.get(hrid) || null;
        if (!before || !after) {
            changes.push({ hrid, before, after });
            continue;
        }
        const patch = {};
        for (const key of [
            "hitpoints",
            "maxHitpoints",
            "manapoints",
            "maxManapoints",
            "stunned",
            "blinded",
            "silenced",
        ]) {
            if (before[key] !== after[key]) {
                patch[key] = { before: before[key], after: after[key] };
            }
        }
        if (Object.keys(patch).length > 0) {
            changes.push({ hrid, changes: patch });
        }
    }

    return changes;
}

function stateDiff(before, after) {
    return {
        players: diffUnits(before?.players, after?.players),
        enemies: diffUnits(before?.enemies, after?.enemies),
        eventQueueSize: before?.eventQueueSize === after?.eventQueueSize
            ? null
            : { before: before?.eventQueueSize || 0, after: after?.eventQueueSize || 0 },
        encountersKilled: before?.encountersKilled === after?.encountersKilled
            ? null
            : { before: before?.encountersKilled || 0, after: after?.encountersKilled || 0 },
        dungeonsCompleted: before?.dungeonsCompleted === after?.dungeonsCompleted
            ? null
            : { before: before?.dungeonsCompleted || 0, after: after?.dungeonsCompleted || 0 },
        dungeonsFailed: before?.dungeonsFailed === after?.dungeonsFailed
            ? null
            : { before: before?.dungeonsFailed || 0, after: after?.dungeonsFailed || 0 },
    };
}

export function attachCombatTrace(simulator, { maxEntries = 100_000 } = {}) {
    if (!simulator || typeof simulator.processEvent !== "function") {
        throw new TypeError("attachCombatTrace requires a combat simulator instance.");
    }
    if (simulator.__combatTraceController) {
        return simulator.__combatTraceController;
    }

    const events = [];
    const orphanRandomDraws = [];
    let truncatedEntries = 0;
    let activeEntry = null;
    let sequence = 0;

    const originalProcessEvent = simulator.processEvent.bind(simulator);
    const queue = simulator.eventQueue;
    const originalAddEvent = queue?.addEvent?.bind(queue);
    const originalClearMatching = queue?.clearMatching?.bind(queue);

    if (originalAddEvent) {
        queue.addEvent = (event) => {
            if (activeEntry) {
                activeEntry.queueOperations.push({ operation: "add", event: eventSnapshot(event) });
            }
            return originalAddEvent(event);
        };
    }

    if (originalClearMatching) {
        queue.clearMatching = (predicate) => {
            const before = queue.minHeap?.toArray?.() || [];
            const result = originalClearMatching(predicate);
            if (activeEntry && result) {
                const after = new Set(queue.minHeap?.toArray?.() || []);
                for (const event of before) {
                    if (!after.has(event)) {
                        activeEntry.queueOperations.push({ operation: "cancel", event: eventSnapshot(event) });
                    }
                }
            }
            return result;
        };
    }

    simulator.processEvent = async (event) => {
        const before = simulationSnapshot(simulator);
        const entry = {
            sequence: sequence++,
            event: eventSnapshot(event),
            before,
            after: null,
            changes: null,
            queueOperations: [],
            randomDraws: [],
            error: null,
        };
        activeEntry = entry;

        try {
            return await originalProcessEvent(event);
        } catch (error) {
            entry.error = error?.message || String(error);
            throw error;
        } finally {
            entry.after = simulationSnapshot(simulator);
            entry.changes = stateDiff(entry.before, entry.after);
            activeEntry = null;
            if (events.length < Math.max(1, Number(maxEntries) || 1)) {
                events.push(entry);
            } else {
                truncatedEntries += 1;
            }
        }
    };

    const controller = {
        version: 1,
        recordRandomDraw(draw) {
            const normalized = {
                index: Number(draw?.index || 0),
                value: Number(draw?.value || 0),
            };
            if (activeEntry) activeEntry.randomDraws.push(normalized);
            else orphanRandomDraws.push(normalized);
        },
        getTrace() {
            return {
                version: 1,
                events,
                orphanRandomDraws,
                truncatedEntries,
            };
        },
        detach() {
            simulator.processEvent = originalProcessEvent;
            if (originalAddEvent) queue.addEvent = originalAddEvent;
            if (originalClearMatching) queue.clearMatching = originalClearMatching;
            delete simulator.__combatTraceController;
        },
    };

    simulator.__combatTraceController = controller;
    return controller;
}
