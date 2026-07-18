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
    pub attack_interval: SimTime,
    pub smash_accuracy_rating: f64,
    pub smash_max_damage: f64,
    pub smash_evasion_rating: f64,
    pub total_armor: f64,
    pub experience: f64,
    pub enrage_time: SimTime,
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
        if !self.monster.total_armor.is_finite() {
            return Err(SimError::InvalidGameData(
                "monster.totalArmor must be finite".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_slice_matches_the_reference_fly_record() {
        let data = BasicGameData::embedded().unwrap();
        let source: Value = serde_json::from_str(include_str!(
            "../../../src/combatsimulator/data/combatMonsterDetailMap.json"
        ))
        .unwrap();
        let fly = &source[BASIC_FLY_MONSTER_HRID];
        let details = &fly["combatDetails"];

        assert_eq!(data.monster.max_hitpoints, details["maxHitpoints"]);
        assert_eq!(data.monster.max_manapoints, details["maxManapoints"]);
        assert_eq!(data.monster.attack_interval.get(), details["attackInterval"]);
        assert_eq!(
            data.monster.smash_accuracy_rating,
            details["smashAccuracyRating"]
        );
        assert_eq!(data.monster.smash_max_damage, details["smashMaxDamage"]);
        assert_eq!(
            data.monster.smash_evasion_rating,
            details["smashEvasionRating"]
        );
        assert_eq!(data.monster.total_armor, details["totalArmor"]);
        assert_eq!(data.monster.experience, fly["experience"]);
        assert_eq!(data.monster.enrage_time.get(), fly["enrageTime"]);
    }
}
