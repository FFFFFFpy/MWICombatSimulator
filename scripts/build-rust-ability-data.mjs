import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const REPOSITORY_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const INPUT_PATH = path.join(
    REPOSITORY_ROOT,
    "src",
    "combatsimulator",
    "data",
    "abilityDetailMap.json",
);
const OUTPUT_PATH = path.join(
    REPOSITORY_ROOT,
    "crates",
    "sim-core",
    "data",
    "ability-data-v1.json",
);

const SCHEMA_VERSION = 1;
const DATA_VERSION = "repository-v1.0.28";

const EFFECT_NUMBER_FIELDS = [
    "baseDamageFlat",
    "baseDamageFlatLevelBonus",
    "baseDamageRatio",
    "baseDamageRatioLevelBonus",
    "bonusAccuracyRatio",
    "bonusAccuracyRatioLevelBonus",
    "damageOverTimeRatio",
    "damageOverTimeDuration",
    "armorDamageRatio",
    "armorDamageRatioLevelBonus",
    "hpDrainRatio",
    "pierceChance",
    "blindChance",
    "blindDuration",
    "silenceChance",
    "silenceDuration",
    "stunChance",
    "stunDuration",
    "spendHpRatio",
];

const PROBABILITY_FIELDS = [
    "pierceChance",
    "blindChance",
    "silenceChance",
    "stunChance",
];

const NON_NEGATIVE_FIELDS = [
    "damageOverTimeDuration",
    "blindDuration",
    "silenceDuration",
    "stunDuration",
];

function parseArguments(argv) {
    const options = { check: false };
    for (const argument of argv) {
        if (argument === "--check") {
            options.check = true;
        } else if (argument === "--help" || argument === "-h") {
            options.help = true;
        } else {
            throw new Error(`Unknown argument: ${argument}`);
        }
    }
    return options;
}

function printHelp() {
    console.log(
        "Usage: node scripts/build-rust-ability-data.mjs [--check]\n\n" +
        `Without --check, writes ${path.relative(REPOSITORY_ROOT, OUTPUT_PATH)}.\n` +
        "With --check, compares the committed snapshot byte-for-byte.",
    );
}

function sha256(text) {
    return createHash("sha256").update(text, "utf8").digest("hex");
}

function canonicalize(value) {
    if (Array.isArray(value)) {
        return value.map(canonicalize);
    }
    if (value && typeof value === "object") {
        return Object.fromEntries(
            Object.keys(value)
                .sort((left, right) => left.localeCompare(right))
                .map((key) => [key, canonicalize(value[key])]),
        );
    }
    return value;
}

function finiteNumber(value, field) {
    const number = Number(value);
    if (!Number.isFinite(number)) {
        throw new Error(`${field} must be finite; received ${String(value)}`);
    }
    return number;
}

function nonNegativeNumber(value, field) {
    const number = finiteNumber(value, field);
    if (number < 0) {
        throw new Error(`${field} must be non-negative; received ${number}`);
    }
    return number;
}

function requiredString(value, field) {
    const text = String(value ?? "");
    if (!text) {
        throw new Error(`${field} is required.`);
    }
    return text;
}

function optionalString(value) {
    return value == null ? "" : String(value);
}

function normalizeTrigger(trigger, location) {
    if (!trigger || typeof trigger !== "object" || Array.isArray(trigger)) {
        throw new Error(`${location} must be an object.`);
    }
    return canonicalize({
        ...trigger,
        dependencyHrid: requiredString(trigger.dependencyHrid, `${location}.dependencyHrid`),
        conditionHrid: requiredString(trigger.conditionHrid, `${location}.conditionHrid`),
        comparatorHrid: requiredString(trigger.comparatorHrid, `${location}.comparatorHrid`),
        value: finiteNumber(trigger.value ?? 0, `${location}.value`),
    });
}

