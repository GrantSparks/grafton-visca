//! Basic camera control demonstration showing blocking-first and async approaches
//!
//! This example demonstrates common camera operations using the new API design.

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, units::Degrees},
    command::{
        exposure::ExposureMode, pan_tilt::PanTiltDirection, white_balance::WhiteBalanceMode,
    },
    Camera, Error,
};
use std::time::Duration;

#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
mod blocking_demo {
    use super::*;
    use grafton_visca::transport::blocking::create;
    use std::thread;

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        println!("🎥 Basic Camera Demo - Blocking Mode");
        println!("====================================\n");

        // Create blocking transport
        let transport = create::tcp("192.168.1.100:5678")?;
        let mut camera = Camera::<PTZOpticsG2>::new(transport);

        // Demo 1: Power Control
        demo_power_control(&mut camera)?;

        // Demo 2: Pan/Tilt Movement
        demo_pan_tilt_movement(&mut camera)?;

        // Demo 3: Zoom Control
        demo_zoom_control(&mut camera)?;

        // Demo 4: Focus Control
        demo_focus_control(&mut camera)?;

        // Demo 5: Exposure Settings
        demo_exposure_settings(&mut camera)?;

        // Demo 6: White Balance
        demo_white_balance(&mut camera)?;

        // Demo 7: Position Control
        demo_position_control(&mut camera)?;

        println!("\n✅ All demos completed successfully!");
        Ok(())
    }

    fn demo_power_control(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
        println!("📍 Demo 1: Power Control");
        println!("Powering on camera...");
        camera.power_on()?;
        println!("✅ Camera powered on successfully");
        thread::sleep(Duration::from_secs(2));
        Ok(())
    }

    fn demo_pan_tilt_movement(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
        println!("\n📍 Demo 2: Pan/Tilt Movement");

        println!("Moving to home position...");
        camera.home()?;
        thread::sleep(Duration::from_secs(2));

        println!("Moving camera up-right...");
        camera.move_continuous(PanTiltDirection::UpRight, 16, 16)?;
        thread::sleep(Duration::from_secs(1));

        println!("Stopping movement...");
        camera.stop()?;
        Ok(())
    }

    fn demo_zoom_control(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
        println!("\n📍 Demo 3: Zoom Control");

        println!("Zooming in...");
        camera.zoom_in()?;
        thread::sleep(Duration::from_secs(1));

        println!("Stopping zoom...");
        camera.zoom_stop()?;

        println!("Setting zoom to 50%...");
        camera.set_zoom(0x3800)?; // Mid-range zoom
        thread::sleep(Duration::from_secs(1));

        println!("Resetting zoom...");
        camera.set_zoom(0x0000)?;
        Ok(())
    }

    fn demo_focus_control(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
        println!("\n📍 Demo 4: Focus Control");

        println!("Setting auto-focus mode...");
        camera.focus_auto()?;
        thread::sleep(Duration::from_millis(500));

        println!("Switching to manual focus...");
        camera.focus_manual()?;
        Ok(())
    }

    fn demo_exposure_settings(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
        println!("\n📍 Demo 5: Exposure Settings");

        println!("Setting exposure to auto...");
        camera.set_exposure_mode(ExposureMode::Auto)?;
        thread::sleep(Duration::from_millis(500));

        println!("Switching to shutter priority mode...");
        camera.set_exposure_mode(ExposureMode::Shutter)?;
        Ok(())
    }

    fn demo_white_balance(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
        println!("\n📍 Demo 6: White Balance");

        println!("Setting white balance to auto...");
        camera.set_white_balance_mode(WhiteBalanceMode::Auto)?;
        thread::sleep(Duration::from_millis(500));

        println!("Switching to indoor mode...");
        camera.set_white_balance_mode(WhiteBalanceMode::Indoor)?;
        Ok(())
    }

    fn demo_position_control(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
        println!("\n📍 Demo 7: Position Control");

        println!("Moving to specific position (45°, 20°)...");
        camera.set_position(Degrees(45.0), Degrees(20.0))?;
        thread::sleep(Duration::from_secs(2));

        println!("Moving to position (-30°, -10°)...");
        camera.set_position(Degrees(-30.0), Degrees(-10.0))?;
        thread::sleep(Duration::from_secs(2));

        println!("Returning to home...");
        camera.home()?;
        thread::sleep(Duration::from_secs(2));
        Ok(())
    }
}

#[cfg(feature = "async-client")]
mod async_demo {
    use super::*;
    use grafton_visca::transport::create;

    pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
        println!("🎥 Basic Camera Demo - Async Mode");
        println!("==================================\n");

        // Create async transport
        let transport = create::tcp("192.168.1.100:5678").await?;
        let camera = Camera::<PTZOpticsG2>::new(transport);

