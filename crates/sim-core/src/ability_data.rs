use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Result, SimError, SimTime};

pub const ABILITY_DATA_SCHEMA_VERSION: u32 = 1;
pub const ABILITY_DATA_VERSION: &str = "repository-v1.0.28";
pub const DIRECT_DAMAGE_REPORT_VERSION: u32 = 1;
pub const AQUA_ARROW_HRID: &str = "/abilities/aqua_arrow";

const DAMAGE_EFFECT_HRID: &str = "/ability_effect_types/damage";
const ENEMY_TARGET: &str = "enemy";
const SUPPORTED_COMBAT_STYLES: &[&str] = &[
    "/combat_styles/stab",
    "/combat_styles/slash",
    "/combat_styles/smash",
    "/combat_styles/ranged",
    "/combat_styles/magic",
];
const SUPPORTED_DAMAGE_TYPES: &[&str] = &[
    "/damage_types/physical",
    "/damage_types/water",
    "/damage_types/nature",
    "/damage_types/fire",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbilityDataSourceV1 {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbilityEffectDataV1 {
    pub target_type: String,
    pub effect_type: String,
    pub combat_style_hrid: String,
    pub damage_type: String,
    pub base_damage_flat: f64,
    pub base_damage_flat_level_bonus: f64,
    pub base_damage_ratio: f64,
    pub base_damage_ratio_level_bonus: f64,
    pub bonus_accuracy_ratio: f64,
    pub bonus_accuracy_ratio_level_bonus: f64,
    pub damage_over_time_ratio: f64,
    pub damage_over_time_duration: SimTime,
    pub armor_damage_ratio: f64,
    pub armor_damage_ratio_level_bonus: f64,
    pub hp_drain_ratio: f64,
    pub pierce_chance: f64,
    pub blind_chance: f64,
    pub blind_duration: SimTime,
    pub silence_chance: f64,
    pub silence_duration: SimTime,
    pub stun_chance: f64,
    pub stun_duration: SimTime,
    pub spend_hp_ratio: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffs: Option<Vec<Value>>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbilityDataV1 {
    pub hrid: String,
    pub name: String,
    pub description: String,
    pub is_special_ability: bool,
    pub mana_cost: f64,
    pub cooldown_duration: SimTime,
    pub cast_duration: SimTime,
    pub ability_effects: Vec<AbilityEffectDataV1>,
    pub default_combat_triggers: Vec<Value>,
    pub sort_index: f64,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbilityDataSnapshotV1 {
    pub schema_version: u32,
    pub data_version: String,
    pub source: AbilityDataSourceV1,
    pub count: usize,
    pub abilities: BTreeMap<String, AbilityDataV1>,
}

impl AbilityDataSnapshotV1 {
    pub fn embedded() -> Result<Self> {
        let snapshot: Self = serde_json::from_str(include_str!("../data/ability-data-v1.json"))?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != ABILITY_DATA_SCHEMA_VERSION {
            return Err(invalid(format!(
                "unsupported ability schema {}; expected {}",
                self.schema_version, ABILITY_DATA_SCHEMA_VERSION
            )));
        }
        if self.data_version != ABILITY_DATA_VERSION {
            return Err(invalid(format!(
                "unexpected ability data version {}; expected {ABILITY_DATA_VERSION}",
                self.data_version
            )));
        }
        validate_source(&self.source)?;
        if self.abilities.is_empty() || self.count != self.abilities.len() {
            return Err(invalid(format!(
                "ability count mismatch: declared {} but found {}",
                self.count,
                self.abilities.len()
            )));
        }
        for (key, ability) in &self.abilities {
            if key != &ability.hrid {
                return Err(invalid(format!(
                    "ability map key {key} does not match row hrid {}",
                    ability.hrid
                )));
            }
            validate_ability(ability)?;
        }
        Ok(())
    }

    pub fn ability(&self, hrid: &str) -> Option<&AbilityDataV1> {
        self.abilities.get(hrid)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectDamageAbilityIssueV1 {
    pub code: String,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectDamageAbilityCandidateV1 {
    pub ability_hrid: String,
    pub combat_style_hrid: String,
    pub damage_type: String,
    pub default_trigger_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectDamageAbilityRejectionV1 {
    pub ability_hrid: String,
    pub issues: Vec<DirectDamageAbilityIssueV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectDamageAbilityCountsV1 {
    pub total_abilities: usize,
    pub candidates: usize,
    pub rejected: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectDamageAbilityReportV1 {
    pub report_version: u32,
    pub data_version: String,
    pub counts: DirectDamageAbilityCountsV1,
    pub reason_counts: BTreeMap<String, usize>,
    pub candidates: BTreeMap<String, DirectDamageAbilityCandidateV1>,
    pub rejected: BTreeMap<String, DirectDamageAbilityRejectionV1>,
}

pub fn classify_direct_damage_abilities(
    snapshot: &AbilityDataSnapshotV1,
) -> Result<DirectDamageAbilityReportV1> {
    snapshot.validate()?;
    let mut candidates = BTreeMap::new();
    let mut rejected = BTreeMap::new();
    let mut reason_counts = BTreeMap::new();

    for (hrid, ability) in &snapshot.abilities {
        let issues = direct_damage_issues(ability);
        if issues.is_empty() {
            let effect = &ability.ability_effects[0];
            candidates.insert(
                hrid.clone(),
                DirectDamageAbilityCandidateV1 {
                    ability_hrid: hrid.clone(),
                    combat_style_hrid: effect.combat_style_hrid.clone(),
                    damage_type: effect.damage_type.clone(),
                    default_trigger_count: ability.default_combat_triggers.len(),
                },
            );
        } else {
            for issue in &issues {
                *reason_counts.entry(issue.code.clone()).or_default() += 1;
            }
            rejected.insert(
                hrid.clone(),
                DirectDamageAbilityRejectionV1 {
                    ability_hrid: hrid.clone(),
                    issues,
                },
            );
        }
    }

    Ok(DirectDamageAbilityReportV1 {
        report_version: DIRECT_DAMAGE_REPORT_VERSION,
        data_version: snapshot.data_version.clone(),
        counts: DirectDamageAbilityCountsV1 {
            total_abilities: snapshot.abilities.len(),
            candidates: candidates.len(),
            rejected: rejected.len(),
        },
        reason_counts,
        candidates,
        rejected,
    })
}

fn validate_source(source: &AbilityDataSourceV1) -> Result<()> {
    if source.path.trim().is_empty() {
        return Err(invalid("ability source path must not be empty"));
    }
    if source.sha256.len() != 64
        || !source
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(
            "ability source sha256 must be 64 lowercase hexadecimal characters",
        ));
    }
    Ok(())
}

fn validate_ability(ability: &AbilityDataV1) -> Result<()> {
    if ability.hrid.trim().is_empty() {
        return Err(invalid("ability hrid must not be empty"));
    }
    finite_non_negative(&format!("{}.manaCost", ability.hrid), ability.mana_cost)?;
    if !ability.sort_index.is_finite() {
        return Err(invalid(format!(
            "{}.sortIndex must be finite",
            ability.hrid
        )));
    }
    if ability.ability_effects.is_empty() {
        return Err(invalid(format!(
            "{}.abilityEffects must not be empty",
            ability.hrid
        )));
    }
    for (index, effect) in ability.ability_effects.iter().enumerate() {
        validate_effect(&ability.hrid, index, effect)?;
    }
    for (index, trigger) in ability.default_combat_triggers.iter().enumerate() {
        if !trigger.is_object() {
            return Err(invalid(format!(
                "{}.defaultCombatTriggers[{index}] must be an object",
                ability.hrid
            )));
        }
    }
    Ok(())
}

fn validate_effect(hrid: &str, index: usize, effect: &AbilityEffectDataV1) -> Result<()> {
    let location = format!("{hrid}.abilityEffects[{index}]");
    if effect.target_type.trim().is_empty() || effect.effect_type.trim().is_empty() {
        return Err(invalid(format!(
            "{location} targetType and effectType are required"
        )));
    }
    for (name, value) in [
        ("baseDamageFlat", effect.base_damage_flat),
        (
            "baseDamageFlatLevelBonus",
            effect.base_damage_flat_level_bonus,
        ),
        ("baseDamageRatio", effect.base_damage_ratio),
        (
            "baseDamageRatioLevelBonus",
            effect.base_damage_ratio_level_bonus,
        ),
        ("bonusAccuracyRatio", effect.bonus_accuracy_ratio),
        (
            "bonusAccuracyRatioLevelBonus",
            effect.bonus_accuracy_ratio_level_bonus,
        ),
        ("damageOverTimeRatio", effect.damage_over_time_ratio),
        ("armorDamageRatio", effect.armor_damage_ratio),
        (
            "armorDamageRatioLevelBonus",
            effect.armor_damage_ratio_level_bonus,
        ),
        ("hpDrainRatio", effect.hp_drain_ratio),
        ("spendHpRatio", effect.spend_hp_ratio),
    ] {
        if !value.is_finite() {
            return Err(invalid(format!("{location}.{name} must be finite")));
        }
    }
    for (name, value) in [
        ("pierceChance", effect.pierce_chance),
        ("blindChance", effect.blind_chance),
        ("silenceChance", effect.silence_chance),
        ("stunChance", effect.stun_chance),
    ] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(invalid(format!(
                "{location}.{name} must be finite and in [0, 1]"
            )));
        }
    }
    if let Some(buffs) = &effect.buffs {
        if buffs.iter().any(|buff| !buff.is_object()) {
            return Err(invalid(format!("{location}.buffs must contain objects")));
        }
    }
    Ok(())
}

fn direct_damage_issues(ability: &AbilityDataV1) -> Vec<DirectDamageAbilityIssueV1> {
    let mut issues = Vec::new();
    if ability.ability_effects.len() != 1 {
        push_issue(
            &mut issues,
            "effect_count",
            format!(
                "Ability has {} effects; exactly one is required",
                ability.ability_effects.len()
            ),
        );
        return issues;
    }

    let effect = &ability.ability_effects[0];
    if effect.effect_type != DAMAGE_EFFECT_HRID {
        push_issue(
            &mut issues,
            "effect_type",
            format!("effectType is {}", effect.effect_type),
        );
    }
    if effect.target_type != ENEMY_TARGET {
        push_issue(
            &mut issues,
            "target_type",
            format!("targetType is {}", effect.target_type),
        );
    }
    if !SUPPORTED_COMBAT_STYLES.contains(&effect.combat_style_hrid.as_str()) {
        push_issue(
            &mut issues,
            "combat_style",
            format!("unsupported combat style {}", effect.combat_style_hrid),
        );
    }
    if !SUPPORTED_DAMAGE_TYPES.contains(&effect.damage_type.as_str()) {
        push_issue(
            &mut issues,
            "damage_type",
            format!("unsupported damage type {}", effect.damage_type),
        );
    }
    if effect.buffs.as_ref().is_some_and(|buffs| !buffs.is_empty()) {
        push_issue(&mut issues, "buffs", "effect applies Buffs".into());
    }
    reject_non_zero(
        &mut issues,
        "damage_over_time",
        effect.damage_over_time_ratio,
        "damageOverTimeRatio",
    );
    if effect.damage_over_time_duration != SimTime::ZERO {
        push_issue(
            &mut issues,
            "damage_over_time",
            "damageOverTimeDuration is non-zero".into(),
        );
    }
    reject_non_zero(
        &mut issues,
        "armor_damage",
        effect.armor_damage_ratio,
        "armorDamageRatio",
    );
    reject_non_zero(
        &mut issues,
        "armor_damage",
        effect.armor_damage_ratio_level_bonus,
        "armorDamageRatioLevelBonus",
    );
    reject_non_zero(
        &mut issues,
        "hp_drain",
        effect.hp_drain_ratio,
        "hpDrainRatio",
    );
    reject_non_zero(&mut issues, "pierce", effect.pierce_chance, "pierceChance");
    reject_non_zero(&mut issues, "blind", effect.blind_chance, "blindChance");
    if effect.blind_duration != SimTime::ZERO {
        push_issue(&mut issues, "blind", "blindDuration is non-zero".into());
    }
    reject_non_zero(
        &mut issues,
        "silence",
        effect.silence_chance,
        "silenceChance",
    );
    if effect.silence_duration != SimTime::ZERO {
        push_issue(&mut issues, "silence", "silenceDuration is non-zero".into());
    }
    reject_non_zero(&mut issues, "stun", effect.stun_chance, "stunChance");
    if effect.stun_duration != SimTime::ZERO {
        push_issue(&mut issues, "stun", "stunDuration is non-zero".into());
    }
    reject_non_zero(
        &mut issues,
        "spend_hp",
        effect.spend_hp_ratio,
        "spendHpRatio",
    );

    for (name, value) in &effect.extensions {
        if !zero_like(value) {
            push_issue(
                &mut issues,
                "unknown_effect_extension",
                format!("unknown non-zero effect field {name}"),
            );
        }
    }

    issues.sort_by(|left, right| (&left.code, &left.detail).cmp(&(&right.code, &right.detail)));
    issues.dedup();
    issues
}

fn reject_non_zero(
    issues: &mut Vec<DirectDamageAbilityIssueV1>,
    code: &str,
    value: f64,
    field: &str,
) {
    if value != 0.0 {
        push_issue(issues, code, format!("{field} is non-zero ({value})"));
    }
}

fn zero_like(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Bool(value) => !value,
        Value::Number(value) => value.as_f64().is_some_and(|number| number == 0.0),
        Value::String(value) => value.is_empty(),
        Value::Array(values) => values.is_empty(),
        Value::Object(values) => values.is_empty(),
    }
}

fn finite_non_negative(location: &str, value: f64) -> Result<()> {
    if !value.is_finite() || value < 0.0 {
        return Err(invalid(format!(
            "{location} must be finite and non-negative"
        )));
    }
    Ok(())
}

fn push_issue(issues: &mut Vec<DirectDamageAbilityIssueV1>, code: &str, detail: String) {
    issues.push(DirectDamageAbilityIssueV1 {
        code: code.into(),
        detail,
    });
}

fn invalid(message: impl Into<String>) -> SimError {
    SimError::InvalidGameData(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_snapshot_is_valid_and_aqua_arrow_is_a_candidate() {
        let snapshot = AbilityDataSnapshotV1::embedded().unwrap();
        let report = classify_direct_damage_abilities(&snapshot).unwrap();

        assert_eq!(snapshot.count, snapshot.abilities.len());
        assert_eq!(report.counts.total_abilities, snapshot.abilities.len());
        assert_eq!(
            report.counts.candidates + report.counts.rejected,
            report.counts.total_abilities
        );
        assert!(report.candidates.contains_key(AQUA_ARROW_HRID));
        assert!(
            report
                .rejected
                .values()
                .all(|entry| !entry.issues.is_empty())
        );
    }

    #[test]
    fn buff_and_control_effects_are_structured_rejections() {
        let snapshot = AbilityDataSnapshotV1::embedded().unwrap();
        let mut ability = snapshot.ability(AQUA_ARROW_HRID).unwrap().clone();
        let effect = &mut ability.ability_effects[0];
        effect.buffs = Some(vec![serde_json::json!({"typeHrid":"/buff_types/test"})]);
        effect.stun_chance = 0.5;
        effect.stun_duration = SimTime::new(1_000_000_000.0).unwrap();

        let issues = direct_damage_issues(&ability);
        assert!(issues.iter().any(|issue| issue.code == "buffs"));
        assert!(issues.iter().any(|issue| issue.code == "stun"));
    }

    #[test]
    fn unknown_zero_extensions_do_not_reject_but_non_zero_extensions_do() {
        let snapshot = AbilityDataSnapshotV1::embedded().unwrap();
        let mut ability = snapshot.ability(AQUA_ARROW_HRID).unwrap().clone();
        let effect = &mut ability.ability_effects[0];
        effect
            .extensions
            .insert("futureZero".into(), Value::from(0));
        assert!(direct_damage_issues(&ability).is_empty());

        effect
            .extensions
            .insert("futureFeature".into(), Value::from(1));
        assert!(
            direct_damage_issues(&ability)
                .iter()
                .any(|issue| issue.code == "unknown_effect_extension")
        );
    }
}
