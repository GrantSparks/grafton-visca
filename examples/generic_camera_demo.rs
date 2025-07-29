//! Demo of the new generic camera API with compile-time profiles.

use grafton_visca::{
    camera::{
        generic::Camera,
        profiles::{PTZOpticsG2, SonyFR7},
    },
    capabilities::{NDFilter, Profile},
    prelude::*,
    Error,
};

// Example function that works with any camera profile
fn print_camera_info<P: Profile>(camera: &Camera<P, impl UnifiedTransport>) {
    println!("Camera: {}", camera.model_name());
    println!("Zoom range: {:?}", camera.zoom_speed_range());
    println!("Max pan speed: {}", camera.max_pan_speed());
    println!("Max presets: {}", camera.max_presets());
}

// Example function that only works with cameras that have ND filters
fn configure_nd_filter<P, T>(camera: &Camera<P, T>) 
where
    P: Profile + NDFilter,
    T: UnifiedTransport,
{
    println!("Camera {} has ND filter mode: {:?}", P::MODEL_NAME, P::ND_MODE);
    // In a real scenario, you could call:
    // camera.set_nd_filter_mode_blocking(NDFilterMode::Variable).unwrap();
}

fn main() -> Result<(), Error> {
    println!("Generic Camera API Demo\n");

    // These are just type demonstrations - we don't actually connect
    
    // Example 1: PTZOptics G2 camera (no ND filter)
    println!("=== PTZOptics G2 ===");
    type G2Camera<T> = Camera<PTZOpticsG2, T>;
    // This would be created like:
    // let camera = G2Camera::new(transport);
    
    // We can call generic functions
    // print_camera_info(&camera);
    
    // But this won't compile because PTZOpticsG2 doesn't implement NDFilter:
    // configure_nd_filter(&camera); // Compile error!
    
    println!("PTZOptics G2 profile loaded (no ND filter support)\n");

    // Example 2: Sony FR7 camera (has ND filter)
    println!("=== Sony FR7 ===");
    type FR7Camera<T> = Camera<SonyFR7, T>;
    // This would be created like:
    // let camera = FR7Camera::new(transport);
    
    // We can call generic functions
    // print_camera_info(&camera);
    
    // And this WILL compile because SonyFR7 implements NDFilter:
    // configure_nd_filter(&camera); // OK!
    
    println!("Sony FR7 profile loaded (has ND filter support)\n");

    // Example 3: Using type aliases from prelude
    println!("=== Using Type Aliases ===");
    // These provide cleaner syntax:
    // let g2_camera: PTZOpticsG2Cam<_> = PTZOpticsG2Cam::new(transport);
    // let fr7_camera: SonyFR7Cam<_> = SonyFR7Cam::new(transport);
    
    println!("Type aliases make the API more ergonomic");

    // Demonstrate compile-time safety
    println!("\n=== Compile-Time Safety ===");
    println!("The compiler prevents calling unsupported methods:");
    println!("- PTZOpticsG2 cannot call ND filter methods");
    println!("- SonyFR7 can call ND filter methods");
    println!("- No runtime checks needed!");

    Ok(())
}