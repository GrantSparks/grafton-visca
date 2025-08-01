//! Example demonstrating the CameraBuilder API for async transports with tokio
//!
//! This example shows how to use the builder pattern to create cameras
//! with different profiles and transports in async mode using tokio.

use grafton_visca::{
    camera::profiles::{GenericVisca, PTZOpticsG2, SonyBRC300},
    CameraBuilder, Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // Example 1: Creating an async TCP camera with PTZOpticsG2 profile
    println!("=== Example 1: Async TCP with PTZOpticsG2 profile ===");
    let tcp_camera = CameraBuilder::tokio_tcp("192.168.1.100:52381")
        .profile::<PTZOpticsG2>()
        .build()
        .await;

    match tcp_camera {
        Ok(_camera) => {
            println!("Successfully created async TCP camera with PTZOpticsG2 profile");
            // You can now use the camera with async methods
            // For example: camera.pan_tilt_home().await?;
        }
        Err(e) => {
            println!("Failed to create async TCP camera: {e}");
        }
    }

    // Example 2: Creating an async UDP camera with GenericVisca profile
    println!("\n=== Example 2: Async UDP with GenericVisca profile ===");
    let udp_camera = CameraBuilder::tokio_udp("239.0.0.1:52381")
        .profile::<GenericVisca>()
        .build()
        .await;

    match udp_camera {
        Ok(_camera) => {
            println!("Successfully created async UDP camera with GenericVisca profile");
            // All camera methods are async when using tokio transports
        }
        Err(e) => {
            println!("Failed to create async UDP camera: {e}");
        }
    }

    // Example 3: Using different camera profiles
    println!("\n=== Example 3: Different Camera Profiles ===");

    // The builder supports any type that implements the Profile trait
    let sony_camera = CameraBuilder::tokio_tcp("192.168.1.101:52381")
        .profile::<SonyBRC300>()
        .build()
        .await;

    match sony_camera {
        Ok(_camera) => {
            println!("Successfully created camera with SonyBRC300 profile");
            // The SonyBRC300 profile provides Sony-specific capabilities
        }
        Err(e) => {
            println!("Failed to create Sony camera: {e}");
        }
    }

    // Example 4: Concurrent camera creation
    println!("\n=== Example 4: Concurrent Camera Creation ===");

    use tokio::join;

    // Create multiple cameras concurrently
    let (cam1_result, cam2_result, cam3_result) = join!(
        CameraBuilder::tokio_tcp("192.168.1.100:52381")
            .profile::<PTZOpticsG2>()
            .build(),
        CameraBuilder::tokio_tcp("192.168.1.101:52381")
            .profile::<PTZOpticsG2>()
            .build(),
        CameraBuilder::tokio_tcp("192.168.1.102:52381")
            .profile::<PTZOpticsG2>()
            .build()
    );

    let created_count = [&cam1_result, &cam2_result, &cam3_result]
        .iter()
        .filter(|r| r.is_ok())
        .count();

    println!("Successfully created {created_count} out of 3 cameras concurrently");

    // Example 5: Type safety with async builders
    println!("\n=== Example 5: Type Safety in Async Context ===");

    // The same compile-time guarantees apply to async builders:
    // 1. Must call .profile() before .build()
    // 2. Cannot call .profile() twice
    // 3. Profile type must implement the Profile trait

    // Additionally, .build() returns a Future that must be awaited
    // This would not compile without .await:
    // let camera = CameraBuilder::tokio_tcp("192.168.1.100:52381")
    //     .profile::<PTZOpticsG2>()
    //     .build();  // Error: unused implementer of `Future`

    println!("Async builders maintain the same type safety guarantees!");

    Ok(())
}
