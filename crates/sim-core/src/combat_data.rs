use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Result, SimError, SimTime};

#[cfg(test)]
use crate::{BASIC_FLY_MONSTER_HRID, BASIC_FLY_ZONE_HRID, BasicGameData};

pub const COMBAT_ZONE_DATA_SCHEMA_VERSION: u32 = 1;
pub const COMBAT_ZONE_DATA_VERSION: &str = "repository-v1.0.28";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatDataSourceV1 {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatDataSourcesV1 {
    pub actions: CombatDataSourceV1,
    pub monsters: CombatDataSourceV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatDataCountsV1 {
    pub zones: usize,
    pub monsters: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatSpawnRuleV1 {
    pub combat_monster_hrid: String,
    pub difficulty_tier: u32,
    pub rate: f64,
    pub strength: f64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatBossSpawnV1 {
    pub combat_monster_hrid: String,
    pub difficulty_tier: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatRandomSpawnInfoV1 {
    pub max_spawn_count: u32,
    pub max_total_strength: f64,
    pub spawns: Vec<CombatSpawnRuleV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatZoneDataV1 {
    pub hrid: String,
    pub name: String,
    pub base_time_cost: f64,
    pub max_difficulty: u32,
    pub max_party_size: u32,
    pub buffs: Vec<Value>,
    pub random_spawn_info: CombatRandomSpawnInfoV1,
    pub boss_spawns: Vec<CombatBossSpawnV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatMonsterLevelsV1 {
    pub stamina_level: f64,
    pub intelligence_level: f64,
    pub attack_level: f64,
    pub melee_level: f64,
    pub defense_level: f64,
    pub ranged_level: f64,
    pub magic_level: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatMonsterDataV1 {
    pub hrid: String,
    pub name: String,
    pub levels: CombatMonsterLevelsV1,
    pub base_attack_interval: SimTime,
    pub combat_stats: Value,
    pub abilities: Vec<Value>,
    pub experience: f64,
    pub enrage_time: SimTime,
    pub drop_table: Vec<Value>,
    pub rare_drop_table: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatZoneDataSnapshotV1 {
    pub schema_version: u32,
    pub data_version: String,
    pub sources: CombatDataSourcesV1,
    pub counts: CombatDataCountsV1,
    pub zones: BTreeMap<String, CombatZoneDataV1>,
    pub monsters: BTreeMap<String, CombatMonsterDataV1>,
}

impl CombatZoneDataSnapshotV1 {
    pub fn embedded() -> Result<Self> {
        let snapshot: Self =
            serde_json::from_str(include_str!("../data/combat-zone-data-v1.json"))?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != COMBAT_ZONE_DATA_SCHEMA_VERSION {
            return Err(invalid(format!(
                "unsupported Combat Zone data schema {}; expected {}",
                self.schema_version, COMBAT_ZONE_DATA_SCHEMA_VERSION
            )));
        }
        if self.data_version != COMBAT_ZONE_DATA_VERSION {
            return Err(invalid(format!(
                "unexpected Combat Zone data version {}; expected {COMBAT_ZONE_DATA_VERSION}",
                self.data_version
            )));
        }
        validate_source("sources.actions", &self.sources.actions)?;
        validate_source("sources.monsters", &self.sources.monsters)?;
        if self.zones.is_empty() || self.monsters.is_empty() {
            return Err(invalid(
                "Combat Zone snapshot must contain zones and monsters",
            ));
        }
        if self.counts.zones != self.zones.len() || self.counts.monsters != self.monsters.len() {
            return Err(invalid(format!(
                "snapshot counts do not match maps: declared {}/{} but found {}/{}",
                self.counts.zones,
                self.counts.monsters,
                self.zones.len(),
                self.monsters.len()
            )));
        }

        let mut referenced = BTreeSet::new();
        for (key, zone) in &self.zones {
            if key != &zone.hrid {
                return Err(invalid(format!(
                    "Zone map key {key} does not match row hrid {}",
                    zone.hrid
                )));
            }
            validate_zone(zone)?;
            for spawn in &zone.random_spawn_info.spawns {
                referenced.insert(spawn.combat_monster_hrid.clone());
            }
            for spawn in &zone.boss_spawns {
                referenced.insert(spawn.combat_monster_hrid.clone());
            }
        }

        for (key, monster) in &self.monsters {
            if key != &monster.hrid {
                return Err(invalid(format!(
                    "Monster map key {key} does not match row hrid {}",
                    monster.hrid
                )));
            }
            validate_monster(monster)?;
        }
        for hrid in &referenced {
            if !self.monsters.contains_key(hrid) {
                return Err(invalid(format!(
                    "Combat Zone spawn references absent monster {hrid}"
                )));
            }
        }
        let unreferenced = self
            .monsters
            .keys()
            .filter(|hrid| !referenced.contains(*hrid))
            .cloned()
            .collect::<Vec<_>>();
        if !unreferenced.is_empty() {
            return Err(invalid(format!(
                "snapshot contains unreferenced monsters: {}",
                unreferenced.join(", ")
            )));
        }
        Ok(())
    }

    pub fn zone(&self, hrid: &str) -> Option<&CombatZoneDataV1> {
        self.zones.get(hrid)
    }

    pub fn monster(&self, hrid: &str) -> Option<&CombatMonsterDataV1> {
        self.monsters.get(hrid)
    }
}

fn validate_source(location: &str, source: &CombatDataSourceV1) -> Result<()> {
    if source.path.trim().is_empty() {
        return Err(invalid(format!("{location}.path must not be empty")));
    }
    if source.sha256.len() != 64
        || !source
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(format!(
            "{location}.sha256 must be 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

fn validate_zone(zone: &CombatZoneDataV1) -> Result<()> {
    if zone.hrid.trim().is_empty() {
        return Err(invalid("Zone hrid must not be empty"));
    }
    finite_non_negative("zone.baseTimeCost", zone.base_time_cost)?;
    if zone.max_party_size == 0 {
        return Err(invalid(format!(
            "{}.maxPartySize must be positive",
            zone.hrid
        )));
    }
    if zone.random_spawn_info.max_spawn_count == 0 {
        return Err(invalid(format!(
            "{}.randomSpawnInfo.maxSpawnCount must be positive",
            zone.hrid
        )));
    }
    finite_positive(
        &format!("{}.randomSpawnInfo.maxTotalStrength", zone.hrid),
        zone.random_spawn_info.max_total_strength,
    )?;
    if zone.random_spawn_info.spawns.is_empty() {
        return Err(invalid(format!(
            "{}.randomSpawnInfo.spawns must not be empty",
            zone.hrid
        )));
    }
    for (index, spawn) in zone.random_spawn_info.spawns.iter().enumerate() {
        if spawn.combat_monster_hrid.trim().is_empty() {
            return Err(invalid(format!(
                "{}.randomSpawnInfo.spawns[{index}].combatMonsterHrid is required",
                zone.hrid
            )));
        }
        finite_positive(
            &format!("{}.randomSpawnInfo.spawns[{index}].rate", zone.hrid),
            spawn.rate,
        )?;
        finite_positive(
            &format!("{}.randomSpawnInfo.spawns[{index}].strength", zone.hrid),
            spawn.strength,
        )?;
    }
    for (index, spawn) in zone.boss_spawns.iter().enumerate() {
        if spawn.combat_monster_hrid.trim().is_empty() {
            return Err(invalid(format!(
                "{}.bossSpawns[{index}].combatMonsterHrid is required",
                zone.hrid
            )));
        }
    }
    Ok(())
}

fn validate_monster(monster: &CombatMonsterDataV1) -> Result<()> {
    if monster.hrid.trim().is_empty() {
        return Err(invalid("Monster hrid must not be empty"));
    }
    for (name, value) in [
        ("staminaLevel", monster.levels.stamina_level),
        ("intelligenceLevel", monster.levels.intelligence_level),
        ("attackLevel", monster.levels.attack_level),
        ("meleeLevel", monster.levels.melee_level),
        ("defenseLevel", monster.levels.defense_level),
        ("rangedLevel", monster.levels.ranged_level),
        ("magicLevel", monster.levels.magic_level),
    ] {
        if !value.is_finite() {
            return Err(invalid(format!(
                "{}.levels.{name} must be finite",
                monster.hrid
            )));
        }
    }
    finite_non_negative(&format!("{}.experience", monster.hrid), monster.experience)?;
    if !monster.combat_stats.is_object() {
        return Err(invalid(format!(
            "{}.combatStats must be an object",
            monster.hrid
        )));
    }
    Ok(())
}

fn finite_non_negative(location: &str, value: f64) -> Result<()> {
    if !value.is_finite() || value < 0.0 {
        return Err(invalid(format!(
            "{location} must be finite and non-negative"
        )));
    }
    Ok(())
}

fn finite_positive(location: &str, value: f64) -> Result<()> {
    if !value.is_finite() || value <= 0.0 {
        return Err(invalid(format!("{location} must be finite and positive")));
    }
    Ok(())
}

#[cfg(test)]
fn stat(monster: &CombatMonsterDataV1, name: &str) -> f64 {
    monster
        .combat_stats
        .get(name)
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

fn invalid(message: impl Into<String>) -> SimError {
    SimError::InvalidGameData(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_snapshot_has_valid_cross_references() {
        let snapshot = CombatZoneDataSnapshotV1::embedded().unwrap();
        assert_eq!(snapshot.counts.zones, snapshot.zones.len());
        assert_eq!(snapshot.counts.monsters, snapshot.monsters.len());
        assert!(snapshot.zone(BASIC_FLY_ZONE_HRID).is_some());
        assert!(snapshot.monster(BASIC_FLY_MONSTER_HRID).is_some());
    }

    #[test]
    fn generic_fly_data_matches_the_m2_slice() {
        let snapshot = CombatZoneDataSnapshotV1::embedded().unwrap();
        let generic = snapshot.monster(BASIC_FLY_MONSTER_HRID).unwrap();
        let basic = BasicGameData::embedded().unwrap();

        assert_eq!(generic.levels.attack_level, basic.monster.attack_level);
        assert_eq!(
            generic.base_attack_interval,
            basic.monster.base_attack_interval
        );
        assert_eq!(
            (10.0 + generic.levels.attack_level) * (1.0 + stat(generic, "smashAccuracy")),
            basic.monster.smash_accuracy_rating
        );
        assert_eq!(
            (10.0 + generic.levels.melee_level) * (1.0 + stat(generic, "smashDamage")),
            basic.monster.smash_max_damage
        );
        assert_eq!(
            (10.0 + generic.levels.defense_level) * (1.0 + stat(generic, "smashEvasion")),
            basic.monster.smash_evasion_rating
        );
        assert_eq!(
            0.2 * generic.levels.defense_level + stat(generic, "armor"),
            basic.monster.total_armor
        );
        assert_eq!(
            stat(generic, "autoAttackDamage"),
            basic.monster.auto_attack_damage
        );
    }
}