        // Demo 1: Power Control
        demo_power_control(&camera).await?;

        // Demo 2: Pan/Tilt Movement
        demo_pan_tilt_movement(&camera).await?;

        // Demo 3: Zoom Control
        demo_zoom_control(&camera).await?;

        // Demo 4: Focus Control
        demo_focus_control(&camera).await?;

        // Demo 5: Exposure Settings
        demo_exposure_settings(&camera).await?;

        // Demo 6: White Balance
        demo_white_balance(&camera).await?;

        // Demo 7: Position Control
        demo_position_control(&camera).await?;

        println!("\n✅ All demos completed successfully!");
        Ok(())
    }

    async fn demo_power_control(camera: &Camera<PTZOpticsG2>) -> Result<(), Error> {
        println!("📍 Demo 1: Power Control");
        println!("Powering on camera...");
        camera.power_on().await?;
        println!("✅ Camera powered on successfully");
        tokio::time::sleep(Duration::from_secs(2)).await;
        Ok(())
    }

    async fn demo_pan_tilt_movement(camera: &Camera<PTZOpticsG2>) -> Result<(), Error> {
        println!("\n📍 Demo 2: Pan/Tilt Movement");

        println!("Moving to home position...");
        camera.home().await?;
        tokio::time::sleep(Duration::from_secs(2)).await;

        println!("Moving camera up-right...");
        camera
            .move_continuous(PanTiltDirection::UpRight, 16, 16)
            .await?;
        tokio::time::sleep(Duration::from_secs(1)).await;

        println!("Stopping movement...");
        camera.stop().await?;
        Ok(())
    }

    async fn demo_zoom_control(camera: &Camera<PTZOpticsG2>) -> Result<(), Error> {
        println!("\n📍 Demo 3: Zoom Control");

        println!("Zooming in...");
        camera.zoom_in().await?;
        tokio::time::sleep(Duration::from_secs(1)).await;

        println!("Stopping zoom...");
        camera.zoom_stop().await?;

        println!("Setting zoom to 50%...");
        camera.set_zoom(0x3800).await?; // Mid-range zoom
        tokio::time::sleep(Duration::from_secs(1)).await;

        println!("Resetting zoom...");
        camera.set_zoom(0x0000).await?;
        Ok(())
    }

    async fn demo_focus_control(camera: &Camera<PTZOpticsG2>) -> Result<(), Error> {
        println!("\n📍 Demo 4: Focus Control");

        println!("Setting auto-focus mode...");
        camera.focus_auto().await?;
        tokio::time::sleep(Duration::from_millis(500)).await;

        println!("Switching to manual focus...");
        camera.focus_manual().await?;
        Ok(())
    }

    async fn demo_exposure_settings(camera: &Camera<PTZOpticsG2>) -> Result<(), Error> {
        println!("\n📍 Demo 5: Exposure Settings");

        println!("Setting exposure to auto...");
        camera.set_exposure_mode(ExposureMode::Auto).await?;
        tokio::time::sleep(Duration::from_millis(500)).await;

        println!("Switching to shutter priority mode...");
        camera.set_exposure_mode(ExposureMode::Shutter).await?;
        Ok(())
    }

    async fn demo_white_balance(camera: &Camera<PTZOpticsG2>) -> Result<(), Error> {
        println!("\n📍 Demo 6: White Balance");

        println!("Setting white balance to auto...");
        camera
            .set_white_balance_mode(WhiteBalanceMode::Auto)
            .await?;
        tokio::time::sleep(Duration::from_millis(500)).await;

        println!("Switching to indoor mode...");
        camera
            .set_white_balance_mode(WhiteBalanceMode::Indoor)
            .await?;
        Ok(())
    }

    async fn demo_position_control(camera: &Camera<PTZOpticsG2>) -> Result<(), Error> {
        println!("\n📍 Demo 7: Position Control");

        println!("Moving to specific position (45°, 20°)...");
        camera.set_position(Degrees(45.0), Degrees(20.0)).await?;
        tokio::time::sleep(Duration::from_secs(2)).await;

        println!("Moving to position (-30°, -10°)...");
        camera.set_position(Degrees(-30.0), Degrees(-10.0)).await?;
        tokio::time::sleep(Duration::from_secs(2)).await;

        println!("Returning to home...");
        camera.home().await?;
        tokio::time::sleep(Duration::from_secs(2)).await;
        Ok(())
    }
}

// Main function adapts based on features
#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    blocking_demo::run()
}

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    async_demo::run().await
}

#[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
fn main() {
    eprintln!("This example requires either 'blocking-client' or 'async-client' feature.");
    eprintln!("Try: cargo run --example basic_camera_demo --features blocking-client");
}
