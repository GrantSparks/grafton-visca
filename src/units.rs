//! Checked physical quantities used by the public camera API.

use std::{borrow::Cow, cmp::Ordering, fmt, str::FromStr};

use crate::{error::Error, Result};

/// A finite angle in degrees.
///
/// Positive pan is right and positive tilt is up. Construction checks only
/// finiteness; the target profile checks its physical range during preparation.
/// Signed zero is stored as positive zero, so equality, ordering and hashing
/// all use the same value convention.
#[crate::value(min = f32::MIN, max = f32::MAX, display = "{}°")]
pub struct Degrees(f32);

/// A checked normalized value in `0.0..=1.0`.
///
/// The selected target domain supplies the range used to lower this value to
/// camera units. Construction rejects values outside the interval and all
/// non-finite values, and normalizes signed zero.
#[crate::value(min = 0.0, max = 1.0)]
pub struct UnitInterval(f32);

/// Color temperature in Kelvin over the union of documented camera domains.
///
/// The target profile validates its own range and lowers Kelvin to its
/// documented wire encoding; the value itself carries no vendor-specific code.
#[crate::value(min = 2000, max = 15000, display = "{} K")]
pub struct Kelvin(u16);

/// Exposure time in seconds, stored as a reduced nonnegative rational value.
///
/// A nonzero denominator and lowest terms make equality and hashing value
/// based: `1/60 == 2/120`. Ordering compares exposure times numerically.
/// `Display`, `FromStr` and serde use `numerator/denominator` seconds.
/// A profile's shutter table maps the physical time to its camera-specific code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "String", into = "String")
)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export, type = "string"))]
pub struct ExposureTime {
    numerator: u32,
    denominator: u32,
}

impl ExposureTime {
    /// The shortest representable exposure time, zero seconds.
    pub const MIN: Self = Self {
        numerator: 0,
        denominator: 1,
    };

    /// The longest representable exposure time, `u32::MAX` seconds.
    pub const MAX: Self = Self {
        numerator: u32::MAX,
        denominator: 1,
    };

    /// Constructs the reciprocal literals used by compile-time shutter tables.
    ///
    /// The public constructor returns the crate's owning error type; extracting
    /// that `Result` in a constant would require const destruction of `Error`.
    pub(crate) const fn reciprocal(denominator: u32) -> Self {
        assert!(
            denominator != 0,
            "exposure time denominator must be nonzero"
        );
        Self {
            numerator: 1,
            denominator,
        }
    }

    /// Creates an exposure time in lowest terms, rejecting a zero denominator.
    pub const fn new(numerator: u32, denominator: u32) -> Result<Self> {
        if denominator == 0 {
            return Err(Error::InvalidParameter {
                parameter: "exposure_time",
                value: Cow::Borrowed("zero denominator"),
                reason: Cow::Borrowed("exposure time must have a nonzero denominator"),
            });
        }
        let (mut a, mut b) = (numerator, denominator);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        Ok(Self {
            numerator: numerator / a,
            denominator: denominator / a,
        })
    }

    /// Returns the numerator in lowest terms.
    #[must_use]
    pub const fn numerator(self) -> u32 {
        self.numerator
    }

    /// Returns the nonzero denominator in lowest terms.
    #[must_use]
    pub const fn denominator(self) -> u32 {
        self.denominator
    }

    /// Returns the exposure time in decimal seconds.
    #[must_use]
    pub fn as_decimal(self) -> f64 {
        f64::from(self.numerator) / f64::from(self.denominator)
    }
}

impl PartialOrd for ExposureTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ExposureTime {
    fn cmp(&self, other: &Self) -> Ordering {
        (u64::from(self.numerator) * u64::from(other.denominator))
            .cmp(&(u64::from(other.numerator) * u64::from(self.denominator)))
    }
}

impl fmt::Display for ExposureTime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.numerator, self.denominator)
    }
}

