const TRACE_DETAIL_BASIC = "basic";
const TRACE_DETAIL_COMBAT = "combat";

const COMBAT_DETAIL_FIELDS = [
    "staminaLevel",
    "intelligenceLevel",
    "attackLevel",
    "meleeLevel",
    "defenseLevel",
    "rangedLevel",
    "magicLevel",
    "stabAccuracyRating",
    "slashAccuracyRating",
    "smashAccuracyRating",
    "rangedAccuracyRating",
    "magicAccuracyRating",
    "stabMaxDamage",
    "slashMaxDamage",
    "smashMaxDamage",
    "rangedMaxDamage",
    "magicMaxDamage",
    "stabEvasionRating",
    "slashEvasionRating",
    "smashEvasionRating",
    "rangedEvasionRating",
    "magicEvasionRating",
    "defensiveMaxDamage",
    "totalArmor",
    "totalWaterResistance",
    "totalNatureResistance",
    "totalFireResistance",
    "abilityHaste",
    "tenacity",
    "totalThreat",
];

const COMBAT_STAT_FIELDS = [
    "combatStyleHrid",
    "damageType",
    "attackInterval",
    "autoAttackDamage",
    "abilityDamage",
    "criticalRate",
    "criticalDamage",
    "physicalAmplify",
    "waterAmplify",
    "natureAmplify",
    "fireAmplify",
    "healingAmplify",
    "physicalThorns",
    "elementalThorns",
    "lifeSteal",
    "hpRegenPer10",
    "mpRegenPer10",
    "armorPenetration",
    "waterPenetration",
    "naturePenetration",
    "firePenetration",
    "manaLeech",
    "castSpeed",
    "threat",
    "parry",
    "mayhem",
    "pierce",
    "curse",
    "ripple",
    "bloom",
    "blaze",
    "weaken",
    "fury",
    "foodHaste",
    "drinkConcentration",
    "damageTaken",
    "attackSpeed",
    "retaliation",
];

function normalizeTraceDetailLevel(value) {
    return value === TRACE_DETAIL_COMBAT ? TRACE_DETAIL_COMBAT : TRACE_DETAIL_BASIC;
}

function jsonScalar(value) {
    if (value == null || typeof value === "string" || typeof value === "boolean") {
        return value ?? null;
    }
    const number = Number(value);
    return Number.isFinite(number) ? number : String(value);
}

function pickFields(source, fields) {
    const result = {};
    for (const field of fields) {
        if (source?.[field] !== undefined) {
            result[field] = jsonScalar(source[field]);
        }
    }
    return result;
}

function buffSnapshots(unit) {
    return Object.entries(unit?.combatBuffs || {})
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, buff]) => ({
            key,
            uniqueHrid: String(buff?.uniqueHrid || key),
            typeHrid: String(buff?.typeHrid || ""),
            ratioBoost: Number(buff?.ratioBoost || 0),
            flatBoost: Number(buff?.flatBoost || 0),
            startTime: jsonScalar(buff?.startTime),
            duration: Number(buff?.duration || 0),
        }));
}

function cooldownSnapshots(values) {
    return (Array.isArray(values) ? values : [])
        .map((value, index) => value ? {
            index,
            hrid: String(value.hrid || ""),
            lastUsed: jsonScalar(value.lastUsed),
            manaCost: value.manaCost == null ? null : Number(value.manaCost),
        } : null)
        .filter(Boolean);
}

function combatStateSnapshot(unit) {
    const combatDetails = unit?.combatDetails || {};
    return {
        outOfMana: unit?.isOutOfMana === true,
        controlExpireTimes: {
            stun: jsonScalar(unit?.stunExpireTime),
            blind: jsonScalar(unit?.blindExpireTime),
            silence: jsonScalar(unit?.silenceExpireTime),
        },
        derived: pickFields(combatDetails, COMBAT_DETAIL_FIELDS),
        stats: pickFields(combatDetails.combatStats || {}, COMBAT_STAT_FIELDS),
        buffs: buffSnapshots(unit),
        abilities: cooldownSnapshots(unit?.abilities),
        food: cooldownSnapshots(unit?.food),
        drinks: cooldownSnapshots(unit?.drinks),
    };
}

