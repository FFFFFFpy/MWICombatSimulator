use std::cmp::Ordering;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

use crate::{Result, SimError};

#[derive(Clone, Copy, Debug, Default)]
pub struct SimTime(f64);

impl SimTime {
    pub const ZERO: Self = Self(0.0);

    pub fn new(value: f64) -> Result<Self> {
        if !value.is_finite() || value < 0.0 {
            return Err(SimError::InvalidTime(value));
        }
        Ok(Self(if value == 0.0 { 0.0 } else { value }))
    }

    pub const fn get(self) -> f64 {
        self.0
    }
}

impl PartialEq for SimTime {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for SimTime {}

impl PartialOrd for SimTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SimTime {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl Serialize for SimTime {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(self.0)
    }
}

impl<'de> Deserialize<'de> for SimTime {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_times_and_normalizes_negative_zero() {
        assert!(matches!(
            SimTime::new(f64::NAN),
            Err(SimError::InvalidTime(_))
        ));
        assert!(matches!(
            SimTime::new(-1.0),
            Err(SimError::InvalidTime(-1.0))
        ));
        assert_eq!(SimTime::new(-0.0).unwrap(), SimTime::ZERO);
    }

    #[test]
    fn preserves_fractional_nanoseconds_and_total_ordering() {
        let earlier = SimTime::new(3_000_000_000.25).unwrap();
        let later = SimTime::new(3_000_000_000.5).unwrap();
        assert!(earlier < later);

        let encoded = serde_json::to_string(&earlier).unwrap();
        assert_eq!(serde_json::from_str::<SimTime>(&encoded).unwrap(), earlier);
    }
}