function normalizeEffect(effect, location) {
    if (!effect || typeof effect !== "object" || Array.isArray(effect)) {
        throw new Error(`${location} must be an object.`);
    }

    const normalized = {
        ...effect,
        targetType: requiredString(effect.targetType, `${location}.targetType`),
        effectType: requiredString(effect.effectType, `${location}.effectType`),
        combatStyleHrid: optionalString(effect.combatStyleHrid),
        damageType: optionalString(effect.damageType),
        buffs: effect.buffs == null
            ? null
            : Array.isArray(effect.buffs)
                ? effect.buffs.map(canonicalize)
                : (() => { throw new Error(`${location}.buffs must be null or an array.`); })(),
    };

    for (const field of EFFECT_NUMBER_FIELDS) {
        normalized[field] = finiteNumber(effect[field] ?? 0, `${location}.${field}`);
    }
    for (const field of PROBABILITY_FIELDS) {
        if (normalized[field] < 0 || normalized[field] > 1) {
            throw new Error(`${location}.${field} must be in [0, 1]; received ${normalized[field]}`);
        }
    }
    for (const field of NON_NEGATIVE_FIELDS) {
        if (normalized[field] < 0) {
            throw new Error(`${location}.${field} must be non-negative; received ${normalized[field]}`);
        }
    }

    return canonicalize(normalized);
}

function normalizeAbility(mapKey, ability) {
    if (!ability || typeof ability !== "object" || Array.isArray(ability)) {
        throw new Error(`${mapKey} must be an ability object.`);
    }
    const hrid = requiredString(ability.hrid, `${mapKey}.hrid`);
    if (hrid !== mapKey) {
        throw new Error(`Ability map key ${mapKey} does not match row hrid ${hrid}.`);
    }
    const effects = Array.isArray(ability.abilityEffects)
        ? ability.abilityEffects.map((effect, index) => normalizeEffect(
            effect,
            `${hrid}.abilityEffects[${index}]`,
        ))
        : [];
    if (effects.length === 0) {
        throw new Error(`${hrid}.abilityEffects must not be empty.`);
    }
    const triggers = Array.isArray(ability.defaultCombatTriggers)
        ? ability.defaultCombatTriggers.map((trigger, index) => normalizeTrigger(
            trigger,
            `${hrid}.defaultCombatTriggers[${index}]`,
        ))
        : [];

    return canonicalize({
        ...ability,
        hrid,
        name: optionalString(ability.name),
        description: optionalString(ability.description),
        isSpecialAbility: ability.isSpecialAbility === true,
        manaCost: nonNegativeNumber(ability.manaCost ?? 0, `${hrid}.manaCost`),
        cooldownDuration: nonNegativeNumber(
            ability.cooldownDuration ?? 0,
            `${hrid}.cooldownDuration`,
        ),
        castDuration: nonNegativeNumber(ability.castDuration ?? 0, `${hrid}.castDuration`),
        abilityEffects: effects,
        defaultCombatTriggers: triggers,
        sortIndex: finiteNumber(ability.sortIndex ?? 0, `${hrid}.sortIndex`),
    });
}

function buildSnapshot(abilityMap, sourceHash) {
    const entries = Object.entries(abilityMap)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([hrid, ability]) => [hrid, normalizeAbility(hrid, ability)]);
    if (entries.length === 0) {
        throw new Error("Ability map is empty.");
    }
    return canonicalize({
        schemaVersion: SCHEMA_VERSION,
        dataVersion: DATA_VERSION,
        source: {
            path: path.relative(REPOSITORY_ROOT, INPUT_PATH).replaceAll("\\", "/"),
            sha256: sourceHash,
        },
        count: entries.length,
        abilities: Object.fromEntries(entries),
    });
}

async function main() {
    const options = parseArguments(process.argv.slice(2));
    if (options.help) {
        printHelp();
        return;
    }

    const inputText = await readFile(INPUT_PATH, "utf8");
    const snapshot = buildSnapshot(JSON.parse(inputText), sha256(inputText));
    const output = `${JSON.stringify(snapshot, null, 2)}\n`;

    if (options.check) {
        const committed = await readFile(OUTPUT_PATH, "utf8").catch(() => null);
        if (committed !== output) {
            throw new Error(
                "Rust ability data snapshot is stale. Run npm run build-rust-ability-data and commit the result.",
            );
        }
        console.log(`Verified Rust ability data snapshot: ${path.relative(REPOSITORY_ROOT, OUTPUT_PATH)}`);
        return;
    }

    await mkdir(path.dirname(OUTPUT_PATH), { recursive: true });
    await writeFile(OUTPUT_PATH, output, "utf8");
    console.log(`Generated Rust ability data snapshot: ${path.relative(REPOSITORY_ROOT, OUTPUT_PATH)}`);
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
