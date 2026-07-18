import { readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const REPOSITORY_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const ACTION_DATA_PATH = path.join(REPOSITORY_ROOT, "src", "combatsimulator", "data", "actionDetailMap.json");
const MONSTER_DATA_PATH = path.join(REPOSITORY_ROOT, "src", "combatsimulator", "data", "combatMonsterDetailMap.json");

function combatTargetSummary(action) {
    const combatZoneInfo = action?.combatZoneInfo || {};
    const fightInfo = combatZoneInfo.fightInfo || {};
    const dungeonInfo = combatZoneInfo.dungeonInfo || {};
    return {
        hrid: action.hrid,
        name: action.name,
        maxDifficulty: Number(action.maxDifficulty || 0),
        maxPartySize: Number(action.maxPartySize || 0),
        isDungeon: combatZoneInfo.isDungeon === true,
        maxWaves: combatZoneInfo.isDungeon === true ? Number(dungeonInfo.maxWaves || 0) : null,
        randomSpawnCount: Number(fightInfo.randomSpawnInfo?.spawns?.length || 0),
        fixedWaveCount: combatZoneInfo.isDungeon === true
            ? Object.keys(dungeonInfo.fixedSpawnsMap || {}).length
            : null,
    };
}

async function main() {
    const [actionDetailMap, combatMonsterDetailMap] = await Promise.all([
        readFile(ACTION_DATA_PATH, "utf8").then(JSON.parse),
        readFile(MONSTER_DATA_PATH, "utf8").then(JSON.parse),
    ]);

    const combatTargets = Object.values(actionDetailMap)
        .filter((action) => action?.combatZoneInfo)
        .map(combatTargetSummary)
        .sort((left, right) => left.hrid.localeCompare(right.hrid));

    const output = {
        generatedFrom: {
            actions: path.relative(REPOSITORY_ROOT, ACTION_DATA_PATH),
            monsters: path.relative(REPOSITORY_ROOT, MONSTER_DATA_PATH),
        },
        counts: {
            zones: combatTargets.filter((target) => !target.isDungeon).length,
            dungeons: combatTargets.filter((target) => target.isDungeon).length,
            monsters: Object.keys(combatMonsterDetailMap).length,
        },
        dungeons: combatTargets.filter((target) => target.isDungeon),
        partyZones: combatTargets.filter((target) => !target.isDungeon && target.maxPartySize > 1),
        soloZones: combatTargets.filter((target) => !target.isDungeon && target.maxPartySize <= 1),
        labyrinthMonsterCandidates: Object.values(combatMonsterDetailMap)
            .map((monster) => ({
                hrid: monster.hrid,
                name: monster.name,
                enrageTime: Number(monster.enrageTime || 0),
            }))
            .sort((left, right) => left.hrid.localeCompare(right.hrid)),
    };

    process.stdout.write(`${JSON.stringify(output, null, 2)}\n`);
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
