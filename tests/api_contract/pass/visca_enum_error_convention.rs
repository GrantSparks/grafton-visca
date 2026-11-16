use grafton_visca::{value_enum, wire::WireValue, Error, Result};

#[value_enum]
enum Closed {
    Valid = 1,
}

#[value_enum]
enum Extensible {
    Valid = 1,
    Unknown(u8),
}

fn main() -> Result<()> {
    let closed: Result<Closed> = Closed::from_wire(1);
    assert_eq!(closed?, Closed::Valid);
    let error: Error = Closed::from_wire(2).expect_err("closed enum rejects unknown code");
    assert!(
        matches!(error, Error::InvalidParameter { parameter: "Closed", value, reason, .. }
        if value == "2" && reason == "unknown wire value")
    );
    assert_eq!(Extensible::from_wire(2)?, Extensible::Unknown(2));
    assert_eq!(Extensible::Unknown(2).to_wire(), 2);
    Ok(())
}
