//! Example demonstrating the custom camera profile builder pattern.
//!
//! NOTE: This example uses an old API that no longer exists in the current version.
//! The CustomProfile, CustomProfileBuilder, and CustomProfileTypedBuilder types
//! have been removed. To create custom camera profiles now, you need to implement
//! the capability traits directly. See the profiles module for examples.

fn main() {
    eprintln!("This example uses an old API that no longer exists in the current version.");
    eprintln!("");
    eprintln!("The CustomProfile, CustomProfileBuilder, and CustomProfileTypedBuilder types");
    eprintln!("have been removed from the API.");
    eprintln!("");
    eprintln!("To create custom camera profiles now, you need to implement the capability");
    eprintln!("traits directly. See the src/camera/profiles.rs module for examples of how");
    eprintln!("to implement camera profiles like PTZOpticsG2, SonyFR7, etc.");
    eprintln!("");
    eprintln!("This example is kept for historical reference but needs to be rewritten");
    eprintln!("to use the new API.");
}