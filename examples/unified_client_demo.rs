//! Demo of the unified Camera API in both blocking and async contexts.
//!
//! This example shows how the Camera API works with compile-time profiles
//! providing type safety and zero runtime overhead.

use grafton_visca::{
    capabilities::{NDFilter, Profile},
    transport::UnifiedTransport,
    Camera, Error, NDFilterMode,
};

#[cfg(not(feature = "tokio"))]
use grafton_visca::transport::blocking::{Tcp, Udp};

#[cfg(feature = "tokio")]
use grafton_visca::transport::tokio::{Tcp, Udp};

// ==================== BLOCKING EXAMPLES ====================

#[cfg(not(feature = "tokio"))]
fn blocking_examples() -> Result<(), Error> {
    use grafton_visca::prelude::blocking::*;

    println!("=== Unified Camera with Blocking Transport ===\n");

    // Example 1: Create camera with GenericVisca profile
    println!("Example 1: GenericVisca profile with UDP");
    let udp_transport = Udp::connect("192.168.1.100:52381")?;
    let camera = GenericViscaCam::new(udp_transport);
    println!("Created GenericVisca camera");

    // Basic operations available on all cameras
    camera.power_on()?;
    camera.zoom_stop()?;

    // Example 2: Create camera with PTZOptics G2 profile
    println!("\nExample 2: PTZOptics G2 profile with TCP");
    let tcp_transport = Tcp::connect("192.168.1.100:5678")?;
    let camera = PTZOpticsG2Cam::new(tcp_transport);
    println!("Created PTZOptics G2 camera");

    // PTZOptics G2 specific operations (compile-time checked)
    camera.power_on()?;
    camera.preset_recall(PresetNumber::new(1).unwrap())?;

    // Example 3: Create Sony FR7 camera
    println!("\nExample 3: Sony FR7 profile");
    let transport = Udp::connect("192.168.1.200:52381")?;
    let camera = SonyFR7Cam::new(transport);
    println!("Created Sony FR7 camera");

    // Sony FR7 supports advanced features
    camera.set_nd_filter_mode(NDFilterMode::Preset)?;

    // Example 4: Compile-time type safety
    println!("\nExample 4: Compile-time type safety");
    println!("Each camera type has access to only its supported features.");
    println!("Attempting to call unsupported methods results in compile errors.");

    // This would not compile if uncommented:
    // let generic_cam = GenericViscaCam::new(udp_transport);
    // generic_cam.set_nd_filter_mode(NDFilterMode::Clear)?; // Error: GenericVisca doesn't support ND filter

    Ok(())
}

// ==================== ASYNC EXAMPLES ====================

#[cfg(feature = "tokio")]
async fn async_examples() -> Result<(), Error> {
    use grafton_visca::prelude::r#async::*;
    println!("=== Unified Camera with Async Transport ===\n");

    // Example 1: Create camera with GenericVisca profile
    println!("Example 1: GenericVisca profile with UDP");
    let udp_transport = Udp::connect("192.168.1.100:52381").await?;
    let camera = GenericViscaCam::new(udp_transport);
    println!("Created GenericVisca camera");

    // Basic operations available on all cameras
    {
        // PowerOps and ZoomOps already imported from prelude
        camera.power_on().await?;
        camera.zoom_stop().await?;
    }

    // Example 2: Create camera with PTZOptics G2 profile
    println!("\nExample 2: PTZOptics G2 profile with TCP");
    let tcp_transport = Tcp::connect("192.168.1.100:5678").await?;
    let camera = PTZOpticsG2Cam::new(tcp_transport);
    println!("Created PTZOptics G2 camera");

    // PTZOptics G2 specific operations
    {
        // PowerOps and PresetsOps already imported from prelude
        camera.power_on().await?;
        camera.preset_recall(PresetNumber::new(1).unwrap()).await?;
    }

    // Example 3: Create Sony FR7 camera
    println!("\nExample 3: Sony FR7 profile");
    let transport = Udp::connect("192.168.1.200:52381").await?;
    let camera = SonyFR7Cam::new(transport);
    println!("Created Sony FR7 camera");

    // Sony FR7 supports advanced features
    {
        // NDFilterOps already imported from prelude
        camera.set_nd_filter_mode(NDFilterMode::Preset).await?;
    }

    // Example 4: Compile-time type safety
    println!("\nExample 4: Compile-time type safety");
    println!("Each camera type has access to only its supported features.");
    println!("Attempting to call unsupported methods results in compile errors.");

    Ok(())
}

// ==================== ADVANCED EXAMPLE ====================

fn advanced_example() {
    println!("=== Advanced: Working with Generic Functions ===\n");

    // NOTE: These are example function signatures showing how to write generic functions.
    // In practice, you would use either blocking or async versions.

    #[cfg(not(feature = "tokio"))]
    {
        // Blocking version - You can write generic functions that work with any camera profile
        fn _operate_any_camera<P, T>(camera: &Camera<P, T>) -> Result<(), Error>
        where
            P: Profile,
            T: UnifiedTransport,
        {
            use grafton_visca::blocking::{PanTiltOps, PowerOps, ZoomOps};
            // All cameras support basic operations
            camera.power_on()?;
            camera.zoom_stop()?;
            camera.pan_tilt_home()?;
            Ok(())
        }

        // Or functions that require specific capabilities
        fn _operate_nd_filter_camera<P, T>(camera: &Camera<P, T>) -> Result<(), Error>
        where
            P: Profile + NDFilter,
            T: UnifiedTransport,
        {
            use grafton_visca::blocking::NDFilterOps;
            // This function can only be called with cameras that support ND filter
            camera.set_nd_filter_mode(NDFilterMode::Preset)?;
            Ok(())
        }
    }

    #[cfg(feature = "tokio")]
    {
        // Async version - You can write generic functions that work with any camera profile
        async fn _operate_any_camera<P, T>(camera: &Camera<P, T>) -> Result<(), Error>
        where
            P: Profile,
            T: UnifiedTransport,
        {
            use grafton_visca::camera::methods::{PanTiltOps, PowerOps, ZoomOps};
            // All cameras support basic operations
            camera.power_on().await?;
            camera.zoom_stop().await?;
            camera.pan_tilt_home().await?;
            Ok(())
        }

        // Or functions that require specific capabilities
        async fn _operate_nd_filter_camera<P, T>(camera: &Camera<P, T>) -> Result<(), Error>
        where
            P: Profile + NDFilter,
            T: UnifiedTransport,
        {
            // This function can only be called with cameras that support ND filter
            camera.set_nd_filter_mode(NDFilterMode::Preset).await?;
            Ok(())
        }
    }

    println!("Generic functions provide flexibility while maintaining type safety.");
}

// ==================== MAIN ====================

#[cfg(not(feature = "tokio"))]
fn main() -> Result<(), Error> {
    env_logger::init();

    blocking_examples()?;
    advanced_example();

    println!("\n✅ All examples completed successfully!");
    Ok(())
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    async_examples().await?;
    advanced_example();

    println!("\n✅ All examples completed successfully!");
    Ok(())
}
