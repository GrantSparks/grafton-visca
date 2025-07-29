//! Test that trait delegations work correctly for both blocking and async wrappers

use grafton_visca::blocking;

fn main() {
    // Test blocking API - for now just test compilation
    println!("Testing blocking trait delegation compilation...");
    test_blocking_compilation();

    // Test async API
    #[cfg(feature = "tokio")]
    {
        println!("\nTesting async trait delegation compilation...");
        test_async_compilation();
    }

    println!("\nAll trait delegations compile successfully!");
}

fn test_blocking_compilation() {
    // Import all blocking traits to ensure they're implemented
    use blocking::{ColorOps, PowerOps, WhiteBalanceOps, ZoomOps};

    // This function just needs to compile - we're verifying trait implementations exist
    fn _test_trait_methods<P, T>(camera: &blocking::Camera<P, T>)
    where
        P: grafton_visca::capabilities::Profile,
        T: grafton_visca::transport::UnifiedTransport,
    {
        // ZoomOps
        let _ = camera.zoom_stop();
        let _ = camera.zoom_in();
        let _ = camera.zoom_out();

        // PowerOps
        let _ = camera.power_on();
        let _ = camera.power_off();

        // ColorOps
        let _ = camera.one_push_trigger();
        let _ = camera.white_balance_auto();

        println!("✓ All blocking traits are properly implemented");
    }
}

#[cfg(feature = "tokio")]
fn test_async_compilation() {
    // Import all async traits to ensure they're implemented
    use grafton_visca::r#async::{ColorOps, PowerOps, WhiteBalanceOps, ZoomOps};

    // This function just needs to compile - we're verifying trait implementations exist
    async fn _test_trait_methods<P, T>(camera: &grafton_visca::r#async::Camera<P, T>)
    where
        P: grafton_visca::capabilities::Profile,
        T: grafton_visca::transport::UnifiedTransport,
    {
        // ZoomOps
        let _ = camera.zoom_stop().await;
        let _ = camera.zoom_in().await;
        let _ = camera.zoom_out().await;

        // PowerOps
        let _ = camera.power_on().await;
        let _ = camera.power_off().await;

        // ColorOps
        let _ = camera.one_push_trigger().await;
        let _ = camera.white_balance_auto().await;

        println!("✓ All async traits are properly implemented");
    }
}
