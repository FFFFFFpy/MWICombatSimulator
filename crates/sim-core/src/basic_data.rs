use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Result, SimError, SimTime};

pub const BASIC_DATA_SCHEMA_VERSION: u32 = 1;
pub const BASIC_DATA_VERSION: &str = "repository-v1.0.28";
pub const BASIC_FLY_ZONE_HRID: &str = "/actions/combat/fly";
pub const BASIC_FLY_MONSTER_HRID: &str = "/monsters/fly";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BasicGameData {
    pub schema_version: u32,
    pub data_version: String,
    pub sources: Value,
    pub zone: BasicZoneData,
    pub monster: BasicMonsterData,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BasicZoneData {
    pub hrid: String,
    pub monster_hrid: String,
    pub difficulty_tier: u32,
    pub spawn_count: u32,
    pub respawn_interval: SimTime,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BasicMonsterData {
    pub hrid: String,
    pub max_hitpoints: f64,
    pub max_manapoints: f64,
    pub attack_level: f64,
    pub base_attack_interval: SimTime,
    pub smash_accuracy_rating: f64,
    pub smash_max_damage: f64,
    pub smash_evasion_rating: f64,
    pub total_armor: f64,
    pub auto_attack_damage: f64,
    pub experience: f64,
    pub enrage_time: SimTime,
}

impl BasicMonsterData {
    pub fn attack_interval(&self) -> Result<SimTime> {
        let denominator = 1.0 + self.attack_level / 2_000.0;
        if !denominator.is_finite() || denominator <= 0.0 {
            return Err(SimError::InvalidGameData(
                "monster.attackLevel produces an invalid attack interval".into(),
            ));
        }
        SimTime::new(self.base_attack_interval.get() / denominator)
    }
}

impl BasicGameData {
    pub fn embedded() -> Result<Self> {
        let data: Self = serde_json::from_str(include_str!("../data/basic-combat-v1.json"))?;
        data.validate()?;
        Ok(data)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != BASIC_DATA_SCHEMA_VERSION {
            return Err(SimError::InvalidGameData(format!(
                "unsupported schema version {}; expected {}",
                self.schema_version, BASIC_DATA_SCHEMA_VERSION
            )));
        }
        if self.data_version != BASIC_DATA_VERSION {
            return Err(SimError::InvalidGameData(format!(
                "unexpected data version {}; expected {BASIC_DATA_VERSION}",
                self.data_version
            )));
        }
        if self.zone.hrid != BASIC_FLY_ZONE_HRID
            || self.zone.monster_hrid != BASIC_FLY_MONSTER_HRID
            || self.zone.difficulty_tier != 0
            || self.zone.spawn_count != 1
        {
            return Err(SimError::InvalidGameData(
                "the M2 data slice must describe exactly one tier-0 Fly spawn".into(),
            ));
        }
        if self.monster.hrid != BASIC_FLY_MONSTER_HRID {
            return Err(SimError::InvalidGameData(
                "the embedded monster does not match the Fly zone".into(),
            ));
        }
        for (name, value) in [
            ("monster.maxHitpoints", self.monster.max_hitpoints),
            ("monster.maxManapoints", self.monster.max_manapoints),
            (
                "monster.smashAccuracyRating",
                self.monster.smash_accuracy_rating,
            ),
            ("monster.smashMaxDamage", self.monster.smash_max_damage),
            (
                "monster.smashEvasionRating",
                self.monster.smash_evasion_rating,
            ),
            ("monster.experience", self.monster.experience),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(SimError::InvalidGameData(format!(
                    "{name} must be finite and positive"
                )));
            }
        }
        for (name, value) in [
            ("monster.attackLevel", self.monster.attack_level),
            ("monster.totalArmor", self.monster.total_armor),
            ("monster.autoAttackDamage", self.monster.auto_attack_damage),
        ] {
            if !value.is_finite() {
                return Err(SimError::InvalidGameData(format!(
                    "{name} must be finite"
                )));
            }
        }
        if self.monster.base_attack_interval == SimTime::ZERO {
            return Err(SimError::InvalidGameData(
                "monster.baseAttackInterval must be positive".into(),
            ));
        }
        self.monster.attack_interval()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(value: &Value) -> f64 {
        value.as_f64().unwrap()
    }

    fn optional_number(value: Option<&Value>) -> f64 {
        value.and_then(Value::as_f64).unwrap_or(0.0)
    }

    #[test]
    fn embedded_slice_matches_the_reference_fly_runtime_calculations() {
        let data = BasicGameData::embedded().unwrap();
        let source: Value = serde_json::from_str(include_str!(
            "../../../src/combatsimulator/data/combatMonsterDetailMap.json"
        ))
        .unwrap();
        let fly = &source[BASIC_FLY_MONSTER_HRID];
        let details = &fly["combatDetails"];
        let stats = &details["combatStats"];
        let attack_level = number(&details["attackLevel"]);
        let melee_level = number(&details["meleeLevel"]);
        let defense_level = number(&details["defenseLevel"]);

        assert_eq!(data.monster.max_hitpoints, number(&details["maxHitpoints"]));
        assert_eq!(
            data.monster.max_manapoints,
            number(&details["maxManapoints"])
        );
        assert_eq!(data.monster.attack_level, attack_level);
        assert_eq!(
            data.monster.base_attack_interval.get(),
            number(&stats["attackInterval"])
        );
        assert_eq!(
            data.monster.attack_interval().unwrap().get(),
            number(&stats["attackInterval"]) / (1.0 + attack_level / 2_000.0)
        );
        assert_eq!(
            data.monster.smash_accuracy_rating,
            (10.0 + attack_level) * (1.0 + number(&stats["smashAccuracy"]))
        );
        assert_eq!(
            data.monster.smash_max_damage,
            (10.0 + melee_level) * (1.0 + number(&stats["smashDamage"]))
        );
        assert_eq!(data.monster.smash_evasion_rating, 10.0 + defense_level);
        assert_eq!(data.monster.total_armor, 0.2 * defense_level);
        assert_eq!(
            data.monster.auto_attack_damage,
            optional_number(stats.get("autoAttackDamage"))
        );
        assert_eq!(data.monster.experience, number(&fly["experience"]));
        assert_eq!(data.monster.enrage_time.get(), number(&fly["enrageTime"]));
    }
}
