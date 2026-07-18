import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const REPOSITORY_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const ACTION_DATA_PATH = path.join(
    REPOSITORY_ROOT,
    "src",
    "combatsimulator",
    "data",
    "actionDetailMap.json",
);
const MONSTER_DATA_PATH = path.join(
    REPOSITORY_ROOT,
    "src",
    "combatsimulator",
    "data",
    "combatMonsterDetailMap.json",
);
const OUTPUT_PATH = path.join(
    REPOSITORY_ROOT,
    "crates",
    "sim-core",
    "data",
    "combat-zone-data-v1.json",
);

const SCHEMA_VERSION = 1;
const DATA_VERSION = "repository-v1.0.28";

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
    console.log(`Usage: node scripts/build-rust-combat-data.mjs [--check]\n\n` +
        `Without --check, writes ${path.relative(REPOSITORY_ROOT, OUTPUT_PATH)}.\n` +
        `With --check, compares the committed snapshot byte-for-byte.`);
}

function sha256(text) {
    return createHash("sha256").update(text, "utf8").digest("hex");
}

function canonicalize(value) {
    if (Array.isArray(value)) {
        // Array order is semantic for weighted spawn selection and ability slots.
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

function positiveNumber(value, field) {
    const number = finiteNumber(value, field);
    if (number <= 0) {
        throw new Error(`${field} must be positive; received ${number}`);
    }
    return number;
}

function normalizedSpawn(spawn, location) {
    if (!spawn || typeof spawn !== "object") {
        throw new Error(`${location} must be an object.`);
    }
    const combatMonsterHrid = String(spawn.combatMonsterHrid || "");
    if (!combatMonsterHrid) {
        throw new Error(`${location}.combatMonsterHrid is required.`);
    }
    return canonicalize({
        combatMonsterHrid,
        difficultyTier: nonNegativeNumber(spawn.difficultyTier || 0, `${location}.difficultyTier`),
        rate: positiveNumber(spawn.rate, `${location}.rate`),
        strength: positiveNumber(spawn.strength, `${location}.strength`),
    });
}

function normalizedBossSpawn(spawn, location) {
    if (!spawn || typeof spawn !== "object") {
        throw new Error(`${location} must be an object.`);
    }
    const combatMonsterHrid = String(spawn.combatMonsterHrid || "");
    if (!combatMonsterHrid) {
        throw new Error(`${location}.combatMonsterHrid is required.`);
    }
    return canonicalize({
        combatMonsterHrid,
        difficultyTier: nonNegativeNumber(spawn.difficultyTier || 0, `${location}.difficultyTier`),
    });
}

function normalizedZone(action) {
    const zoneHrid = String(action?.hrid || "");
    if (!zoneHrid) {
        throw new Error("A combat action is missing hrid.");
    }
    const combatZoneInfo = action.combatZoneInfo;
    if (!combatZoneInfo || combatZoneInfo.isDungeon === true) {
        throw new Error(`${zoneHrid} is not a normal Combat Zone.`);
    }
    const fightInfo = combatZoneInfo.fightInfo || {};
    const randomSpawnInfo = fightInfo.randomSpawnInfo || {};
    const spawns = Array.isArray(randomSpawnInfo.spawns)
        ? randomSpawnInfo.spawns.map((spawn, index) => normalizedSpawn(
            spawn,
            `${zoneHrid}.randomSpawnInfo.spawns[${index}]`,
        ))
        : [];
    if (spawns.length === 0) {
        throw new Error(`${zoneHrid} has no random spawn rules.`);
    }
    const maxSpawnCount = positiveNumber(
        randomSpawnInfo.maxSpawnCount,
        `${zoneHrid}.randomSpawnInfo.maxSpawnCount`,
    );
    const maxTotalStrength = positiveNumber(
        randomSpawnInfo.maxTotalStrength,
        `${zoneHrid}.randomSpawnInfo.maxTotalStrength`,
    );
    const bossSpawns = Array.isArray(fightInfo.bossSpawns)
        ? fightInfo.bossSpawns.map((spawn, index) => normalizedBossSpawn(
            spawn,
            `${zoneHrid}.bossSpawns[${index}]`,
        ))
        : [];

    return canonicalize({
        hrid: zoneHrid,
        name: String(action.name || ""),
        baseTimeCost: nonNegativeNumber(action.baseTimeCost || 0, `${zoneHrid}.baseTimeCost`),
        maxDifficulty: nonNegativeNumber(action.maxDifficulty || 0, `${zoneHrid}.maxDifficulty`),
        maxPartySize: positiveNumber(action.maxPartySize || 1, `${zoneHrid}.maxPartySize`),
        buffs: Array.isArray(action.buffs) ? action.buffs : [],
        randomSpawnInfo: {
            maxSpawnCount,
            maxTotalStrength,
            spawns,
        },
        bossSpawns,
    });
}

function normalizedMonster(monster) {
    const hrid = String(monster?.hrid || "");
    if (!hrid) {
        throw new Error("A referenced monster is missing hrid.");
    }
    const combatDetails = monster.combatDetails || {};
    const levels = {};
    for (const name of [
        "staminaLevel",
        "intelligenceLevel",
        "attackLevel",
        "meleeLevel",
        "defenseLevel",
        "rangedLevel",
        "magicLevel",
    ]) {
        levels[name] = finiteNumber(combatDetails[name] || 0, `${hrid}.combatDetails.${name}`);
    }

    return canonicalize({
        hrid,
        name: String(monster.name || ""),
        levels,
        baseAttackInterval: nonNegativeNumber(
            combatDetails.combatStats?.attackInterval ?? combatDetails.attackInterval ?? 0,
            `${hrid}.combatDetails.attackInterval`,
        ),
        combatStats: combatDetails.combatStats && typeof combatDetails.combatStats === "object"
            ? combatDetails.combatStats
            : {},
        abilities: Array.isArray(monster.abilities) ? monster.abilities : [],
        experience: nonNegativeNumber(monster.experience || 0, `${hrid}.experience`),
        enrageTime: nonNegativeNumber(monster.enrageTime || 0, `${hrid}.enrageTime`),
        dropTable: Array.isArray(monster.dropTable) ? monster.dropTable : [],
        rareDropTable: Array.isArray(monster.rareDropTable) ? monster.rareDropTable : [],
    });
}

function sortedObject(entries) {
    return Object.fromEntries(
        [...entries].sort(([left], [right]) => left.localeCompare(right)),
    );
}

function buildSnapshot(actionMap, monsterMap, sourceHashes) {
    const zones = Object.values(actionMap)
        .filter((action) => action?.combatZoneInfo && action.combatZoneInfo.isDungeon !== true)
        .map(normalizedZone)
        .sort((left, right) => left.hrid.localeCompare(right.hrid));
    if (zones.length === 0) {
        throw new Error("No normal Combat Zones were found.");
    }

    const referencedMonsterHrids = new Set();
    for (const zone of zones) {
        for (const spawn of zone.randomSpawnInfo.spawns) {
            referencedMonsterHrids.add(spawn.combatMonsterHrid);
        }
        for (const spawn of zone.bossSpawns) {
            referencedMonsterHrids.add(spawn.combatMonsterHrid);
        }
    }

    const monsters = [];
    for (const hrid of [...referencedMonsterHrids].sort((left, right) => left.localeCompare(right))) {
        const monster = monsterMap[hrid];
        if (!monster) {
            throw new Error(`Combat Zone spawn references missing monster ${hrid}.`);
        }
        monsters.push(normalizedMonster(monster));
    }

    const snapshot = canonicalize({
        schemaVersion: SCHEMA_VERSION,
        dataVersion: DATA_VERSION,
        sources: {
            actions: {
                path: path.relative(REPOSITORY_ROOT, ACTION_DATA_PATH).replaceAll("\\", "/"),
                sha256: sourceHashes.actions,
            },
            monsters: {
                path: path.relative(REPOSITORY_ROOT, MONSTER_DATA_PATH).replaceAll("\\", "/"),
                sha256: sourceHashes.monsters,
            },
        },
        counts: {
            zones: zones.length,
            monsters: monsters.length,
        },
        zones: sortedObject(zones.map((zone) => [zone.hrid, zone])),
        monsters: sortedObject(monsters.map((monster) => [monster.hrid, monster])),
    });

    for (const [zoneHrid, zone] of Object.entries(snapshot.zones)) {
        if (zoneHrid !== zone.hrid) {
            throw new Error(`Zone map key ${zoneHrid} does not match row hrid ${zone.hrid}.`);
        }
        for (const spawn of [...zone.randomSpawnInfo.spawns, ...zone.bossSpawns]) {
            if (!snapshot.monsters[spawn.combatMonsterHrid]) {
                throw new Error(`${zoneHrid} references absent monster ${spawn.combatMonsterHrid}.`);
            }
        }
    }

    return snapshot;
}

async function main() {
    const options = parseArguments(process.argv.slice(2));
    if (options.help) {
        printHelp();
        return;
    }

    const [actionText, monsterText] = await Promise.all([
        readFile(ACTION_DATA_PATH, "utf8"),
        readFile(MONSTER_DATA_PATH, "utf8"),
    ]);
    const snapshot = buildSnapshot(
        JSON.parse(actionText),
        JSON.parse(monsterText),
        { actions: sha256(actionText), monsters: sha256(monsterText) },
    );
    const output = `${JSON.stringify(snapshot, null, 2)}\n`;

    if (options.check) {
        const committed = await readFile(OUTPUT_PATH, "utf8").catch(() => null);
        if (committed !== output) {
            throw new Error(
                "Rust combat data snapshot is stale. Run npm run build-rust-combat-data and commit the result.",
            );
        }
        console.log(`Verified Rust combat data snapshot: ${path.relative(REPOSITORY_ROOT, OUTPUT_PATH)}`);
        return;
    }

    await mkdir(path.dirname(OUTPUT_PATH), { recursive: true });
    await writeFile(OUTPUT_PATH, output, "utf8");
    console.log(`Generated Rust combat data snapshot: ${path.relative(REPOSITORY_ROOT, OUTPUT_PATH)}`);
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
