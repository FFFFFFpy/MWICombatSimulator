use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    EventHandle, RandomConfigV1, RandomSource, Result, SeededRandom, SequenceRandom, SimError,
    SimTime, SimulationRequestV1, SimulationTargetV1, StableEventQueue, StatisticsMode,
    ability_data::{
        AQUA_ARROW_HRID, AbilityDataSnapshotV1, AbilityEffectDataV1,
        classify_direct_damage_abilities,
    },
    basic_combat::AttackHistogram,
    basic_data::{BASIC_DATA_VERSION, BASIC_FLY_MONSTER_HRID, BASIC_FLY_ZONE_HRID, BasicGameData},
    combat_data::{CombatMonsterDataV1, CombatZoneDataSnapshotV1},
};

pub const DIRECT_DAMAGE_ENGINE_ID: &str = "rust-direct-damage";
pub const DIRECT_DAMAGE_COMPATIBILITY_LEVEL: &str = "direct-damage-cast-v1";

const ONE_SECOND: f64 = 1_000_000_000.0;
const REGEN_TICK_INTERVAL: f64 = 10.0 * ONE_SECOND;
const PLAYER_RESPAWN_INTERVAL: f64 = 150.0 * ONE_SECOND;
const INITIAL_LAST_USED: f64 = -9_007_199_254_740_991.0;

pub type ManaUsage = BTreeMap<String, BTreeMap<String, u64>>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectDamageCombatResultV1 {
    pub contract_version: u32,
    #[serde(rename = "type")]
    pub result_type: String,
    pub request_id: String,
    pub engine: String,
    pub engine_version: String,
    pub data_version: String,
    pub compatibility_level: String,
    pub zone_name: String,
    pub simulated_time: SimTime,
    pub encounters: u64,
    pub deaths: BTreeMap<String, u64>,
    pub attacks: AttackHistogram,
    pub mana_used: ManaUsage,
    pub last_encounter_finish_time: SimTime,
    pub random_draws: u64,
    pub events_processed: u64,
}

impl DirectDamageCombatResultV1 {
    fn new(request: &SimulationRequestV1) -> Self {
        Self {
            contract_version: request.contract_version,
            result_type: "direct_damage_combat_result".into(),
            request_id: request.request_id.clone(),
            engine: DIRECT_DAMAGE_ENGINE_ID.into(),
            engine_version: env!("CARGO_PKG_VERSION").into(),
            data_version: BASIC_DATA_VERSION.into(),
            compatibility_level: DIRECT_DAMAGE_COMPATIBILITY_LEVEL.into(),
            zone_name: BASIC_FLY_ZONE_HRID.into(),
            simulated_time: SimTime::ZERO,
            encounters: 0,
            deaths: BTreeMap::new(),
            attacks: BTreeMap::new(),
            mana_used: BTreeMap::new(),
            last_encounter_finish_time: SimTime::ZERO,
            random_draws: 0,
            events_processed: 0,
        }
    }

    fn record_attack(
        &mut self,
        source: &str,
        target: &str,
        attack_type: &str,
        outcome: AttackOutcome,
    ) {
        let hit = if outcome.did_hit {
            outcome.damage_done.to_string()
        } else {
            "miss".into()
        };
        *self
            .attacks
            .entry(source.into())
            .or_default()
            .entry(target.into())
            .or_default()
            .entry(attack_type.into())
            .or_default()
            .entry(hit)
            .or_default() += 1;
    }

    fn record_death(&mut self, hrid: &str) {
        *self.deaths.entry(hrid.into()).or_default() += 1;
    }

