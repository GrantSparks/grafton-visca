//! Example demonstrating how to implement custom retry logic
//! after the removal of built-in resilient transport.
//!
//! This shows various patterns for handling retries at the application level.

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    transport::create,
    Error,
};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== Custom Retry Logic Demo ===\n");

    // Example 1: Simple retry wrapper
    simple_retry_example().await?;

    // Example 2: Exponential backoff
    exponential_backoff_example().await?;

    // Example 3: Custom retry wrapper struct
    custom_wrapper_example().await?;

    // Example 4: Advanced retry with error classification
    advanced_retry_example().await?;

    Ok(())
}

/// Example 1: Simple retry with fixed delay
async fn simple_retry_example() -> Result<(), Error> {
    println!("1. Simple retry with fixed delay:");

    let transport = create::tcp("192.168.1.100:5678").await?;
    let camera = Camera::<PTZOpticsG2>::new(transport);

    // Simple retry loop with fixed delay
    let max_retries = 3;
    let retry_delay = Duration::from_secs(1);

    for attempt in 1..=max_retries {
        match camera.get_power_state().await {
            Ok(power_on) => {
                println!("  ✓ Power state: {}", if power_on { "ON" } else { "OFF" });
                return Ok(());
            }
            Err(e) if attempt < max_retries => {
                println!("  ⚠️ Attempt {} failed: {}. Retrying...", attempt, e);
                sleep(retry_delay).await;
            }
            Err(e) => {
                println!("  ✗ All attempts failed: {}", e);
                return Err(e);
            }
        }
    }

    Ok(())
}

/// Example 2: Exponential backoff retry
async fn exponential_backoff_example() -> Result<(), Error> {
    println!("\n2. Exponential backoff retry:");

    let transport = create::tcp("192.168.1.100:5678").await?;
    let camera = Camera::<PTZOpticsG2>::new(transport);

    // Exponential backoff configuration
    let max_retries = 5;
    let initial_delay = Duration::from_millis(100);
    let max_delay = Duration::from_secs(10);
    let backoff_factor = 2.0;

    let mut delay = initial_delay;

    for attempt in 1..=max_retries {
        match camera.home().await {
            Ok(_) => {
                println!("  ✓ Camera moved to home position");
                return Ok(());
            }
            Err(e) if attempt < max_retries => {
                println!(
                    "  ⚠️ Attempt {} failed: {}. Waiting {:?}...",
                    attempt, e, delay
                );
                sleep(delay).await;

                // Calculate next delay with exponential backoff
                delay = Duration::from_millis((delay.as_millis() as f64 * backoff_factor) as u64);
                if delay > max_delay {
                    delay = max_delay;
                }
            }
            Err(e) => {
                println!("  ✗ All attempts exhausted: {}", e);
                return Err(e);
            }
        }
    }

    Ok(())
}

/// Example 3: Custom retry wrapper implementation
async fn custom_wrapper_example() -> Result<(), Error> {
    println!("\n3. Custom retry wrapper:");

    // Create a camera with our retry wrapper
    let transport = create::tcp("192.168.1.100:5678").await?;
    let camera = RetryingCamera::new(
        Camera::<PTZOpticsG2>::new(transport),
        RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(5),
            backoff_factor: 1.5,
        },
    );

    // Now all operations automatically retry
    match camera.zoom_in().await {
        Ok(_) => println!("  ✓ Zoom in successful"),
        Err(e) => println!("  ✗ Failed after retries: {}", e),
    }

    // Use other retry methods
    match camera.home().await {
        Ok(_) => println!("  ✓ Home successful"),
        Err(e) => println!("  ✗ Failed after retries: {}", e),
    }

    match camera.get_power_state().await {
        Ok(power_on) => println!("  ✓ Power state: {}", if power_on { "ON" } else { "OFF" }),
        Err(e) => println!("  ✗ Failed after retries: {}", e),
    }

    Ok(())
}

/// Configuration for retry behavior
#[derive(Clone, Debug)]
struct RetryConfig {
    max_attempts: u32,
    initial_delay: Duration,
    max_delay: Duration,
    backoff_factor: f64,
}

/// A wrapper that adds retry logic to any camera
struct RetryingCamera<P: grafton_visca::camera::CameraProfile> {
    camera: Camera<P>,
    config: RetryConfig,
}

impl<P: grafton_visca::camera::CameraProfile> RetryingCamera<P> {
    fn new(camera: Camera<P>, config: RetryConfig) -> Self {
        Self { camera, config }
    }

