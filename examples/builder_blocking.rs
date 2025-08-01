//! Example demonstrating the CameraBuilder API for blocking transports
//!
//! This example shows how to use the builder pattern to create cameras
//! with different profiles and transports in blocking mode.

use grafton_visca::{
    camera::profiles::{GenericVisca, PTZOpticsG2},
    CameraBuilder, Result,
};

fn main() -> Result<()> {
    env_logger::init();

    // Example 1: Creating a TCP camera with PTZOpticsG2 profile
    println!("=== Example 1: TCP with PTZOpticsG2 profile ===");
    let tcp_camera = CameraBuilder::tcp("192.168.1.100:52381")
        .profile::<PTZOpticsG2>()
        .build();

    match tcp_camera {
        Ok(_camera) => {
            println!("Successfully created TCP camera with PTZOpticsG2 profile");
            // You can now use the camera to send commands
            // For example: camera.pan_tilt_home()?;
        }
        Err(e) => {
            println!("Failed to create TCP camera: {e}");
        }
    }

    // Example 2: Creating a UDP camera with GenericVisca profile
    println!("\n=== Example 2: UDP with GenericVisca profile ===");
    let udp_camera = CameraBuilder::udp("239.0.0.1:52381")
        .profile::<GenericVisca>()
        .build();

    match udp_camera {
        Ok(_camera) => {
            println!("Successfully created UDP camera with GenericVisca profile");
            // The GenericVisca profile supports a wide range of VISCA commands
        }
        Err(e) => {
            println!("Failed to create UDP camera: {e}");
        }
    }

    // Example 3: Demonstrating compile-time type safety
    println!("\n=== Example 3: Type Safety ===");

    // The builder pattern ensures type safety at compile time:
    // 1. You must call .profile() before .build()
    // 2. You cannot call .profile() twice
    // 3. The profile type must implement the Profile trait

    // This would not compile:
    // let camera = CameraBuilder::tcp("192.168.1.100:52381").build();
    //              ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    //              Error: no method named `build` found

    // This would also not compile:
    // let camera = CameraBuilder::tcp("192.168.1.100:52381")
    //     .profile::<PTZOpticsG2>()
    //     .profile::<GenericVisca>()  // Error: no method named `profile`
    //     .build();

    println!("The builder API enforces correct usage at compile time!");

    // Example 4: Handling connection errors gracefully
    println!("\n=== Example 4: Error Handling ===");
    let result = CameraBuilder::tcp("invalid-address:not-a-port")
        .profile::<PTZOpticsG2>()
        .build();

    if let Err(e) = result {
        println!("Expected error for invalid address: {e}");
    }

    Ok(())
}