    fn record_mana_used(&mut self, source: &str, ability: &str, amount: f64) -> Result<()> {
        if !amount.is_finite() || amount < 0.0 || amount.fract() != 0.0 {
            return Err(unsupported(format!(
                "mana cost for {ability} must be a non-negative integer"
            )));
        }
        *self
            .mana_used
            .entry(source.into())
            .or_default()
            .entry(ability.into())
            .or_default() += amount as u64;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DirectAbilityDto {
    hrid: String,
    level: u32,
    #[serde(default)]
    triggers: Vec<Value>,
    #[serde(flatten)]
    extensions: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DirectPlayerDto {
    hrid: String,
    stamina_level: f64,
    intelligence_level: f64,
    attack_level: f64,
    melee_level: f64,
    defense_level: f64,
    ranged_level: f64,
    magic_level: f64,
    #[serde(default)]
    equipment: Value,
    #[serde(default)]
    food: Vec<Value>,
    #[serde(default)]
    drinks: Vec<Value>,
    #[serde(default)]
    abilities: Vec<DirectAbilityDto>,
    #[serde(default)]
    house_rooms: Value,
    #[serde(default)]
    guild_buffs: Value,
    #[serde(default)]
    achievements: Value,
    #[serde(default)]
    debuff_on_level_gap: f64,
    #[serde(flatten)]
    extensions: BTreeMap<String, Value>,
}

impl DirectPlayerDto {
    fn parse(value: &Value) -> Result<Self> {
        let player: Self = serde_json::from_value(value.clone())?;
        player.validate()?;
        Ok(player)
    }

    fn validate(&self) -> Result<()> {
        if self.hrid.trim().is_empty() {
            return Err(unsupported("player.hrid must not be empty"));
        }
        for (name, value) in [
            ("staminaLevel", self.stamina_level),
            ("intelligenceLevel", self.intelligence_level),
            ("attackLevel", self.attack_level),
            ("meleeLevel", self.melee_level),
            ("defenseLevel", self.defense_level),
            ("rangedLevel", self.ranged_level),
            ("magicLevel", self.magic_level),
        ] {
            if !value.is_finite() {
                return Err(unsupported(format!("player.{name} must be finite")));
            }
        }
        if 10.0 + self.stamina_level <= 0.0 || 10.0 + self.intelligence_level <= 0.0 {
            return Err(unsupported(
                "player stamina/intelligence levels must produce positive HP and MP",
            ));
        }
        if !is_empty_json_collection(&self.equipment)
            || !self.food.is_empty()
            || !self.drinks.is_empty()
            || !is_empty_json_collection(&self.house_rooms)
            || !is_empty_json_collection(&self.guild_buffs)
            || !is_empty_json_collection(&self.achievements)
        {
            return Err(unsupported(
                "M3D does not support equipment, consumables, rooms, guild buffs, or achievements",
            ));
        }
        if self.abilities.len() != 1 {
            return Err(unsupported("M3D requires exactly one ability"));
        }
        let ability = &self.abilities[0];
        if ability.hrid != AQUA_ARROW_HRID || ability.level != 1 {
            return Err(unsupported(
                "M3D formally supports only level-1 /abilities/aqua_arrow",
            ));
        }
        if !ability.triggers.is_empty() || !ability.extensions.is_empty() {
            return Err(unsupported(
                "M3D Aqua Arrow requires an empty custom Trigger list and no unknown ability fields",
            ));
        }
        if self.debuff_on_level_gap != 0.0 {
            return Err(unsupported("debuffOnLevelGap must be zero in M3D"));
        }
        if !self.extensions.is_empty() {
            return Err(unsupported(format!(
                "unsupported player fields: {}",
                self.extensions
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        Ok(())
    }
}

fn is_empty_json_collection(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Array(values) => values.is_empty(),
        Value::Object(values) => values.is_empty(),
        _ => false,
    }
}

#[derive(Clone, Debug)]
struct StyleValues {
    stab: f64,
    slash: f64,
    smash: f64,
    ranged: f64,
    magic: f64,
}

impl StyleValues {
    fn get(&self, style: &str) -> Result<f64> {
        match style {
            "/combat_styles/stab" => Ok(self.stab),
            "/combat_styles/slash" => Ok(self.slash),
            "/combat_styles/smash" => Ok(self.smash),
            "/combat_styles/ranged" => Ok(self.ranged),
            "/combat_styles/magic" => Ok(self.magic),
            _ => Err(unsupported(format!("unsupported combat style {style}"))),
        }
    }
}

#[derive(Clone, Debug)]
struct DamageValues {
    physical: f64,
    water: f64,
    nature: f64,
    fire: f64,
}

impl DamageValues {
    fn get(&self, damage_type: &str) -> Result<f64> {
        match damage_type {
            "/damage_types/physical" => Ok(self.physical),
            "/damage_types/water" => Ok(self.water),
            "/damage_types/nature" => Ok(self.nature),
            "/damage_types/fire" => Ok(self.fire),
            _ => Err(unsupported(format!(
                "unsupported damage type {damage_type}"
            ))),
        }
    }
}

#[derive(Clone, Debug)]
struct DirectUnit {
    hrid: String,
    current_hitpoints: f64,
    max_hitpoints: f64,
    current_manapoints: f64,
    max_manapoints: f64,
    attack_interval: SimTime,
    cast_speed: f64,
    combat_style_hrid: String,
    damage_type: String,
    accuracy: StyleValues,
    max_damage: StyleValues,
    evasion: StyleValues,
    resistance: DamageValues,
    amplify: DamageValues,
    penetration: DamageValues,
    critical_rate: f64,
    critical_damage: f64,
    task_damage: f64,
    damage_taken: f64,
    auto_attack_damage: f64,
    ability_damage: f64,
    mayhem: f64,
    pierce: f64,
    hp_regen_per_10: f64,
    mp_regen_per_10: f64,
}

impl DirectUnit {
    fn player(dto: &DirectPlayerDto) -> Result<Self> {
        let max_hitpoints = (10.0 * (10.0 + dto.stamina_level)).floor();
        let max_manapoints = (10.0 * (10.0 + dto.intelligence_level)).floor();
        let attack_interval = SimTime::new(3_000_000_000.0 / (1.0 + dto.attack_level / 2_000.0))?;
        let accuracy = StyleValues {
            stab: 10.0 + dto.attack_level,
            slash: 10.0 + dto.attack_level,
            smash: 10.0 + dto.attack_level,
            ranged: 10.0 + dto.attack_level,
            magic: 10.0 + dto.attack_level,
        };
        let max_damage = StyleValues {
            stab: 10.0 + dto.melee_level,
            slash: 10.0 + dto.melee_level,
            smash: 10.0 + dto.melee_level,
            ranged: 10.0 + dto.ranged_level,
            magic: 10.0 + dto.magic_level,
        };
        let evasion_value = 10.0 + dto.defense_level;
        let resistance_value = 0.2 * dto.defense_level;
        Ok(Self {
            hrid: dto.hrid.clone(),
            current_hitpoints: max_hitpoints,
            max_hitpoints,
            current_manapoints: max_manapoints,
            max_manapoints,
            attack_interval,
            cast_speed: dto.attack_level / 2_000.0,
            combat_style_hrid: "/combat_styles/smash".into(),
            damage_type: "/damage_types/physical".into(),
            accuracy,
            max_damage,
            evasion: StyleValues {
                stab: evasion_value,
                slash: evasion_value,
                smash: evasion_value,
                ranged: evasion_value,
                magic: evasion_value,
            },
            resistance: DamageValues {
                physical: resistance_value,
                water: resistance_value,
                nature: resistance_value,
                fire: resistance_value,
            },
            amplify: DamageValues {
                physical: 0.0,
                water: 0.0,
                nature: 0.0,
                fire: 0.0,
            },
            penetration: DamageValues {
                physical: 0.0,
                water: 0.0,
                nature: 0.0,
                fire: 0.0,
            },
            critical_rate: 0.0,
            critical_damage: 0.0,
            task_damage: 0.0,
            damage_taken: 0.0,
            auto_attack_damage: 0.0,
            ability_damage: 0.0,
            mayhem: 0.0,
            pierce: 0.0,
            hp_regen_per_10: 0.01,
            mp_regen_per_10: 0.01,
        })
    }

    fn monster(data: &CombatMonsterDataV1) -> Result<Self> {
        let stats = data
            .combat_stats
            .as_object()
            .ok_or_else(|| unsupported(format!("{} combatStats must be an object", data.hrid)))?;
        let levels = &data.levels;
        let style = stats
            .get("combatStyleHrids")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_str)
            .ok_or_else(|| unsupported(format!("{} requires one combat style", data.hrid)))?;
        let damage_type = stats
            .get("damageType")
            .and_then(Value::as_str)
            .ok_or_else(|| unsupported(format!("{} requires a damage type", data.hrid)))?;
        let attack_speed = stat(stats, "attackSpeed");
        let attack_interval = SimTime::new(
            data.base_attack_interval.get()
                / (1.0 + levels.attack_level / 2_000.0)
                / (1.0 + attack_speed),
        )?;
        let melee_damage = 10.0 + levels.melee_level;
        let evasion_base = 10.0 + levels.defense_level;
        let resistance_base = 0.2 * levels.defense_level;
        Ok(Self {
            hrid: data.hrid.clone(),
            current_hitpoints: (10.0 * (10.0 + levels.stamina_level)).floor(),
            max_hitpoints: (10.0 * (10.0 + levels.stamina_level)).floor(),
            current_manapoints: (10.0 * (10.0 + levels.intelligence_level)).floor(),
            max_manapoints: (10.0 * (10.0 + levels.intelligence_level)).floor(),
            attack_interval,
            cast_speed: stat(stats, "castSpeed") + levels.attack_level / 2_000.0,
            combat_style_hrid: style.into(),
            damage_type: damage_type.into(),
            accuracy: StyleValues {
                stab: (10.0 + levels.attack_level) * (1.0 + stat(stats, "stabAccuracy")),
                slash: (10.0 + levels.attack_level) * (1.0 + stat(stats, "slashAccuracy")),
                smash: (10.0 + levels.attack_level) * (1.0 + stat(stats, "smashAccuracy")),
                ranged: (10.0 + levels.attack_level) * (1.0 + stat(stats, "rangedAccuracy")),
                magic: (10.0 + levels.attack_level) * (1.0 + stat(stats, "magicAccuracy")),
            },
            max_damage: StyleValues {
                stab: melee_damage * (1.0 + stat(stats, "stabDamage")),
                slash: melee_damage * (1.0 + stat(stats, "slashDamage")),
                smash: melee_damage * (1.0 + stat(stats, "smashDamage")),
                ranged: (10.0 + levels.ranged_level) * (1.0 + stat(stats, "rangedDamage")),
                magic: (10.0 + levels.magic_level) * (1.0 + stat(stats, "magicDamage")),
            },
            evasion: StyleValues {
                stab: evasion_base * (1.0 + stat(stats, "stabEvasion")),
                slash: evasion_base * (1.0 + stat(stats, "slashEvasion")),
                smash: evasion_base * (1.0 + stat(stats, "smashEvasion")),
                ranged: evasion_base * (1.0 + stat(stats, "rangedEvasion")),
                magic: evasion_base * (1.0 + stat(stats, "magicEvasion")),
            },
            resistance: DamageValues {
                physical: resistance_base + stat(stats, "armor"),
                water: resistance_base + stat(stats, "waterResistance"),
                nature: resistance_base + stat(stats, "natureResistance"),
                fire: resistance_base + stat(stats, "fireResistance"),
            },
            amplify: DamageValues {
                physical: stat(stats, "physicalAmplify"),
                water: stat(stats, "waterAmplify"),
                nature: stat(stats, "natureAmplify"),
                fire: stat(stats, "fireAmplify"),
            },
            penetration: DamageValues {
                physical: stat(stats, "armorPenetration"),
                water: stat(stats, "waterPenetration"),
                nature: stat(stats, "naturePenetration"),
                fire: stat(stats, "firePenetration"),
            },
            critical_rate: stat(stats, "criticalRate"),
            critical_damage: stat(stats, "criticalDamage"),
            task_damage: stat(stats, "taskDamage"),
            damage_taken: 0.0,
            auto_attack_damage: stat(stats, "autoAttackDamage"),
            ability_damage: stat(stats, "abilityDamage"),
            mayhem: stat(stats, "mayhem"),
            pierce: stat(stats, "pierce"),
            hp_regen_per_10: 0.0,
            mp_regen_per_10: 0.0,
        })
    }

    fn is_alive(&self) -> bool {
        self.current_hitpoints > 0.0
    }

    fn restore(&mut self) {
        self.current_hitpoints = self.max_hitpoints;
        self.current_manapoints = self.max_manapoints;
    }
}

fn stat(stats: &serde_json::Map<String, Value>, name: &str) -> f64 {
    stats.get(name).and_then(Value::as_f64).unwrap_or(0.0)
}

#[derive(Clone, Debug)]
struct DirectAbilityEffect {
    combat_style_hrid: String,
    damage_type: String,
    damage_flat: f64,
    damage_ratio: f64,
    bonus_accuracy_ratio: f64,
    armor_damage_ratio: f64,
    pierce_chance: f64,
}

#[derive(Clone, Debug)]
struct DirectAbilityRuntime {
    hrid: String,
    mana_cost: f64,
    cooldown_duration: SimTime,
    cast_duration: SimTime,
    last_used: f64,
    effect: DirectAbilityEffect,
}

impl DirectAbilityRuntime {
    fn aqua_arrow(snapshot: &AbilityDataSnapshotV1, level: u32) -> Result<Self> {
        let report = classify_direct_damage_abilities(snapshot)?;
        if !report.candidates.contains_key(AQUA_ARROW_HRID) {
            return Err(unsupported("Aqua Arrow is not a direct-damage candidate"));
        }
        let ability = snapshot
            .ability(AQUA_ARROW_HRID)
            .ok_or_else(|| unsupported("Aqua Arrow is absent from the ability snapshot"))?;
        if ability.ability_effects.len() != 1 {
            return Err(unsupported("Aqua Arrow must have exactly one effect"));
        }
        let effect = resolve_effect(&ability.ability_effects[0], level);
        Ok(Self {
            hrid: ability.hrid.clone(),
            mana_cost: ability.mana_cost,
            cooldown_duration: ability.cooldown_duration,
            cast_duration: ability.cast_duration,
            last_used: INITIAL_LAST_USED,
            effect,
        })
    }

    fn is_ready(&self, current_time: SimTime) -> bool {
        self.last_used + self.cooldown_duration.get() <= current_time.get()
    }
}

fn resolve_effect(effect: &AbilityEffectDataV1, level: u32) -> DirectAbilityEffect {
    let level_offset = f64::from(level.saturating_sub(1));
    DirectAbilityEffect {
        combat_style_hrid: effect.combat_style_hrid.clone(),
        damage_type: effect.damage_type.clone(),
        damage_flat: effect.base_damage_flat + level_offset * effect.base_damage_flat_level_bonus,
        damage_ratio: effect.base_damage_ratio
            + level_offset * effect.base_damage_ratio_level_bonus,
        bonus_accuracy_ratio: effect.bonus_accuracy_ratio
            + level_offset * effect.bonus_accuracy_ratio_level_bonus,
        armor_damage_ratio: effect.armor_damage_ratio
            + level_offset * effect.armor_damage_ratio_level_bonus,
        pierce_chance: effect.pierce_chance,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Side {
    Player,
    Enemy,
}

#[derive(Clone, Copy, Debug)]
enum DirectEvent {
    CombatStart,
    AutoAttack(Side),
    AbilityCastEnd,
    RegenTick,
    EnemyRespawn,
    PlayerRespawn,
}

#[derive(Clone, Copy, Debug)]
struct AttackOutcome {
    did_hit: bool,
    damage_done: u64,
}

enum DirectRandom {
    Seeded(SeededRandom),
    Sequence(SequenceRandom),
}

impl DirectRandom {
    fn from_request(request: &SimulationRequestV1) -> Result<Self> {
        match request.random.as_ref() {
            Some(RandomConfigV1::Seeded {
                seed: Value::String(seed),
            }) => Ok(Self::Seeded(SeededRandom::from_seed_text(seed))),
            Some(RandomConfigV1::Seeded { .. }) => Err(unsupported(
                "M3D seeded RNG requires a string seed for JavaScript parity",
            )),
            Some(RandomConfigV1::Sequence {
                values,
                loop_values,
            }) => Ok(Self::Sequence(SequenceRandom::new(
                values.clone(),
                *loop_values,
            )?)),
            Some(RandomConfigV1::Native) | None => Err(unsupported(
                "M3D requires a deterministic seeded or sequence random source",
            )),
        }
    }

    fn next(&mut self) -> Result<f64> {
        match self {
            Self::Seeded(random) => random.next_unit_f64(),
            Self::Sequence(random) => random.next_unit_f64(),
        }
    }

    fn draw_count(&self) -> u64 {
        match self {
            Self::Seeded(random) => random.draw_count(),
            Self::Sequence(random) => random.draw_count(),
        }
    }
}

struct DirectDamageEngine {
    request: SimulationRequestV1,
    basic_data: BasicGameData,
    player: DirectUnit,
    enemy: Option<DirectUnit>,
    ability: DirectAbilityRuntime,
    random: DirectRandom,
    queue: StableEventQueue<DirectEvent>,
    current_time: SimTime,
    result: DirectDamageCombatResultV1,
    player_action: Option<EventHandle>,
    enemy_action: Option<EventHandle>,
    enemy_respawn: Option<EventHandle>,
    player_respawn: Option<EventHandle>,
    fly: CombatMonsterDataV1,
}

impl DirectDamageEngine {
    fn new(request: SimulationRequestV1) -> Result<Self> {
        let basic_data = BasicGameData::embedded()?;
        let combat_snapshot = CombatZoneDataSnapshotV1::embedded()?;
        let ability_snapshot = AbilityDataSnapshotV1::embedded()?;
        validate_request_scope(&request, &basic_data)?;
        let player_dto = DirectPlayerDto::parse(&request.players[0])?;
        let player = DirectUnit::player(&player_dto)?;
        let fly = combat_snapshot
            .monster(BASIC_FLY_MONSTER_HRID)
            .ok_or_else(|| unsupported("Fly is absent from the Combat Zone snapshot"))?
            .clone();
        let ability = DirectAbilityRuntime::aqua_arrow(&ability_snapshot, 1)?;
        let random = DirectRandom::from_request(&request)?;
        let result = DirectDamageCombatResultV1::new(&request);
        let mut queue = StableEventQueue::new();
        queue.schedule(SimTime::ZERO, DirectEvent::CombatStart);
        Ok(Self {
            request,
            basic_data,
            player,
            enemy: None,
            ability,
            random,
            queue,
            current_time: SimTime::ZERO,
            result,
            player_action: None,
            enemy_action: None,
            enemy_respawn: None,
            player_respawn: None,
            fly,
        })
    }

    fn run(mut self) -> Result<DirectDamageCombatResultV1> {
        while self.current_time < self.request.simulation_time_limit {
            let scheduled = self
                .queue
                .pop_next()
                .ok_or(SimError::DirectDamageQueueExhausted)?;
            self.current_time = scheduled.key.at;
            self.result.events_processed += 1;
            self.process_event(scheduled.handle, scheduled.payload)?;
        }
        self.result.simulated_time = self.current_time;
        self.result.random_draws = self.random.draw_count();
        Ok(self.result)
    }

    fn process_event(&mut self, handle: EventHandle, event: DirectEvent) -> Result<()> {
        match event {
            DirectEvent::CombatStart => self.process_combat_start(),
            DirectEvent::AutoAttack(side) => {
                match side {
                    Side::Player if self.player_action == Some(handle) => self.player_action = None,
                    Side::Enemy if self.enemy_action == Some(handle) => self.enemy_action = None,
                    _ => {}
                }
                self.process_auto_attack(side)
            }
            DirectEvent::AbilityCastEnd => {
                if self.player_action == Some(handle) {
                    self.player_action = None;
                }
                self.process_ability_cast_end()
            }
            DirectEvent::RegenTick => self.process_regen_tick(),
            DirectEvent::EnemyRespawn => {
                if self.enemy_respawn == Some(handle) {
                    self.enemy_respawn = None;
                }
                self.spawn_enemy()
            }
            DirectEvent::PlayerRespawn => {
                if self.player_respawn == Some(handle) {
                    self.player_respawn = None;
                }
                self.process_player_respawn()
            }
        }
    }

    fn process_combat_start(&mut self) -> Result<()> {
        self.player.restore();
        self.schedule_event_after(REGEN_TICK_INTERVAL, DirectEvent::RegenTick);
        self.spawn_enemy()
    }

    fn spawn_enemy(&mut self) -> Result<()> {
        let _random_weight = self.random.next()?;
        self.enemy = Some(DirectUnit::monster(&self.fly)?);
        if self.player.is_alive() {
            self.schedule_player_action()?;
            self.schedule_enemy_action()?;
        }
        Ok(())
    }

    fn schedule_player_action(&mut self) -> Result<()> {
        if !self.player.is_alive() || !self.enemy.as_ref().is_some_and(DirectUnit::is_alive) {
            return Ok(());
        }
        let event = if self.ability.is_ready(self.current_time)
            && self.player.current_manapoints >= self.ability.mana_cost
        {
            let cast_duration = self.ability.cast_duration.get() / (1.0 + self.player.cast_speed);
            let handle = self.schedule_event_after(cast_duration, DirectEvent::AbilityCastEnd);
            self.player_action = Some(handle);
            return Ok(());
        } else {
            DirectEvent::AutoAttack(Side::Player)
        };
        let handle = self.schedule_event_after(self.player.attack_interval.get(), event);
        self.player_action = Some(handle);
        Ok(())
    }

    fn schedule_enemy_action(&mut self) -> Result<()> {
        let interval = self
            .enemy
            .as_ref()
            .filter(|enemy| enemy.is_alive())
            .ok_or_else(|| unsupported("cannot schedule an enemy attack without a live enemy"))?
            .attack_interval
            .get();
        let handle = self.schedule_event_after(interval, DirectEvent::AutoAttack(Side::Enemy));
        self.enemy_action = Some(handle);
        Ok(())
    }

    fn process_auto_attack(&mut self, side: Side) -> Result<()> {
        let (source, target) = match side {
            Side::Player => {
                let Some(enemy) = self.enemy.as_ref() else {
                    return Ok(());
                };
                if !self.player.is_alive() || !enemy.is_alive() {
                    return Ok(());
                }
                (self.player.clone(), enemy.clone())
            }
            Side::Enemy => {
                let Some(enemy) = self.enemy.as_ref() else {
                    return Ok(());
                };
                if !enemy.is_alive() || !self.player.is_alive() {
                    return Ok(());
                }
                (enemy.clone(), self.player.clone())
            }
        };

        let outcome = process_attack(&source, &target, None, &mut self.random)?;
        match side {
            Side::Player => {
                if let Some(enemy) = self.enemy.as_mut() {
                    enemy.current_hitpoints -= outcome.damage_done as f64;
                }
            }
            Side::Enemy => self.player.current_hitpoints -= outcome.damage_done as f64,
        }

        let _mayhem = source.mayhem > self.random.next()?;
        self.result
            .record_attack(&source.hrid, &target.hrid, "autoAttack", outcome);
        if outcome.did_hit {
            let _pierced = source.pierce > self.random.next()?;
        }

        let target_died = match side {
            Side::Player => self.enemy.as_ref().is_some_and(|enemy| !enemy.is_alive()),
            Side::Enemy => !self.player.is_alive(),
        };
        if target_died {
            match side {
                Side::Player => self.process_enemy_death()?,
                Side::Enemy => self.process_player_death()?,
            }
            return Ok(());
        }

        match side {
            Side::Player => self.schedule_player_action(),
            Side::Enemy => self.schedule_enemy_action(),
        }
    }

    fn process_ability_cast_end(&mut self) -> Result<()> {
        let Some(enemy) = self.enemy.as_ref() else {
            return Ok(());
        };
        if !self.player.is_alive() || !enemy.is_alive() {
            return Ok(());
        }
        if self.player.current_manapoints < self.ability.mana_cost {
            return Err(unsupported(
                "Aqua Arrow reached cast completion without enough mana; M3D OOM recovery is not implemented",
            ));
        }

        self.player.current_manapoints -= self.ability.mana_cost;
        self.result.record_mana_used(
            &self.player.hrid,
            &self.ability.hrid,
            self.ability.mana_cost,
        )?;
        self.ability.last_used = self.current_time.get();

        let source = self.player.clone();
        let target = enemy.clone();
        let outcome = process_attack(
            &source,
            &target,
            Some(&self.ability.effect),
            &mut self.random,
        )?;
        if let Some(enemy) = self.enemy.as_mut() {
            enemy.current_hitpoints -= outcome.damage_done as f64;
        }
        self.result
            .record_attack(&source.hrid, &target.hrid, &self.ability.hrid, outcome);
        if outcome.did_hit {
            let _pierced = self.ability.effect.pierce_chance > self.random.next()?;
        }

        self.schedule_player_action()?;
        if self.enemy.as_ref().is_some_and(|enemy| !enemy.is_alive()) {
            self.process_enemy_death()?;
        }
        Ok(())
    }

    fn process_enemy_death(&mut self) -> Result<()> {
        let enemy_hrid = self
            .enemy
            .as_ref()
            .map(|enemy| enemy.hrid.clone())
            .ok_or_else(|| unsupported("enemy death was processed without an enemy"))?;
        self.result.record_death(&enemy_hrid);
        self.result.encounters += 1;
        self.result.last_encounter_finish_time = self.current_time;
        self.enemy = None;
        self.cancel_action_handles();
        let handle = self.schedule_event_after(
            self.basic_data.zone.respawn_interval.get(),
            DirectEvent::EnemyRespawn,
        );
        self.enemy_respawn = Some(handle);
        Ok(())
    }

    fn process_player_death(&mut self) -> Result<()> {
        let player_hrid = self.player.hrid.clone();
        self.result.record_death(&player_hrid);
        self.cancel_action_handles();
        let handle = self.schedule_event_after(PLAYER_RESPAWN_INTERVAL, DirectEvent::PlayerRespawn);
        self.player_respawn = Some(handle);
        Ok(())
    }

    fn process_player_respawn(&mut self) -> Result<()> {
        self.player.restore();
        if self.enemy.as_ref().is_some_and(DirectUnit::is_alive) {
            self.schedule_player_action()?;
            self.schedule_enemy_action()?;
        }
        Ok(())
    }

    fn process_regen_tick(&mut self) -> Result<()> {
        if self.player.is_alive() {
            let hitpoints = (self.player.max_hitpoints * self.player.hp_regen_per_10).floor();
            self.player.current_hitpoints =
                (self.player.current_hitpoints + hitpoints).min(self.player.max_hitpoints);
            let manapoints = (self.player.max_manapoints * self.player.mp_regen_per_10).floor();
            self.player.current_manapoints =
                (self.player.current_manapoints + manapoints).min(self.player.max_manapoints);
        }
        self.schedule_event_after(REGEN_TICK_INTERVAL, DirectEvent::RegenTick);
        Ok(())
    }

    fn cancel_action_handles(&mut self) {
        cancel_if_live(&mut self.queue, self.player_action.take());
        cancel_if_live(&mut self.queue, self.enemy_action.take());
    }

    fn schedule_event_after(&mut self, duration: f64, event: DirectEvent) -> EventHandle {
        let at = SimTime::new(self.current_time.get() + duration)
            .expect("validated simulation times must remain finite and non-negative");
        self.queue.schedule(at, event)
    }
}

fn cancel_if_live(queue: &mut StableEventQueue<DirectEvent>, handle: Option<EventHandle>) {
    if let Some(handle) = handle {
        let _ = queue.cancel(handle);
    }
}

fn process_attack(
    source: &DirectUnit,
    target: &DirectUnit,
    effect: Option<&DirectAbilityEffect>,
    random: &mut DirectRandom,
) -> Result<AttackOutcome> {
    let combat_style = effect
        .map(|effect| effect.combat_style_hrid.as_str())
        .unwrap_or(&source.combat_style_hrid);
    let damage_type = effect
        .map(|effect| effect.damage_type.as_str())
        .unwrap_or(&source.damage_type);
    let mut source_accuracy = source.accuracy.get(combat_style)?;
    if let Some(effect) = effect {
        source_accuracy *= 1.0 + effect.bonus_accuracy_ratio;
    }
    let source_max_damage = source.max_damage.get(combat_style)?;
    let target_evasion = target.evasion.get(combat_style)?;
    let hit_chance =
        source_accuracy.powf(1.4) / (source_accuracy.powf(1.4) + target_evasion.powf(1.4));
    let mut crit_chance = source.critical_rate;
    if combat_style == "/combat_styles/ranged" {
        crit_chance += 0.3 * hit_chance;
    }

    let source_damage_multiplier = 1.0 + source.amplify.get(damage_type)?;
    let base_damage_flat = effect.map(|effect| effect.damage_flat).unwrap_or(0.0);
    let base_damage_ratio = effect.map(|effect| effect.damage_ratio).unwrap_or(1.0);
    let armor_damage_flat = effect
        .map(|effect| effect.armor_damage_ratio * source.resistance.physical)
        .unwrap_or(0.0);
    let mut source_min_damage =
        source_damage_multiplier * (1.0 + base_damage_flat + armor_damage_flat);
    let mut source_max_damage = source_damage_multiplier
        * (base_damage_ratio * source_max_damage + base_damage_flat + armor_damage_flat);

    if random.next()? < crit_chance {
        source_max_damage *= 1.0 + source.critical_damage;
        source_min_damage = source_max_damage;
    }

    let mut damage_roll = random_int(source_min_damage, source_max_damage, random)?;
    damage_roll *= 1.0 + source.task_damage;
    damage_roll *= 1.0 + target.damage_taken;
    if effect.is_some() {
        damage_roll *= 1.0 + source.ability_damage;
    } else {
        damage_roll += damage_roll * source.auto_attack_damage;
    }

    if random.next()? >= hit_chance {
        return Ok(AttackOutcome {
            did_hit: false,
            damage_done: 0,
        });
    }

    let target_resistance = target.resistance.get(damage_type)?;
    let source_penetration = source.penetration.get(damage_type)?;
    let penetrated_resistance = if source_penetration > 0.0 && target_resistance > 0.0 {
        target_resistance / (1.0 + source_penetration)
    } else {
        target_resistance
    };
    let damage_taken_ratio = if penetrated_resistance < 0.0 {
        (100.0 - penetrated_resistance) / 100.0
    } else {
        100.0 / (100.0 + penetrated_resistance)
    };
    let mitigated_damage = (damage_taken_ratio * damage_roll).ceil();
    Ok(AttackOutcome {
        did_hit: true,
        damage_done: mitigated_damage.min(target.current_hitpoints) as u64,
    })
}

fn random_int(minimum: f64, maximum: f64, random: &mut DirectRandom) -> Result<f64> {
    let (minimum, maximum) = if maximum < minimum {
        (maximum, minimum)
    } else {
        (minimum, maximum)
    };
    let minimum_ceil = minimum.ceil();
    let maximum_floor = maximum.floor();
    if minimum.floor() == maximum_floor {
        return Ok(((minimum + maximum) / 2.0 + random.next()?).floor());
    }

    let minimum_tail = -(minimum - minimum_ceil);
    let maximum_tail = maximum - maximum_floor;
    let balanced_weight = 2.0 * minimum_tail + (maximum_floor - minimum_ceil);
    let balanced_average = (maximum_floor + minimum_ceil) / 2.0;
    let average = (maximum + minimum) / 2.0;
    let extra_tail_weight =
        balanced_weight * (average - balanced_average) / (maximum_floor + 1.0 - average);
    let extra_tail_chance = (extra_tail_weight / (extra_tail_weight + balanced_weight)).abs();

    if random.next()? < extra_tail_chance {
        return Ok(if maximum_tail > minimum_tail {
            (maximum_floor + 1.0).floor()
        } else {
            (minimum_ceil - 1.0).floor()
        });
    }

    if maximum_tail > minimum_tail {
        Ok((minimum + random.next()? * (maximum_floor + minimum_tail - minimum + 1.0)).floor())
    } else {
        let lower = minimum_ceil - maximum_tail;
        Ok((lower + random.next()? * (maximum - lower + 1.0)).floor())
    }
}

fn validate_request_scope(request: &SimulationRequestV1, data: &BasicGameData) -> Result<()> {
    request.validate()?;
    if request.data_version != BASIC_DATA_VERSION || request.data_version != data.data_version {
        return Err(unsupported(format!(
            "dataVersion must be {BASIC_DATA_VERSION}"
        )));
    }
    if request.players.len() != 1 {
        return Err(unsupported("M3D supports exactly one player"));
    }
    match &request.target {
        SimulationTargetV1::Zone {
            zone_hrid,
            difficulty_tier,
            extensions,
        } if zone_hrid == BASIC_FLY_ZONE_HRID && *difficulty_tier == 0 && extensions.is_empty() => {
        }
        _ => {
            return Err(unsupported(
                "M3D supports only the tier-0 /actions/combat/fly Zone",
            ));
        }
    }
    if request.options.statistics_mode != StatisticsMode::Full {
        return Err(unsupported("statisticsMode must be full"));
    }
    if request.options.enable_hp_mp_visualization {
        return Err(unsupported("HP/MP visualization is not supported in M3D"));
    }
    if request.options.trace.enabled {
        return Err(unsupported(
            "event trace is not supported in the Rust M3D runtime",
        ));
    }
    if !is_empty_json_collection(&request.options.extra)
        || !request.options.extensions.is_empty()
        || !request.extensions.is_empty()
    {
        return Err(unsupported(
            "M3D does not accept extra or unknown request options",
        ));
    }
    Ok(())
}

fn unsupported(message: impl Into<String>) -> SimError {
    SimError::UnsupportedDirectDamageCombat(message.into())
}

pub fn simulate_direct_damage(request: &SimulationRequestV1) -> Result<DirectDamageCombatResultV1> {
    DirectDamageEngine::new(request.clone())?.run()
}

pub fn simulate_direct_damage_json(input: &str) -> Result<String> {
    let request = SimulationRequestV1::from_json(input)?;
    Ok(serde_json::to_string_pretty(&simulate_direct_damage(
        &request,
    )?)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUEST: &str =
        include_str!("../../../fixtures/parity/ability-aqua-arrow-basic/request.json");
    const EXPECTED: &str =
        include_str!("../../../fixtures/parity/ability-aqua-arrow-basic/expected-result.json");
    const METADATA: &str =
        include_str!("../../../fixtures/parity/ability-aqua-arrow-basic/metadata.json");

    #[test]
    fn matches_the_aqua_arrow_reference_subset() {
        let request = SimulationRequestV1::from_json(REQUEST).unwrap();
        let result = simulate_direct_damage(&request).unwrap();
        let expected: Value = serde_json::from_str(EXPECTED).unwrap();
        let metadata: Value = serde_json::from_str(METADATA).unwrap();

        assert_eq!(result.encounters, expected["encounters"]);
        assert_eq!(
            serde_json::to_value(&result.deaths).unwrap(),
            expected["deaths"]
        );
        assert_eq!(
            serde_json::to_value(&result.attacks).unwrap(),
            expected["attacks"]
        );
        assert_eq!(
            serde_json::to_value(&result.mana_used).unwrap(),
            expected["manaUsed"]
        );
        assert_eq!(result.simulated_time.get(), expected["simulatedTime"]);
        assert_eq!(
            result.last_encounter_finish_time.get(),
            expected["lastEncounterFinishTime"]
        );
        assert_eq!(result.random_draws, metadata["randomDraws"]);
    }

    #[test]
    fn direct_damage_replay_is_exact() {
        let request = SimulationRequestV1::from_json(REQUEST).unwrap();
        assert_eq!(
            simulate_direct_damage(&request).unwrap(),
            simulate_direct_damage(&request).unwrap()
        );
    }

    #[test]
    fn rejects_unpromoted_abilities() {
        let mut request = SimulationRequestV1::from_json(REQUEST).unwrap();
        request.players[0]["abilities"][0]["hrid"] = Value::from("/abilities/fireball");
        assert!(matches!(
            simulate_direct_damage(&request),
            Err(SimError::UnsupportedDirectDamageCombat(_))
        ));
    }
}
