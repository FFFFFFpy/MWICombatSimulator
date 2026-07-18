use serde_json::Value;

use crate::{Result, SimError};

pub trait RandomSource {
    fn next_unit_f64(&mut self) -> Result<f64>;
    fn draw_count(&self) -> u64;
}

#[derive(Clone, Debug)]
pub struct SequenceRandom {
    values: Vec<f64>,
    index: usize,
    draw_count: u64,
    loop_values: bool,
}

impl SequenceRandom {
    pub fn new(values: Vec<f64>, loop_values: bool) -> Result<Self> {
        if values.is_empty() {
            return Err(SimError::EmptyRandomSequence);
        }
        for value in &values {
            validate_unit_value(*value)?;
        }
        Ok(Self {
            values,
            index: 0,
            draw_count: 0,
            loop_values,
        })
    }
}

impl RandomSource for SequenceRandom {
    fn next_unit_f64(&mut self) -> Result<f64> {
        if self.index >= self.values.len() {
            if !self.loop_values {
                return Err(SimError::RandomSequenceExhausted {
                    draws: self.draw_count,
                });
            }
            self.index = 0;
        }

        let value = self.values[self.index];
        self.index += 1;
        self.draw_count += 1;
        Ok(value)
    }

    fn draw_count(&self) -> u64 {
        self.draw_count
    }
}

#[derive(Clone, Debug)]
pub struct SeededRandom {
    state: u32,
    draw_count: u64,
}

impl SeededRandom {
    pub fn from_seed(seed: &Value) -> Result<Self> {
        let seed_text = match seed {
            Value::String(text) => text.clone(),
            value => serde_json::to_string(value)?,
        };
        Ok(Self::from_seed_text(&seed_text))
    }

    pub fn from_seed_text(seed: &str) -> Self {
        Self {
            state: hash_seed(seed),
            draw_count: 0,
        }
    }
}

impl RandomSource for SeededRandom {
    fn next_unit_f64(&mut self) -> Result<f64> {
        self.draw_count += 1;
        self.state = self.state.wrapping_add(0x6d2b_79f5);

        let mut value = self.state;
        value = (value ^ (value >> 15)).wrapping_mul(value | 1);
        value ^= value.wrapping_add((value ^ (value >> 7)).wrapping_mul(value | 61));
        let output = value ^ (value >> 14);

        Ok(f64::from(output) / 4_294_967_296.0)
    }

    fn draw_count(&self) -> u64 {
        self.draw_count
    }
}

fn hash_seed(seed: &str) -> u32 {
    let mut hash = 2_166_136_261_u32;
    for code_unit in seed.encode_utf16() {
        hash ^= u32::from(code_unit);
        hash = hash.wrapping_mul(16_777_619);
    }
    if hash == 0 { 0x6d2b_79f5 } else { hash }
}

fn validate_unit_value(value: f64) -> Result<()> {
    if !value.is_finite() || !(0.0..1.0).contains(&value) {
        return Err(SimError::InvalidRandomValue(value));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_rng_matches_reference_javascript_values() {
        let mut rng = SeededRandom::from_seed_text("fixture-a");
        let actual = (0..5)
            .map(|_| rng.next_unit_f64().unwrap())
            .collect::<Vec<_>>();
        let expected = vec![
            0.931_052_014_464_512_5,
            0.429_355_571_279_302_24,
            0.412_502_722_349_017_86,
            0.971_877_912_990_748_9,
            0.858_559_500_426_054,
        ];

        assert_eq!(actual, expected);
        assert_eq!(rng.draw_count(), 5);
    }

    #[test]
    fn seed_hash_uses_javascript_utf16_code_units() {
        let mut left = SeededRandom::from_seed_text("fixture-😀");
        let mut right = SeededRandom::from_seed(&Value::String("fixture-😀".into())).unwrap();
        assert_eq!(
            left.next_unit_f64().unwrap(),
            right.next_unit_f64().unwrap()
        );
    }

    #[test]
    fn finite_sequence_exhausts_without_incrementing_the_draw_count() {
        let mut rng = SequenceRandom::new(vec![0.1, 0.2], false).unwrap();
        assert_eq!(rng.next_unit_f64().unwrap(), 0.1);
        assert_eq!(rng.next_unit_f64().unwrap(), 0.2);
        assert!(matches!(
            rng.next_unit_f64(),
            Err(SimError::RandomSequenceExhausted { draws: 2 })
        ));
        assert_eq!(rng.draw_count(), 2);
    }

    #[test]
    fn looping_sequence_restarts_in_the_same_order() {
        let mut rng = SequenceRandom::new(vec![0.3, 0.4], true).unwrap();
        assert_eq!(rng.next_unit_f64().unwrap(), 0.3);
        assert_eq!(rng.next_unit_f64().unwrap(), 0.4);
        assert_eq!(rng.next_unit_f64().unwrap(), 0.3);
        assert_eq!(rng.draw_count(), 3);
    }
}
