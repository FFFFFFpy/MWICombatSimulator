import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dataRoot = path.join(repositoryRoot, "src", "combatsimulator", "data");

async function readJson(name) {
    return JSON.parse(await readFile(path.join(dataRoot, name), "utf8"));
}

function nonZero(value) {
    return Number.isFinite(Number(value)) && Number(value) !== 0;
}

function compactObject(value) {
    return Object.fromEntries(
        Object.entries(value).filter(([, entry]) => {
            if (entry == null) return false;
            if (Array.isArray(entry)) return entry.length > 0;
            if (typeof entry === "object") return Object.keys(entry).length > 0;
            if (typeof entry === "string") return entry.length > 0;
            return true;
        }),
    );
}

function summarizeTrigger(trigger) {
    if (!trigger) return null;
    return compactObject({
        dependencyHrid: trigger.dependencyHrid,
        conditionHrid: trigger.conditionHrid,
        comparatorHrid: trigger.comparatorHrid,
        value: trigger.value,
    });
}

function summarizeBuff(buff) {
    if (!buff) return null;
    return compactObject({
        uniqueHrid: buff.uniqueHrid,
        typeHrid: buff.typeHrid,
        ratioBoost: buff.ratioBoost,
        ratioBoostLevelBonus: buff.ratioBoostLevelBonus,
        flatBoost: buff.flatBoost,
        flatBoostLevelBonus: buff.flatBoostLevelBonus,
        duration: buff.duration,
    });
}

function summarizeAbility(ability) {
    const effects = Array.isArray(ability.abilityEffects) ? ability.abilityEffects : [];
    const effectTypes = [...new Set(effects.map((effect) => effect.effectType).filter(Boolean))];
    const targetTypes = [...new Set(effects.map((effect) => effect.targetType).filter(Boolean))];
    const buffs = effects.flatMap((effect) => Array.isArray(effect.buffs) ? effect.buffs : []).map(summarizeBuff);
    const features = [];

    if (effects.some((effect) => nonZero(effect.damageOverTimeRatio) || nonZero(effect.damageOverTimeDuration))) features.push("dot");
    if (effects.some((effect) => String(effect.effectType || "").includes("heal"))) features.push("heal");
    if (effects.some((effect) => nonZero(effect.blindChance) || nonZero(effect.blindDuration))) features.push("blind");
    if (effects.some((effect) => nonZero(effect.silenceChance) || nonZero(effect.silenceDuration))) features.push("silence");
    if (effects.some((effect) => nonZero(effect.stunChance) || nonZero(effect.stunDuration))) features.push("stun");
    if (effects.some((effect) => nonZero(effect.pierceChance))) features.push("pierce");
    if (effects.some((effect) => nonZero(effect.armorDamageRatio))) features.push("armor_damage");
    if (effects.some((effect) => nonZero(effect.hpDrainRatio))) features.push("hp_drain");
    if (effects.some((effect) => nonZero(effect.spendHpRatio))) features.push("spend_hp");
    if (buffs.length > 0) features.push("buff");
    if (Number(ability.manaCost || 0) > 0) features.push("mana");
    if (targetTypes.some((target) => String(target).toLowerCase().includes("all"))) features.push("multi_target");

    return compactObject({
        hrid: ability.hrid,
        name: ability.name,
        description: ability.description,
        manaCost: ability.manaCost,
        cooldownDuration: ability.cooldownDuration,
        castDuration: ability.castDuration,
        isSpecialAbility: ability.isSpecialAbility === true,
        effectTypes,
        targetTypes,
        features,
        buffs,
        defaultCombatTriggers: (ability.defaultCombatTriggers || []).map(summarizeTrigger),
    });
}

function summarizeConsumable(item) {
    const detail = item.consumableDetail || {};
    const features = [];
    if (nonZero(detail.hitpointRestore)) features.push("hp_restore");
    if (nonZero(detail.manapointRestore)) features.push("mp_restore");
    if (nonZero(detail.recoveryDuration)) features.push("recovery");
    if (Array.isArray(detail.buffs) && detail.buffs.length > 0) features.push("buff");

    return compactObject({
        hrid: item.hrid,
        name: item.name,
        categoryHrid: item.categoryHrid,
        cooldownDuration: detail.cooldownDuration,
        hitpointRestore: detail.hitpointRestore,
        manapointRestore: detail.manapointRestore,
        recoveryDuration: detail.recoveryDuration,
        features,
        buffs: (detail.buffs || []).map(summarizeBuff),
        defaultCombatTriggers: (detail.defaultCombatTriggers || []).map(summarizeTrigger),
    });
}

