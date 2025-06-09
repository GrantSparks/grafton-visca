//! Test example for declarative macro approach

use grafton_visca::command::Command;
use grafton_visca::{visca_command, Error};

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

    println!("Home bytes: {:02X?}", home.to_bytes()?);
    println!("Power On bytes: {:02X?}", power_on.to_bytes()?);
    println!("Home category: {:?}", home.command_category());
    println!("Power category: {:?}", power_on.command_category());

    // Verify
    assert_eq!(home.to_bytes()?, vec![0x81, 0x01, 0x06, 0x04, 0xFF]);
    assert_eq!(
        power_on.to_bytes()?,
        vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]
    );

    println!("✅ Declarative macro tests passed!");

    Ok(())
}
