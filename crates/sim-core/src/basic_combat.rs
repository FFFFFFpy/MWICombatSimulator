use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    BASIC_DATA_VERSION, BASIC_FLY_ZONE_HRID, BasicGameData, EventHandle, RandomConfigV1,
    RandomSource, Result, SeededRandom, SequenceRandom, SimError, SimTime, SimulationRequestV1,
    SimulationTargetV1, StableEventQueue, StatisticsMode,
};

pub const BASIC_COMBAT_ENGINE_ID: &str = "rust-basic-combat";
pub const BASIC_COMBAT_COMPATIBILITY_LEVEL: &str = "basic-auto-attack-v1";

const ONE_SECOND: f64 = 1_000_000_000.0;
const REGEN_TICK_INTERVAL: f64 = 10.0 * ONE_SECOND;
const PLAYER_RESPAWN_INTERVAL: f64 = 150.0 * ONE_SECOND;

pub type AttackHistogram =
    BTreeMap<String, BTreeMap<String, BTreeMap<String, BTreeMap<String, u64>>>>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BasicCombatResultV1 {
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
    pub last_encounter_finish_time: SimTime,
    pub random_draws: u64,
    pub events_processed: u64,
}

impl BasicCombatResultV1 {
    fn new(request: &SimulationRequestV1, data: &BasicGameData) -> Self {
        Self {
            contract_version: request.contract_version,
            result_type: "basic_combat_result".into(),
            request_id: request.request_id.clone(),
            engine: BASIC_COMBAT_ENGINE_ID.into(),
            engine_version: env!("CARGO_PKG_VERSION").into(),
            data_version: data.data_version.clone(),
            compatibility_level: BASIC_COMBAT_COMPATIBILITY_LEVEL.into(),
            zone_name: data.zone.hrid.clone(),
            simulated_time: SimTime::ZERO,
            encounters: 0,
            deaths: BTreeMap::new(),
            attacks: BTreeMap::new(),
            last_encounter_finish_time: SimTime::ZERO,
            random_draws: 0,
            events_processed: 0,
        }
    }

    fn record_attack(&mut self, source: &str, target: &str, outcome: AttackOutcome) {
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
            .entry("autoAttack".into())
            .or_default()
            .entry(hit)
            .or_default() += 1;
    }

