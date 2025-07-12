//! Example program

//! Test example for declarative macro approach

// Allow missing docs for macro-generated code
#![allow(missing_docs)]

use grafton_visca::{command::encode_visca::EncodeVisca, visca_command, Error};

visca_command! {
    category = "Movement",
    enum TestCommands {
        Home => [0x81, 0x01, 0x06, 0x04, 0xFF],
        Reset => [0x81, 0x01, 0x06, 0x05, 0xFF],
    }
}

visca_command! {
    category = "Quick",
    enum PowerCommands {
        On => [0x81, 0x01, 0x04, 0x00, 0x02, 0xFF],
        Standby => [0x81, 0x01, 0x04, 0x00, 0x03, 0xFF],
    }
}

fn main() -> Result<(), Error> {
    env_logger::init();

    println!("Testing declarative macro approach...");

    let home = TestCommands::Home;
    let power_on = PowerCommands::On;

    println!("Home bytes: {:02X?}", home.try_into_vec()?);
    println!("Power On bytes: {:02X?}", power_on.try_into_vec()?);
    println!("Home category: {:?}", home.timeout_kind());
    println!("Power category: {:?}", power_on.timeout_kind());

    // Verify
    assert_eq!(home.try_into_vec()?, vec![0x81, 0x01, 0x06, 0x04, 0xFF]);
    assert_eq!(
        power_on.try_into_vec()?,
        vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]
    );

    println!("✅ Declarative macro tests passed!");

    Ok(())
}
