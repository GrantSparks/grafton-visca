//! Demonstration of compile-time safety in the new Camera API.
//!
//! This example shows how methods only exist for cameras that support
//! the corresponding capabilities, preventing runtime errors.

// This example demonstrates compile-time safety in the Camera API
#![cfg(feature = "compile_time_safety_demo")]

use grafton_visca::{
    camera::{methods::*, Camera},
    profiles::{GenericVisca, PTZOpticsG2, SonyFR7},
    transport::blocking::create,
    Error,
};

fn main() -> Result<(), Error> {
    // Mock transport for demonstration
    struct MockTransport;

    impl grafton_visca::transport::blocking::BlockingTransport for MockTransport {
        fn send(&mut self, _data: &[u8]) -> Result<(), Error> {
            Ok(())
        }
        fn receive(&mut self) -> Result<Vec<u8>, Error> {
            Ok(vec![0x90, 0x50, 0xFF]) // Mock completion response
        }
    }

    demonstrate_ptzoptics_g2()?;
    demonstrate_sony_fr7()?;
    demonstrate_generic_camera()?;
    demonstrate_compile_time_errors();

    Ok(())
}

fn demonstrate_ptzoptics_g2() -> Result<(), Error> {
    println!("=== PTZOptics G2 Demo ===");

    let mut camera: Camera<PTZOpticsG2, _> = Camera::new(MockTransport);

    // ✅ These methods exist - G2 supports these capabilities
    camera.power_on()?;
    camera.pan_tilt_home()?;
    camera.pan_tilt_absolute(45.0, 30.0, 10)?;
    camera.zoom_stop()?;
    camera.focus_auto()?;
    camera.exposure_auto()?;
    camera.white_balance_auto()?;
    camera.enable_flip()?;
    camera.preset_recall(1)?;

    // ❌ This would NOT compile - G2 doesn't support ND filters!
    // camera.set_nd_filter(2)?;  // COMPILE ERROR!

    println!("All G2 operations completed successfully");
    Ok(())
}

fn demonstrate_sony_fr7() -> Result<(), Error> {
    println!("\n=== Sony FR7 Demo ===");

    let mut camera: Camera<SonyFR7, _> = Camera::new(MockTransport);

    // ✅ FR7 has all standard features
    camera.power_on()?;
    camera.pan_tilt_home()?;
    camera.zoom_stop()?;

    // ✅ PLUS ND filter support!
    camera.set_nd_filter(128)?; // This compiles!
    let nd_level = camera.get_nd_filter()?;
    println!("Current ND filter level: {}", nd_level);

    println!("All FR7 operations completed successfully");
    Ok(())
}

fn demonstrate_generic_camera() -> Result<(), Error> {
    println!("\n=== Generic VISCA Demo ===");

    let mut camera: Camera<GenericVisca, _> = Camera::new(MockTransport);

    // ✅ Generic camera only has basic features
    camera.power_on()?;
    camera.pan_tilt_home()?;
    camera.zoom_stop()?;

    // ❌ These would NOT compile - Generic doesn't support these
    // camera.focus_auto()?;        // COMPILE ERROR!
    // camera.exposure_auto()?;     // COMPILE ERROR!
    // camera.white_balance_auto()?; // COMPILE ERROR!
    // camera.set_nd_filter(1)?;    // COMPILE ERROR!

    println!("Generic camera basic operations completed");
    Ok(())
}

fn demonstrate_compile_time_errors() {
    println!("\n=== Compile-Time Error Examples ===");

    // These examples show what WOULD happen if you uncommented them:

    /*
    // Example 1: PTZOptics G2 doesn't have ND filter
    let mut g2_camera: Camera<PTZOpticsG2, MockTransport> = Camera::new(MockTransport);
    g2_camera.set_nd_filter(2)?;

    // Compile error:
    // error[E0599]: no method named `set_nd_filter` found for struct `Camera<PTZOpticsG2, MockTransport>`
    // note: the method `set_nd_filter` exists but the trait bound `PTZOpticsG2: SupportsNDFilter` is not satisfied
    */

    /*
    // Example 2: Generic camera doesn't have focus control
    let mut generic: Camera<GenericVisca, MockTransport> = Camera::new(MockTransport);
    generic.focus_auto()?;

    // Compile error:
    // error[E0599]: no method named `focus_auto` found for struct `Camera<GenericVisca, MockTransport>`
    // note: the method `focus_auto` exists but the trait bound `GenericVisca: SupportsFocus` is not satisfied
    */

    println!("Compile-time safety prevents calling unsupported methods!");
}

// Generic function that requires specific capabilities
fn center_and_focus<P, T>(camera: &mut Camera<P, T>) -> Result<(), Error>
where
    P: grafton_visca::capabilities::ProfileMetadata
        + grafton_visca::capabilities::SupportsPanTilt
        + grafton_visca::capabilities::SupportsFocus,
    T: grafton_visca::transport::blocking::BlockingTransport,
{
    camera.pan_tilt_home()?;
    camera.focus_auto()?;
    Ok(())
}

// This function can only be called with cameras that have ND filters
fn set_neutral_exposure<P, T>(camera: &mut Camera<P, T>) -> Result<(), Error>
where
    P: grafton_visca::capabilities::ProfileMetadata
        + grafton_visca::capabilities::SupportsNDFilter
        + grafton_visca::capabilities::SupportsExposure,
    T: grafton_visca::transport::blocking::BlockingTransport,
{
    camera.set_nd_filter(1)?;
    camera.exposure_auto()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_time_trait_bounds() {
        let mut g2 = Camera::<PTZOpticsG2, MockTransport>::new(MockTransport);
        let mut fr7 = Camera::<SonyFR7, MockTransport>::new(MockTransport);

        // ✅ Both cameras can use center_and_focus
        center_and_focus(&mut g2).unwrap();
        center_and_focus(&mut fr7).unwrap();

        // ❌ Only FR7 can use set_neutral_exposure
        // set_neutral_exposure(&mut g2).unwrap(); // COMPILE ERROR!
        set_neutral_exposure(&mut fr7).unwrap(); // OK!
    }
}