impl FromStr for ExposureTime {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        let invalid = || Error::InvalidParameter {
            parameter: "exposure_time",
            value: Cow::Owned(text.to_owned()),
            reason: Cow::Borrowed("expected `numerator/denominator` with a nonzero denominator"),
        };
        let (numerator, denominator) = text.split_once('/').ok_or_else(invalid)?;
        let numerator = numerator.trim().parse().map_err(|_| invalid())?;
        let denominator = denominator.trim().parse().map_err(|_| invalid())?;
        Self::new(numerator, denominator).map_err(|_| invalid())
    }
}

impl TryFrom<String> for ExposureTime {
    type Error = Error;

    fn try_from(text: String) -> Result<Self> {
        text.parse()
    }
}

impl From<ExposureTime> for String {
    fn from(value: ExposureTime) -> Self {
        value.to_string()
    }
}

#[cfg(feature = "schemars")]
impl schemars::JsonSchema for ExposureTime {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("ExposureTime")
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": "^[0-9]+/[0-9]*[1-9][0-9]*$",
            "description": "Exposure time as `numerator/denominator` seconds, such as `1/60`.",
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::wire::WireValue;
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };

    use super::*;

    fn hash(value: impl Hash) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[crate::value(min = -1.0, max = 1.0)]
    struct FiniteDouble(f64);

    #[crate::value(min = -0.0, max = 0.0)]
    struct Zero(f32);

    #[test]
    fn angles_are_finite_and_have_consistent_value_ordering() -> Result<()> {
        const RIGHT: Result<Degrees> = Degrees::new(90.0);
        assert_eq!(RIGHT?.value(), 90.0);
        assert!(Degrees::MIN < Degrees::new(-1.0)?);
        assert!(Degrees::new(-1.0)? < Degrees::new(1.0)?);
        assert!(Degrees::new(1.0)? < Degrees::MAX);
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(Degrees::new(value).is_err());
            assert!(Degrees::from_wire(value).is_err());
        }
        let negative_zero = Degrees::new(-0.0)?;
        let positive_zero = Degrees::new(0.0)?;
        assert_eq!(negative_zero.value().to_bits(), 0);
        assert_eq!(negative_zero, positive_zero);
        assert_eq!(negative_zero.cmp(&positive_zero), Ordering::Equal);
        assert_eq!(hash(negative_zero), hash(positive_zero));
        Ok(())
    }

    #[test]
    fn normalized_values_enforce_bounds_and_canonicalize_zero() -> Result<()> {
        const HALF: Result<UnitInterval> = UnitInterval::new(0.5);
        let half = HALF?;
        assert!(UnitInterval::MIN < half);
        assert!(half < UnitInterval::MAX);
        for value in [-0.1, 1.1, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(UnitInterval::new(value).is_err());
            assert!(UnitInterval::from_wire(value).is_err());
        }
        let negative_zero = UnitInterval::new(-0.0)?;
        assert_eq!(negative_zero.value().to_bits(), 0);
        assert_eq!(negative_zero, UnitInterval::MIN);
        assert_eq!(negative_zero.cmp(&UnitInterval::MIN), Ordering::Equal);
        assert_eq!(hash(negative_zero), hash(UnitInterval::MIN));
        Ok(())
    }

    #[test]
    fn floating_generator_supports_f64_and_normalizes_zero_bounds() -> Result<()> {
        const HALF: Result<FiniteDouble> = FiniteDouble::new(0.5);
        assert_eq!(HALF?.value(), 0.5);
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.1, 1.1] {
            assert!(FiniteDouble::new(value).is_err());
            assert!(FiniteDouble::from_wire(value).is_err());
        }
        let negative_zero = FiniteDouble::new(-0.0)?;
        let positive_zero = FiniteDouble::new(0.0)?;
        assert_eq!(negative_zero.value().to_bits(), 0);
        assert_eq!(negative_zero, positive_zero);
        assert_eq!(negative_zero.cmp(&positive_zero), Ordering::Equal);
        assert_eq!(hash(negative_zero), hash(positive_zero));
        assert_eq!(Zero::MIN.value().to_bits(), 0);
        assert_eq!(Zero::MAX.value().to_bits(), 0);
        Ok(())
    }

    #[cfg(feature = "serde")]
    #[test]
    fn floating_generator_rejects_nonfinite_serde_input() -> Result<()> {
        use serde::Deserialize;

        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let input = || serde::de::value::F32Deserializer::<serde::de::value::Error>::new(value);
            assert!(Degrees::deserialize(input()).is_err());
            assert!(UnitInterval::deserialize(input()).is_err());
        }
        let negative_zero = serde::de::value::F32Deserializer::<serde::de::value::Error>::new(-0.0);
        let normalized = UnitInterval::deserialize(negative_zero)
            .map_err(|_| Error::invalid_parameter("unit_interval", "-0.0", "valid finite zero"))?;
        assert_eq!(normalized.value().to_bits(), 0);
        Ok(())
    }

    #[cfg(feature = "schemars")]
    #[test]
    fn floating_generator_keeps_schema_bounds() {
        let schema = schemars::schema_for!(UnitInterval);
        assert_eq!(schema.get("minimum"), Some(&serde_json::json!(0.0)));
        assert_eq!(schema.get("maximum"), Some(&serde_json::json!(1.0)));
    }

    #[test]
    fn exposure_times_reduce_and_order_as_physical_values() -> Result<()> {
        let sixtieth = ExposureTime::new(1, 60)?;
        let equivalent = ExposureTime::new(2, 120)?;
        assert_eq!(sixtieth, equivalent);
        assert_eq!(hash(sixtieth), hash(equivalent));
        assert_eq!(sixtieth.to_string(), "1/60");
        assert_eq!("2/120".parse::<ExposureTime>()?, sixtieth);
        assert!(sixtieth < ExposureTime::new(1, 50)?);
        assert_eq!(ExposureTime::reciprocal(60), sixtieth);
        assert_eq!(ExposureTime::new(0, 7)?, ExposureTime::MIN);
        assert!(ExposureTime::new(u32::MAX - 1, u32::MAX)? < ExposureTime::new(1, 1)?);
        assert!(ExposureTime::new(u32::MAX, u32::MAX - 1)? > ExposureTime::new(1, 1)?);
        assert!(ExposureTime::new(1, 0).is_err());
        for invalid in ["", "1", "1/0", "a/60", "1/60/2", "-1/60"] {
            assert!(
                invalid.parse::<ExposureTime>().is_err(),
                "accepted {invalid:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn kelvin_construction_validates_union_domain_only() -> Result<()> {
        for value in [2000, 2500, 3200, 8000, 15000] {
            assert_eq!(Kelvin::new(value)?.value(), value);
        }
        for value in [0, 1999, 15001, u16::MAX] {
            assert!(Kelvin::new(value).is_err());
        }
        assert_eq!(Kelvin::MIN.value(), 2000);
        assert_eq!(Kelvin::MAX.value(), 15000);
        Ok(())
    }

    #[cfg(feature = "serde")]
    #[test]
    fn physical_values_serde_validates_and_preserves_their_units(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let angle = Degrees::new(45.0)?;
        assert_eq!(serde_json::to_value(angle)?, 45.0);
        assert_eq!(
            serde_json::from_value::<Degrees>(serde_json::json!(-0.0))?
                .value()
                .to_bits(),
            0
        );
        for value in [-0.1, 1.1] {
            assert!(serde_json::from_value::<UnitInterval>(serde_json::json!(value)).is_err());
        }
        assert!(serde_json::from_value::<Kelvin>(serde_json::json!(15001)).is_err());
        let time = ExposureTime::new(1, 60)?;
        assert_eq!(serde_json::to_value(time)?, "1/60");
        assert_eq!(
            serde_json::from_value::<ExposureTime>(serde_json::json!("2/120"))?,
            time
        );
        assert!(serde_json::from_value::<ExposureTime>(serde_json::json!("1/0")).is_err());
        Ok(())
    }
}