function unitSnapshot(unit, index, group, detailLevel) {
    if (!unit) return null;
    const hrid = String(unit.hrid || "");
    const snapshot = {
        key: `${group}:${index}:${hrid}`,
        hrid,
        index,
        isPlayer: unit.isPlayer === true,
        hitpoints: Number(unit.combatDetails?.currentHitpoints || 0),
        maxHitpoints: Number(unit.combatDetails?.maxHitpoints || 0),
        manapoints: Number(unit.combatDetails?.currentManapoints || 0),
        maxManapoints: Number(unit.combatDetails?.maxManapoints || 0),
        stunned: unit.isStunned === true,
        blinded: unit.isBlinded === true,
        silenced: unit.isSilenced === true,
    };
    if (detailLevel === TRACE_DETAIL_COMBAT) {
        snapshot.combatState = combatStateSnapshot(unit);
    }
    return snapshot;
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

function eventQueueSize(simulator) {
    const heap = simulator?.eventQueue?.minHeap;
    if (Number.isFinite(Number(heap?.length))) {
        return Number(heap.length);
    }
    if (typeof heap?.size === "function") {
        return Number(heap.size()) || 0;
    }
    if (Number.isFinite(Number(heap?.size))) {
        return Number(heap.size);
    }
    return heap?.toArray?.().length || 0;
}

function simulationSnapshot(simulator, detailLevel) {
    const players = Array.isArray(simulator?.players) ? simulator.players : [];
    const enemies = Array.isArray(simulator?.enemies) ? simulator.enemies : [];
    return {
        simulationTime: Number(simulator?.simulationTime || 0),
        players: players.map((unit, index) => unitSnapshot(unit, index, "player", detailLevel)),
        enemies: enemies.map((unit, index) => unitSnapshot(unit, index, "enemy", detailLevel)),
        eventQueueSize: eventQueueSize(simulator),
        encountersKilled: Number(simulator?.zone?.encountersKilled || 0),
        dungeonsCompleted: Number(simulator?.zone?.dungeonsCompleted || 0),
        dungeonsFailed: Number(simulator?.zone?.dungeonsFailed || 0),
    };
}

function valuesEqual(left, right) {
    if (Object.is(left, right)) return true;
    return JSON.stringify(left) === JSON.stringify(right);
}

function diffUnits(beforeUnits, afterUnits) {
    const beforeMap = new Map((beforeUnits || []).filter(Boolean).map((unit) => [unit.key, unit]));
    const afterMap = new Map((afterUnits || []).filter(Boolean).map((unit) => [unit.key, unit]));
    const changes = [];

    for (const key of new Set([...beforeMap.keys(), ...afterMap.keys()])) {
        const before = beforeMap.get(key) || null;
        const after = afterMap.get(key) || null;
        const hrid = after?.hrid || before?.hrid || "";
        if (!before || !after) {
            changes.push({ key, hrid, before, after });
            continue;
        }
        const patch = {};
        for (const field of [
            "hitpoints",
            "maxHitpoints",
            "manapoints",
            "maxManapoints",
            "stunned",
            "blinded",
            "silenced",
            "combatState",
        ]) {
            if (!valuesEqual(before[field], after[field])) {
                patch[field] = { before: before[field] ?? null, after: after[field] ?? null };
            }
        }
        if (Object.keys(patch).length > 0) {
            changes.push({ key, hrid, changes: patch });
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

export function attachCombatTrace(simulator, { maxEntries = 100_000, detailLevel = TRACE_DETAIL_BASIC } = {}) {
    if (!simulator || typeof simulator.processEvent !== "function") {
        throw new TypeError("attachCombatTrace requires a combat simulator instance.");
    }
    if (simulator.__combatTraceController) {
        return simulator.__combatTraceController;
    }

    const normalizedDetailLevel = normalizeTraceDetailLevel(detailLevel);
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
        const before = simulationSnapshot(simulator, normalizedDetailLevel);
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
            entry.after = simulationSnapshot(simulator, normalizedDetailLevel);
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
        detailLevel: normalizedDetailLevel,
        recordRandomDraw(draw) {
            const normalized = {
                index: Number(draw?.index || 0),
                value: Number(draw?.value || 0),
            };
            if (activeEntry) activeEntry.randomDraws.push(normalized);
            else orphanRandomDraws.push(normalized);
        },
        getTrace() {
            const trace = {
                version: 1,
                events,
                orphanRandomDraws,
                truncatedEntries,
            };
            if (normalizedDetailLevel === TRACE_DETAIL_COMBAT) {
                trace.detailLevel = TRACE_DETAIL_COMBAT;
            }
            return trace;
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