    fn record_death(&mut self, hrid: &str) {
        *self.deaths.entry(hrid.into()).or_default() += 1;
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BasicPlayerDto {
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
    abilities: Vec<Value>,
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

impl BasicPlayerDto {
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
            || !self.abilities.is_empty()
            || !is_empty_json_collection(&self.house_rooms)
            || !is_empty_json_collection(&self.guild_buffs)
            || !is_empty_json_collection(&self.achievements)
        {
            return Err(unsupported(
                "M2 only supports players without equipment, abilities, consumables, rooms, guild buffs, or achievements",
            ));
        }
        if self.debuff_on_level_gap != 0.0 {
            return Err(unsupported("debuffOnLevelGap must be zero in M2"));
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Side {
    Player,
    Enemy,
}

#[derive(Clone, Copy, Debug)]
enum BasicEvent {
    CombatStart,
    AutoAttack(Side),
    RegenTick,
    EnemyRespawn,
    PlayerRespawn,
}

#[derive(Clone, Debug)]
struct BasicUnit {
    hrid: String,
    current_hitpoints: f64,
    max_hitpoints: f64,
    current_manapoints: f64,
    max_manapoints: f64,
    attack_interval: SimTime,
    smash_accuracy_rating: f64,
    smash_max_damage: f64,
    smash_evasion_rating: f64,
    total_armor: f64,
    critical_rate: f64,
    critical_damage: f64,
    physical_amplify: f64,
    task_damage: f64,
    damage_taken: f64,
    auto_attack_damage: f64,
    pierce: f64,
    mayhem: f64,
    hp_regen_per_10: f64,
    mp_regen_per_10: f64,
}

impl BasicUnit {
    fn player(dto: BasicPlayerDto) -> Result<Self> {
        let max_hitpoints = (10.0 * (10.0 + dto.stamina_level)).floor();
        let max_manapoints = (10.0 * (10.0 + dto.intelligence_level)).floor();
        let attack_interval = SimTime::new(3_000_000_000.0 / (1.0 + dto.attack_level / 2_000.0))?;
        Ok(Self {
            hrid: dto.hrid,
            current_hitpoints: max_hitpoints,
            max_hitpoints,
            current_manapoints: max_manapoints,
            max_manapoints,
            attack_interval,
            smash_accuracy_rating: 10.0 + dto.attack_level,
            smash_max_damage: 10.0 + dto.melee_level,
            smash_evasion_rating: 10.0 + dto.defense_level,
            total_armor: 0.2 * dto.defense_level,
            critical_rate: 0.0,
            critical_damage: 0.0,
            physical_amplify: 0.0,
            task_damage: 0.0,
            damage_taken: 0.0,
            auto_attack_damage: 0.0,
            pierce: 0.0,
            mayhem: 0.0,
            hp_regen_per_10: 0.01,
            mp_regen_per_10: 0.01,
        })
    }

    fn enemy(data: &BasicGameData) -> Result<Self> {
        Ok(Self {
            hrid: data.monster.hrid.clone(),
            current_hitpoints: data.monster.max_hitpoints,
            max_hitpoints: data.monster.max_hitpoints,
            current_manapoints: data.monster.max_manapoints,
            max_manapoints: data.monster.max_manapoints,
            attack_interval: data.monster.attack_interval()?,
            smash_accuracy_rating: data.monster.smash_accuracy_rating,
            smash_max_damage: data.monster.smash_max_damage,
            smash_evasion_rating: data.monster.smash_evasion_rating,
            total_armor: data.monster.total_armor,
            critical_rate: 0.0,
            critical_damage: 0.0,
            physical_amplify: 0.0,
            task_damage: 0.0,
            damage_taken: 0.0,
            auto_attack_damage: data.monster.auto_attack_damage,
            pierce: 0.0,
            mayhem: 0.0,
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

#[derive(Clone, Copy, Debug)]
struct AttackOutcome {
    did_hit: bool,
    damage_done: u64,
}

enum BasicRandom {
    Seeded(SeededRandom),
    Sequence(SequenceRandom),
}

impl BasicRandom {
    fn from_request(request: &SimulationRequestV1) -> Result<Self> {
        match request.random.as_ref() {
            Some(RandomConfigV1::Seeded {
                seed: Value::String(seed),
            }) => Ok(Self::Seeded(SeededRandom::from_seed_text(seed))),
            Some(RandomConfigV1::Seeded { .. }) => Err(unsupported(
                "M2 seeded RNG requires a string seed for JavaScript parity",
            )),
            Some(RandomConfigV1::Sequence {
                values,
                loop_values,
            }) => Ok(Self::Sequence(SequenceRandom::new(
                values.clone(),
                *loop_values,
            )?)),
            Some(RandomConfigV1::Native) | None => Err(unsupported(
                "M2 requires a deterministic seeded or sequence random source",
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

struct BasicCombatEngine {
    request: SimulationRequestV1,
    data: BasicGameData,
    player: BasicUnit,
    enemy: Option<BasicUnit>,
    random: BasicRandom,
    queue: StableEventQueue<BasicEvent>,
    current_time: SimTime,
    result: BasicCombatResultV1,
    player_attack: Option<EventHandle>,
    enemy_attack: Option<EventHandle>,
    enemy_respawn: Option<EventHandle>,
    player_respawn: Option<EventHandle>,
}

impl BasicCombatEngine {
    fn new(request: SimulationRequestV1) -> Result<Self> {
        let data = BasicGameData::embedded()?;
        validate_request_scope(&request, &data)?;
        let player = BasicUnit::player(BasicPlayerDto::parse(&request.players[0])?)?;
        let random = BasicRandom::from_request(&request)?;
        let result = BasicCombatResultV1::new(&request, &data);
        let mut queue = StableEventQueue::new();
        queue.schedule(SimTime::ZERO, BasicEvent::CombatStart);
        Ok(Self {
            request,
            data,
            player,
            enemy: None,
            random,
            queue,
            current_time: SimTime::ZERO,
            result,
            player_attack: None,
            enemy_attack: None,
            enemy_respawn: None,
            player_respawn: None,
        })
    }

    fn run(mut self) -> Result<BasicCombatResultV1> {
        while self.current_time < self.request.simulation_time_limit {
            let scheduled = self
                .queue
                .pop_next()
                .ok_or(SimError::SimulationQueueExhausted)?;
            self.current_time = scheduled.key.at;
            self.result.events_processed += 1;
            self.process_event(scheduled.handle, scheduled.payload)?;
        }
        self.result.simulated_time = self.current_time;
        self.result.random_draws = self.random.draw_count();
        Ok(self.result)
    }

    fn process_event(&mut self, handle: EventHandle, event: BasicEvent) -> Result<()> {
        match event {
            BasicEvent::CombatStart => self.process_combat_start(),
            BasicEvent::AutoAttack(side) => {
                match side {
                    Side::Player if self.player_attack == Some(handle) => self.player_attack = None,
                    Side::Enemy if self.enemy_attack == Some(handle) => self.enemy_attack = None,
                    _ => {}
                }
                self.process_auto_attack(side)
            }
            BasicEvent::RegenTick => self.process_regen_tick(),
            BasicEvent::EnemyRespawn => {
                if self.enemy_respawn == Some(handle) {
                    self.enemy_respawn = None;
                }
                self.spawn_enemy()
            }
            BasicEvent::PlayerRespawn => {
                if self.player_respawn == Some(handle) {
                    self.player_respawn = None;
                }
                self.process_player_respawn()
            }
        }
    }

    fn process_combat_start(&mut self) -> Result<()> {
        self.player.restore();
        self.schedule_event_after(REGEN_TICK_INTERVAL, BasicEvent::RegenTick);
        self.spawn_enemy()
    }

    fn spawn_enemy(&mut self) -> Result<()> {
        let _random_weight = self.random.next()?;
        self.enemy = Some(BasicUnit::enemy(&self.data)?);
        if self.player.is_alive() {
            self.schedule_attack(Side::Player)?;
            self.schedule_attack(Side::Enemy)?;
        }
        Ok(())
    }

    fn schedule_attack(&mut self, side: Side) -> Result<()> {
        let interval = match side {
            Side::Player => self.player.attack_interval,
            Side::Enemy => {
                self.enemy
                    .as_ref()
                    .ok_or_else(|| unsupported("cannot schedule an enemy attack without an enemy"))?
                    .attack_interval
            }
        };
        let handle = self.schedule_event_after(interval.get(), BasicEvent::AutoAttack(side));
        match side {
            Side::Player => self.player_attack = Some(handle),
            Side::Enemy => self.enemy_attack = Some(handle),
        }
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

        let outcome = process_attack(&source, &target, &mut self.random)?;
        match side {
            Side::Player => {
                if let Some(enemy) = self.enemy.as_mut() {
                    enemy.current_hitpoints -= outcome.damage_done as f64;
                }
            }
            Side::Enemy => {
                self.player.current_hitpoints -= outcome.damage_done as f64;
            }
        }

        let _mayhem = source.mayhem > self.random.next()?;
        self.result
            .record_attack(&source.hrid, &target.hrid, outcome);
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

        self.schedule_attack(side)
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
        self.cancel_attack_handles();
        let handle = self.schedule_event_after(
            self.data.zone.respawn_interval.get(),
            BasicEvent::EnemyRespawn,
        );
        self.enemy_respawn = Some(handle);
        Ok(())
    }

    fn process_player_death(&mut self) -> Result<()> {
        let player_hrid = self.player.hrid.clone();
        self.result.record_death(&player_hrid);
        self.cancel_attack_handles();
        let handle = self.schedule_event_after(PLAYER_RESPAWN_INTERVAL, BasicEvent::PlayerRespawn);
        self.player_respawn = Some(handle);
        Ok(())
    }

    fn process_player_respawn(&mut self) -> Result<()> {
        self.player.restore();
        if self.enemy.as_ref().is_some_and(BasicUnit::is_alive) {
            self.schedule_attack(Side::Player)?;
            self.schedule_attack(Side::Enemy)?;
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
        self.schedule_event_after(REGEN_TICK_INTERVAL, BasicEvent::RegenTick);
        Ok(())
    }

    fn cancel_attack_handles(&mut self) {
        cancel_if_live(&mut self.queue, self.player_attack.take());
        cancel_if_live(&mut self.queue, self.enemy_attack.take());
    }

    fn schedule_event_after(&mut self, duration: f64, event: BasicEvent) -> EventHandle {
        let at = SimTime::new(self.current_time.get() + duration)
            .expect("validated simulation times must remain finite and non-negative");
        self.queue.schedule(at, event)
    }
}

fn cancel_if_live(queue: &mut StableEventQueue<BasicEvent>, handle: Option<EventHandle>) {
    if let Some(handle) = handle {
        let _ = queue.cancel(handle);
    }
}

fn process_attack(
    source: &BasicUnit,
    target: &BasicUnit,
    random: &mut BasicRandom,
) -> Result<AttackOutcome> {
    let hit_chance = source.smash_accuracy_rating.powf(1.4)
        / (source.smash_accuracy_rating.powf(1.4) + target.smash_evasion_rating.powf(1.4));
    let crit_chance = source.critical_rate;
    let source_damage_multiplier = 1.0 + source.physical_amplify;
    let mut source_min_damage = source_damage_multiplier;
    let mut source_max_damage = source_damage_multiplier * source.smash_max_damage;

    if random.next()? < crit_chance {
        source_max_damage *= 1.0 + source.critical_damage;
        source_min_damage = source_max_damage;
    }

    let mut damage_roll = random_int(source_min_damage, source_max_damage, random)?;
    damage_roll *= 1.0 + source.task_damage;
    damage_roll *= 1.0 + target.damage_taken;
    damage_roll += damage_roll * source.auto_attack_damage;

    if random.next()? >= hit_chance {
        return Ok(AttackOutcome {
            did_hit: false,
            damage_done: 0,
        });
    }

    let target_damage_taken_ratio = if target.total_armor < 0.0 {
        (100.0 - target.total_armor) / 100.0
    } else {
        100.0 / (100.0 + target.total_armor)
    };
    let mitigated_damage = (target_damage_taken_ratio * damage_roll).ceil();
    Ok(AttackOutcome {
        did_hit: true,
        damage_done: mitigated_damage.min(target.current_hitpoints) as u64,
    })
}

fn random_int(minimum: f64, maximum: f64, random: &mut BasicRandom) -> Result<f64> {
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
        return Err(unsupported("M2 supports exactly one player"));
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
                "M2 supports only the tier-0 /actions/combat/fly Zone",
            ));
        }
    }
    if request.options.statistics_mode != StatisticsMode::Full {
        return Err(unsupported("statisticsMode must be full"));
    }
    if request.options.enable_hp_mp_visualization {
        return Err(unsupported("HP/MP visualization is not supported in M2"));
    }
    if request.options.trace.enabled {
        return Err(unsupported("event trace is not supported in M2"));
    }
    if !is_empty_json_collection(&request.options.extra)
        || !request.options.extensions.is_empty()
        || !request.extensions.is_empty()
    {
        return Err(unsupported(
            "M2 does not accept extra or unknown request options",
        ));
    }
    Ok(())
}

fn unsupported(message: impl Into<String>) -> SimError {
    SimError::UnsupportedBasicCombat(message.into())
}

pub fn simulate_basic(request: &SimulationRequestV1) -> Result<BasicCombatResultV1> {
    BasicCombatEngine::new(request.clone())?.run()
}

pub fn simulate_basic_json(input: &str) -> Result<String> {
    let request = SimulationRequestV1::from_json(input)?;
    Ok(serde_json::to_string_pretty(&simulate_basic(&request)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUEST: &str = include_str!("../../../fixtures/parity/zone-solo-basic/request.json");
    const EXPECTED: &str =
        include_str!("../../../fixtures/parity/zone-solo-basic/expected-result.json");
    const METADATA: &str = include_str!("../../../fixtures/parity/zone-solo-basic/metadata.json");

    #[test]
    fn matches_the_reference_basic_result_subset() {
        let request = SimulationRequestV1::from_json(REQUEST).unwrap();
        let result = simulate_basic(&request).unwrap();
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
        assert_eq!(result.simulated_time.get(), expected["simulatedTime"]);
        assert_eq!(
            result.last_encounter_finish_time.get(),
            expected["lastEncounterFinishTime"]
        );
        assert_eq!(result.random_draws, metadata["randomDraws"]);
    }

    #[test]
    fn deterministic_replay_is_exact() {
        let request = SimulationRequestV1::from_json(REQUEST).unwrap();
        assert_eq!(
            simulate_basic(&request).unwrap(),
            simulate_basic(&request).unwrap()
        );
    }

    #[test]
    fn rejects_requests_outside_the_basic_capability() {
        let mut request = SimulationRequestV1::from_json(REQUEST).unwrap();
        request.players.push(request.players[0].clone());
        assert!(matches!(
            simulate_basic(&request),
            Err(SimError::UnsupportedBasicCombat(_))
        ));

        let mut request = SimulationRequestV1::from_json(REQUEST).unwrap();
        request.players[0]["abilities"] = serde_json::json!([{"hrid":"/abilities/smack"}]);
        assert!(matches!(
            simulate_basic(&request),
            Err(SimError::UnsupportedBasicCombat(_))
        ));
    }
}