    // Example methods with retry logic
    pub async fn zoom_in(&self) -> Result<(), Error> {
        let mut delay = self.config.initial_delay;

        for attempt in 1..=self.config.max_attempts {
            match self.camera.zoom_in().await {
                Ok(result) => return Ok(result),
                Err(e) if attempt < self.config.max_attempts => {
                    log::warn!(
                        "Zoom in failed (attempt {}/{}): {}. Retrying in {:?}...",
                        attempt,
                        self.config.max_attempts,
                        e,
                        delay
                    );
                    sleep(delay).await;

                    // Calculate next delay
                    delay = Duration::from_millis(
                        (delay.as_millis() as f64 * self.config.backoff_factor) as u64,
                    );
                    if delay > self.config.max_delay {
                        delay = self.config.max_delay;
                    }
                }
                Err(e) => {
                    log::error!("Zoom in failed after {} attempts: {}", attempt, e);
                    return Err(e);
                }
            }
        }
        unreachable!()
    }

    pub async fn home(&self) -> Result<(), Error> {
        let mut delay = self.config.initial_delay;

        for attempt in 1..=self.config.max_attempts {
            match self.camera.home().await {
                Ok(result) => return Ok(result),
                Err(e) if attempt < self.config.max_attempts => {
                    log::warn!(
                        "Home failed (attempt {}/{}): {}. Retrying in {:?}...",
                        attempt,
                        self.config.max_attempts,
                        e,
                        delay
                    );
                    sleep(delay).await;

                    // Calculate next delay
                    delay = Duration::from_millis(
                        (delay.as_millis() as f64 * self.config.backoff_factor) as u64,
                    );
                    if delay > self.config.max_delay {
                        delay = self.config.max_delay;
                    }
                }
                Err(e) => {
                    log::error!("Home failed after {} attempts: {}", attempt, e);
                    return Err(e);
                }
            }
        }
        unreachable!()
    }

    pub async fn get_power_state(&self) -> Result<bool, Error> {
        let mut delay = self.config.initial_delay;

        for attempt in 1..=self.config.max_attempts {
            match self.camera.get_power_state().await {
                Ok(result) => return Ok(result),
                Err(e) if attempt < self.config.max_attempts => {
                    log::warn!(
                        "Get power state failed (attempt {}/{}): {}. Retrying in {:?}...",
                        attempt,
                        self.config.max_attempts,
                        e,
                        delay
                    );
                    sleep(delay).await;

                    // Calculate next delay
                    delay = Duration::from_millis(
                        (delay.as_millis() as f64 * self.config.backoff_factor) as u64,
                    );
                    if delay > self.config.max_delay {
                        delay = self.config.max_delay;
                    }
                }
                Err(e) => {
                    log::error!("Get power state failed after {} attempts: {}", attempt, e);
                    return Err(e);
                }
            }
        }
        unreachable!()
    }
}

/// Helper function to determine if an error is retryable
fn is_retryable(error: &Error) -> bool {
    matches!(
        error,
        Error::Io(_) | Error::Timeout | Error::TransportError(_) | Error::NoResponse
    )
}

/// Advanced retry with error classification
async fn retry_with_classification<F, Fut, T>(operation: F, max_attempts: u32) -> Result<T, Error>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, Error>>,
{
    for attempt in 1..=max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if is_retryable(&e) && attempt < max_attempts => {
                log::info!("Retryable error on attempt {}: {}", attempt, e);
                sleep(Duration::from_secs(1)).await;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

/// Example 4: Advanced retry with error classification
async fn advanced_retry_example() -> Result<(), Error> {
    println!("\n4. Advanced retry with error classification:");

    let transport = create::tcp("192.168.1.100:5678").await?;
    let camera = Camera::<PTZOpticsG2>::new(transport);

    // Use the retry function with error classification
    let result = retry_with_classification(|| async { camera.get_power_state().await }, 3).await;

    match result {
        Ok(power_on) => println!(
            "  ✓ Power state retrieved: {}",
            if power_on { "ON" } else { "OFF" }
        ),
        Err(e) => println!("  ✗ Failed with non-retryable error: {}", e),
    }

    // Another example with a different operation
    let zoom_result = retry_with_classification(|| async { camera.zoom_in().await }, 3).await;

    match zoom_result {
        Ok(_) => println!("  ✓ Zoom in successful with retry classification"),
        Err(e) => {
            if is_retryable(&e) {
                println!("  ✗ Failed with retryable error after all attempts: {}", e);
            } else {
                println!("  ✗ Failed with non-retryable error: {}", e);
            }
        }
    }

    Ok(())
}
