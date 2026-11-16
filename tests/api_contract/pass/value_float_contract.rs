use grafton_visca::{value, wire::WireValue, Error};

#[value(min = -10.0, max = 10.0, display = "Angle {}")]
struct Angle(f32);

#[value(min = -1.0, max = 1.0)]
struct Fractional(f64);

const CHECKED: Result<Angle, Error> = Angle::new(5.0);

fn full_traits<T: Copy + Clone + std::fmt::Debug + Eq + Ord + std::hash::Hash>() {}

fn main() -> Result<(), Error> {
    full_traits::<Angle>();
    full_traits::<Fractional>();
    let positive_zero = Angle::new(0.0)?;
    let negative_zero = Angle::new(-0.0)?;
    assert_eq!(positive_zero, negative_zero);
    assert_eq!(positive_zero.cmp(&negative_zero), std::cmp::Ordering::Equal);
    assert_eq!(
        std::collections::HashSet::from([positive_zero, negative_zero]).len(),
        1
    );
    assert_eq!(Angle::MIN.value(), -10.0);
    assert_eq!(Angle::MAX.value(), 10.0);
    assert_eq!(CHECKED?.value(), 5.0);
    assert_eq!(Angle::from_wire(2.5)?.to_wire(), 2.5);
    assert_eq!(Angle::new(2.5)?.to_string(), "Angle 2.5");
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -10.1, 10.1] {
        assert!(Angle::new(invalid).is_err());
        assert!(Angle::from_wire(invalid).is_err());
    }
    assert_eq!(Fractional::new(0.25)?.value(), 0.25);
    assert!(Fractional::new(f64::NAN).is_err());
    Ok(())
}
