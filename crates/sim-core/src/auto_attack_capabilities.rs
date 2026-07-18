use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    BASIC_FLY_ZONE_HRID, CombatMonsterDataV1, CombatZoneDataSnapshotV1, CombatZoneDataV1, Result,
};

pub const AUTO_ATTACK_CAPABILITY_REPORT_VERSION: u32 = 1;

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

const UNSUPPORTED_PASSIVE_STATS: &[&str] = &[
    "physicalThorns",
    "elementalThorns",
    "lifeSteal",
    "manaLeech",
    "parry",
    "mayhem",
    "pierce",
    "curse",
    "fury",
    "weaken",
    "ripple",
    "bloom",
    "blaze",
    "retaliation",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoAttackCapabilityIssueV1 {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoAttackZoneCandidateV1 {
    pub zone_hrid: String,
    pub monster_hrids: Vec<String>,
    pub has_boss_spawn: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoAttackZoneRejectionV1 {
    pub zone_hrid: String,
    pub issues: Vec<AutoAttackCapabilityIssueV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoAttackCapabilityCountsV1 {
    pub total_zones: usize,
    pub candidates: usize,
    pub rejected: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoAttackCapabilityReportV1 {
    pub report_version: u32,
    pub data_version: String,
    pub counts: AutoAttackCapabilityCountsV1,
    pub reason_counts: BTreeMap<String, usize>,
    pub candidates: BTreeMap<String, AutoAttackZoneCandidateV1>,
    pub rejected: BTreeMap<String, AutoAttackZoneRejectionV1>,
}

pub fn classify_auto_attack_zones(
    snapshot: &CombatZoneDataSnapshotV1,
) -> Result<AutoAttackCapabilityReportV1> {
    snapshot.validate()?;

    let mut candidates = BTreeMap::new();
    let mut rejected = BTreeMap::new();
    let mut reason_counts = BTreeMap::new();

    for (zone_hrid, zone) in &snapshot.zones {
        let monster_hrids = referenced_monster_hrids(zone);
        let issues = classify_zone(snapshot, zone, &monster_hrids);
        if issues.is_empty() {
            candidates.insert(
                zone_hrid.clone(),
                AutoAttackZoneCandidateV1 {
                    zone_hrid: zone_hrid.clone(),
                    monster_hrids: monster_hrids.into_iter().collect(),
                    has_boss_spawn: !zone.boss_spawns.is_empty(),
                },
            );
        } else {
            for issue in &issues {
                *reason_counts.entry(issue.code.clone()).or_default() += 1;
            }
            rejected.insert(
                zone_hrid.clone(),
                AutoAttackZoneRejectionV1 {
                    zone_hrid: zone_hrid.clone(),
                    issues,
                },
            );
        }
    }

    Ok(AutoAttackCapabilityReportV1 {
        report_version: AUTO_ATTACK_CAPABILITY_REPORT_VERSION,
        data_version: snapshot.data_version.clone(),
        counts: AutoAttackCapabilityCountsV1 {
            total_zones: snapshot.zones.len(),
            candidates: candidates.len(),
            rejected: rejected.len(),
        },
        reason_counts,
        candidates,
        rejected,
    })
}

fn classify_zone(
    snapshot: &CombatZoneDataSnapshotV1,
    zone: &CombatZoneDataV1,
    monster_hrids: &BTreeSet<String>,
) -> Vec<AutoAttackCapabilityIssueV1> {
    let mut issues = Vec::new();

    if !zone.buffs.is_empty() {
        push_issue(
            &mut issues,
            "zone_buffs",
            None,
            format!("Zone has {} active Buff definitions", zone.buffs.len()),
        );
    }
    if zone.random_spawn_info.max_spawn_count != 1 {
        push_issue(
            &mut issues,
            "multi_enemy_spawn",
            None,
            format!(
                "maxSpawnCount is {}; the current engine supports exactly one enemy",
                zone.random_spawn_info.max_spawn_count
            ),
        );
    }
    if zone.boss_spawns.len() > 1 {
        push_issue(
            &mut issues,
            "multi_boss_spawn",
            None,
            format!(
                "Zone has {} Boss spawns; the current engine supports at most one",
                zone.boss_spawns.len()
            ),
        );
    }

    for (index, spawn) in zone.random_spawn_info.spawns.iter().enumerate() {
        if spawn.difficulty_tier != 0 {
            push_issue(
                &mut issues,
                "spawn_difficulty",
                Some(spawn.combat_monster_hrid.clone()),
                format!(
                    "random spawn {index} has difficultyTier {}",
                    spawn.difficulty_tier
                ),
            );
        }
        if spawn.strength > zone.random_spawn_info.max_total_strength {
            push_issue(
                &mut issues,
                "spawn_strength_exceeds_cap",
                Some(spawn.combat_monster_hrid.clone()),
                format!(
                    "spawn strength {} exceeds maxTotalStrength {}",
                    spawn.strength, zone.random_spawn_info.max_total_strength
                ),
            );
        }
    }
    for (index, spawn) in zone.boss_spawns.iter().enumerate() {
        if spawn.difficulty_tier != 0 {
            push_issue(
                &mut issues,
                "boss_difficulty",
                Some(spawn.combat_monster_hrid.clone()),
                format!(
                    "Boss spawn {index} has difficultyTier {}",
                    spawn.difficulty_tier
                ),
            );
        }
    }

    for monster_hrid in monster_hrids {
        let Some(monster) = snapshot.monsters.get(monster_hrid) else {
            push_issue(
                &mut issues,
                "missing_monster",
                Some(monster_hrid.clone()),
                "Referenced monster is absent from the snapshot".into(),
            );
            continue;
        };
        classify_monster(monster, &mut issues);
    }

    issues.sort_by(|left, right| {
        (&left.code, &left.subject, &left.detail).cmp(&(&right.code, &right.subject, &right.detail))
    });
    issues.dedup();
    issues
}

fn classify_monster(monster: &CombatMonsterDataV1, issues: &mut Vec<AutoAttackCapabilityIssueV1>) {
    let tier_zero_abilities = monster
        .abilities
        .iter()
        .filter(|ability| {
            ability
                .get("minDifficultyTier")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                == 0
        })
        .count();
    if tier_zero_abilities > 0 {
        push_issue(
            issues,
            "monster_active_abilities",
            Some(monster.hrid.clone()),
            format!("Monster has {tier_zero_abilities} tier-0 active abilities"),
        );
    }

    let styles = monster
        .combat_stats
        .get("combatStyleHrids")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    if styles.len() != 1 {
        push_issue(
            issues,
            "combat_style_count",
            Some(monster.hrid.clone()),
            format!(
                "Monster has {} combat styles; exactly one is required",
                styles.len()
            ),
        );
    } else if !SUPPORTED_COMBAT_STYLES.contains(&styles[0]) {
        push_issue(
            issues,
            "unsupported_combat_style",
            Some(monster.hrid.clone()),
            format!("Unsupported combat style {}", styles[0]),
        );
    }

    let damage_type = monster
        .combat_stats
        .get("damageType")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !SUPPORTED_DAMAGE_TYPES.contains(&damage_type) {
        push_issue(
            issues,
            "unsupported_damage_type",
            Some(monster.hrid.clone()),
            format!("Unsupported damage type {damage_type}"),
        );
    }

    let attack_interval = stat(monster, "attackInterval");
    if attack_interval <= 0.0 && monster.base_attack_interval == crate::SimTime::ZERO {
        push_issue(
            issues,
            "invalid_attack_interval",
            Some(monster.hrid.clone()),
            "Neither combatStats.attackInterval nor baseAttackInterval is positive".into(),
        );
    }

    for stat_name in UNSUPPORTED_PASSIVE_STATS {
        let value = stat(monster, stat_name);
        if value != 0.0 {
            push_issue(
                issues,
                "unsupported_passive",
                Some(monster.hrid.clone()),
                format!("{stat_name} is non-zero ({value})"),
            );
        }
    }
}

fn referenced_monster_hrids(zone: &CombatZoneDataV1) -> BTreeSet<String> {
    zone.random_spawn_info
        .spawns
        .iter()
        .map(|spawn| spawn.combat_monster_hrid.clone())
        .chain(
            zone.boss_spawns
                .iter()
                .map(|spawn| spawn.combat_monster_hrid.clone()),
        )
        .collect()
}

fn stat(monster: &CombatMonsterDataV1, name: &str) -> f64 {
    monster
        .combat_stats
        .get(name)
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

fn push_issue(
    issues: &mut Vec<AutoAttackCapabilityIssueV1>,
    code: &str,
    subject: Option<String>,
    detail: String,
) {
    issues.push(AutoAttackCapabilityIssueV1 {
        code: code.into(),
        subject,
        detail,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_snapshot_classification_is_complete_and_keeps_fly_as_a_candidate() {
        let snapshot = CombatZoneDataSnapshotV1::embedded().unwrap();
        let report = classify_auto_attack_zones(&snapshot).unwrap();

        assert_eq!(report.counts.total_zones, snapshot.zones.len());
        assert_eq!(
            report.counts.candidates + report.counts.rejected,
            report.counts.total_zones
        );
        assert!(report.candidates.contains_key(BASIC_FLY_ZONE_HRID));
        assert!(report.rejected.values().all(|zone| !zone.issues.is_empty()));
    }

    #[test]
    fn zone_buffs_and_tier_zero_abilities_are_structured_rejection_reasons() {
        let mut snapshot = CombatZoneDataSnapshotV1::embedded().unwrap();
        snapshot
            .zones
            .get_mut(BASIC_FLY_ZONE_HRID)
            .unwrap()
            .buffs
            .push(serde_json::json!({"typeHrid":"/buff_types/test"}));
        snapshot
            .monsters
            .get_mut("/monsters/fly")
            .unwrap()
            .abilities
            .push(serde_json::json!({
                "abilityHrid": "/abilities/test",
                "level": 1,
                "minDifficultyTier": 0
            }));

        let report = classify_auto_attack_zones(&snapshot).unwrap();
        let rejection = report.rejected.get(BASIC_FLY_ZONE_HRID).unwrap();
        assert!(
            rejection
                .issues
                .iter()
                .any(|issue| issue.code == "zone_buffs")
        );
        assert!(
            rejection
                .issues
                .iter()
                .any(|issue| issue.code == "monster_active_abilities")
        );
    }

    #[test]
    fn higher_tier_monster_abilities_do_not_reject_a_tier_zero_candidate() {
        let mut snapshot = CombatZoneDataSnapshotV1::embedded().unwrap();
        snapshot
            .monsters
            .get_mut("/monsters/fly")
            .unwrap()
            .abilities
            .push(serde_json::json!({
                "abilityHrid": "/abilities/future",
                "level": 1,
                "minDifficultyTier": 1
            }));

        let report = classify_auto_attack_zones(&snapshot).unwrap();
        assert!(report.candidates.contains_key(BASIC_FLY_ZONE_HRID));
    }
}