function summarizeEquipment(item) {
    const detail = item.equipmentDetail || {};
    const stats = detail.combatStats || {};
    const nonZeroStats = Object.fromEntries(
        Object.entries(stats).filter(([, value]) => {
            if (Array.isArray(value)) return value.length > 0;
            if (typeof value === "string") return value.length > 0;
            return nonZero(value);
        }),
    );
    const interestingStatNames = [
        "physicalThorns",
        "elementalThorns",
        "parry",
        "retaliation",
        "pierce",
        "curse",
        "fury",
        "weaken",
        "ripple",
        "bloom",
        "blaze",
        "manaLeech",
        "lifeSteal",
        "criticalRate",
        "criticalDamage",
        "abilityHaste",
        "castSpeed",
        "foodHaste",
        "drinkConcentration",
        "maxManapoints",
        "mpRegenPer10",
    ];
    const features = interestingStatNames.filter((name) => nonZero(stats[name]));

    return compactObject({
        hrid: item.hrid,
        name: item.name,
        categoryHrid: item.categoryHrid,
        equipmentTypeHrid: detail.typeHrid || detail.equipmentTypeHrid,
        features,
        combatStats: nonZeroStats,
    });
}

function groupAbilities(abilities) {
    const featureNames = [
        "dot",
        "heal",
        "blind",
        "silence",
        "stun",
        "pierce",
        "armor_damage",
        "hp_drain",
        "spend_hp",
        "buff",
        "mana",
        "multi_target",
    ];
    return Object.fromEntries(
        featureNames.map((feature) => [feature, abilities.filter((ability) => ability.features.includes(feature))]),
    );
}

function groupEquipment(equipment) {
    const featureNames = [...new Set(equipment.flatMap((item) => item.features))].sort();
    return Object.fromEntries(
        featureNames.map((feature) => [feature, equipment.filter((item) => item.features.includes(feature))]),
    );
}

function collectTriggerVocabulary(abilities, consumables) {
    const triggers = [
        ...abilities.flatMap((ability) => ability.defaultCombatTriggers || []),
        ...consumables.flatMap((item) => item.defaultCombatTriggers || []),
    ];
    return {
        dependencies: [...new Set(triggers.map((trigger) => trigger.dependencyHrid).filter(Boolean))].sort(),
        conditions: [...new Set(triggers.map((trigger) => trigger.conditionHrid).filter(Boolean))].sort(),
        comparators: [...new Set(triggers.map((trigger) => trigger.comparatorHrid).filter(Boolean))].sort(),
    };
}

async function main() {
    const [abilityMap, itemMap] = await Promise.all([
        readJson("abilityDetailMap.json"),
        readJson("itemDetailMap.json"),
    ]);

    const abilities = Object.values(abilityMap).map(summarizeAbility).sort((left, right) => left.hrid.localeCompare(right.hrid));
    const items = Object.values(itemMap);
    const consumables = items
        .filter((item) => item?.consumableDetail)
        .map(summarizeConsumable)
        .sort((left, right) => left.hrid.localeCompare(right.hrid));
    const equipment = items
        .filter((item) => item?.equipmentDetail)
        .map(summarizeEquipment)
        .sort((left, right) => left.hrid.localeCompare(right.hrid));

    const report = {
        generatedFrom: {
            abilities: "src/combatsimulator/data/abilityDetailMap.json",
            items: "src/combatsimulator/data/itemDetailMap.json",
        },
        counts: {
            abilities: abilities.length,
            consumables: consumables.length,
            equipment: equipment.length,
        },
        triggerVocabulary: collectTriggerVocabulary(abilities, consumables),
        abilitiesByFeature: groupAbilities(abilities),
        consumables,
        equipmentByFeature: groupEquipment(equipment),
    };

    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
