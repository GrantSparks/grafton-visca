//! Example showing how the library uses declarative macros internally
//!
//! This demonstrates how the grafton-visca library implements commands internally
//! using declarative macros, though users typically won't need to use these directly.

fn main() {
    println!("The grafton-visca library uses declarative macros internally to generate");
    println!("VISCA command implementations. This reduces boilerplate and ensures consistency.");
    println!();
    println!("For example, internally the library might define commands like:");
    println!();
    println!("visca_command! {{");
    println!("    category = \"Movement\",");
    println!("    enum TestCommands {{");
    println!("        Home => {{ Ok(vec![0x81, 0x01, 0x06, 0x04, 0xFF]) }},");
    println!("        Reset => {{ Ok(vec![0x81, 0x01, 0x06, 0x05, 0xFF]) }},");
    println!("    }}");
    println!("}}");
    println!();
    println!("But as a user, you simply call high-level methods on the Camera:");
    println!();
    println!("camera.pan_tilt_home()?;");
    println!("camera.reset()?;");
    println!();
    println!("The library handles all the protocol details internally!");
}