//! Demo of the unified Camera API in both blocking and async contexts.
//!
//! This example shows how the UnifiedCamera API works seamlessly without
//! requiring generic type parameters for profile or transport.

use grafton_visca::{Error, ProfileId, UnifiedCamera};

#[cfg(not(feature = "tokio"))]
use grafton_visca::transport::blocking::{Tcp, Udp};

#[cfg(feature = "tokio")]
use grafton_visca::transport::tokio::{Tcp, Udp};

// ==================== BLOCKING EXAMPLES ====================

#[cfg(not(feature = "tokio"))]
fn blocking_examples() -> Result<(), Error> {
    println!("=== Unified Camera with Blocking Transport ===\n");

    // Example 1: Create camera with default profile (GenericVisca)
    println!("Example 1: Default profile with UDP");
    let udp_transport = Udp::connect("192.168.1.100:52381")?;
    let camera = UnifiedCamera::new_blocking(udp_transport);
    println!("Created camera: {}", camera.model_name());
    println!("Profile info: {}", camera.profile_info());

    // Example 2: Create camera with specific profile
    println!("\nExample 2: PTZOptics G2 profile with TCP");
    let tcp_transport = Tcp::connect("192.168.1.100:5678")?;
    let camera = UnifiedCamera::with_profile_blocking(ProfileId::PTZOpticsG2, tcp_transport);
    println!("Created camera: {}", camera.model_name());
    println!("Profile info: {}", camera.profile_info());

    // Example 3: Create Sony FR7 camera
    println!("\nExample 3: Sony FR7 profile");
    let transport = Udp::connect("192.168.1.200:52381")?;
    let camera = UnifiedCamera::with_profile_blocking(ProfileId::SonyFR7, transport);
    println!("Created camera: {}", camera.model_name());
    println!("Profile info: {}", camera.profile_info());

    // Example 4: Check capabilities at runtime
    println!("\nExample 4: Runtime capability checking");
    let transport = Tcp::connect("192.168.1.100:5678")?;
    let camera = UnifiedCamera::with_profile_blocking(ProfileId::PTZOpticsG2, transport);
    
    println!("Capabilities for {}:", camera.model_name());
    println!("  - Pan/Tilt: {}", camera.supports_capability("pan_tilt"));
    println!("  - Zoom: {}", camera.supports_capability("zoom"));
    println!("  - Focus: {}", camera.supports_capability("focus"));
    println!("  - ND Filter: {}", camera.supports_capability("nd_filter"));
    println!("  - Presets: {}", camera.supports_capability("presets"));

    Ok(())
}

// ==================== ASYNC EXAMPLES ====================

#[cfg(feature = "tokio")]
async fn async_examples() -> Result<(), Error> {
    println!("=== Unified Camera with Async Transport ===\n");

    // Example 1: Create camera with default profile (GenericVisca)
    println!("Example 1: Default profile with UDP");
    let udp_transport = Udp::connect("192.168.1.100:52381").await?;
    let camera = UnifiedCamera::new(udp_transport);
    println!("Created camera: {}", camera.model_name());
    println!("Profile info: {}", camera.profile_info());

    // Example 2: Create camera with specific profile
    println!("\nExample 2: PTZOptics G2 profile with TCP");
    let tcp_transport = Tcp::connect("192.168.1.100:5678").await?;
    let camera = UnifiedCamera::with_profile(ProfileId::PTZOpticsG2, tcp_transport);
    println!("Created camera: {}", camera.model_name());
    println!("Profile info: {}", camera.profile_info());

    // Example 3: Create Sony FR7 camera
    println!("\nExample 3: Sony FR7 profile");
    let transport = Udp::connect("192.168.1.200:52381").await?;
    let camera = UnifiedCamera::with_profile(ProfileId::SonyFR7, transport);
    println!("Created camera: {}", camera.model_name());
    println!("Profile info: {}", camera.profile_info());

    // Example 4: Check capabilities at runtime
    println!("\nExample 4: Runtime capability checking");
    let transport = Tcp::connect("192.168.1.100:5678").await?;
    let camera = UnifiedCamera::with_profile(ProfileId::PTZOpticsG2, transport);
    
    println!("Capabilities for {}:", camera.model_name());
    println!("  - Pan/Tilt: {}", camera.supports_capability("pan_tilt"));
    println!("  - Zoom: {}", camera.supports_capability("zoom"));
    println!("  - Focus: {}", camera.supports_capability("focus"));
    println!("  - ND Filter: {}", camera.supports_capability("nd_filter"));
    println!("  - Presets: {}", camera.supports_capability("presets"));

    Ok(())
}

// ==================== ADVANCED EXAMPLE ====================

fn advanced_example() {
    println!("\n=== Advanced UnifiedCamera Features ===\n");

    // Show the benefits of the unified API
    println!("Benefits of UnifiedCamera:");
    println!("1. No generic type parameters needed");
    println!("2. Profile selected at runtime via enum");
    println!("3. Works with any transport (async or blocking)");
    println!("4. Runtime capability introspection");
    println!("5. Simplified API for new users");
    
    println!("\nComparison with generic API:");
    println!("OLD: CameraAsync<PTZOpticsG2, Tcp>");
    println!("NEW: UnifiedCamera (profile and transport hidden)");
    
    println!("\nProfile options:");
    println!("- ProfileId::GenericVisca (default)");
    println!("- ProfileId::PTZOpticsG2");
    println!("- ProfileId::SonyFR7");
}

// ==================== MAIN FUNCTIONS ====================

#[cfg(not(feature = "tokio"))]
fn main() -> Result<(), Error> {
    env_logger::init();

    println!("=== UnifiedCamera Demo (Blocking Mode) ===\n");
    println!("This demo shows the new UnifiedCamera API that eliminates generics.\n");

    blocking_examples()?;
    advanced_example();

    println!("\n=== Demo Complete ===");
    println!("\nKey takeaways:");
    println!("• UnifiedCamera provides a single type for all cameras");
    println!("• No generic parameters required");
    println!("• Profile selection is done at runtime");
    println!("• Works with both async and blocking transports");
    println!("• Provides runtime capability introspection");

    Ok(())
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    println!("=== UnifiedCamera Demo (Async Mode) ===\n");
    println!("This demo shows the new UnifiedCamera API that eliminates generics.\n");

    async_examples().await?;
    advanced_example();

    println!("\n=== Demo Complete ===");
    println!("\nKey takeaways:");
    println!("• UnifiedCamera provides a single type for all cameras");
    println!("• No generic parameters required");
    println!("• Profile selection is done at runtime");
    println!("• Works with both async and blocking transports");
    println!("• Provides runtime capability introspection");

    Ok(())
}