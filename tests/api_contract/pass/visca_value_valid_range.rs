use grafton_visca::value;

#[value(min = 1, max = 4, display = "Value {:#04x}")]
struct ValidRange(u8);

#[value(valid_values = [0x01, 0x03])]
struct ValidSet(u8);

fn main() {
    let value = ValidRange::new(2).expect("range-bound value compiles downstream");
    let raw = u8::from(value);
    let round_trip = ValidRange::try_from(raw).expect("TryFrom compiles downstream");
    assert_eq!(round_trip, value);
    assert_eq!(ValidRange::MIN.value(), 1);
    assert_eq!(ValidRange::MAX.value(), 4);
    assert_eq!(format!("{value}"), "Value 0x02");

    assert_eq!(ValidSet::MIN.value(), 1);
    assert_eq!(ValidSet::MAX.value(), 3);
    assert!(ValidSet::new(1).is_ok());
    assert!(ValidSet::new(2).is_err());
}
