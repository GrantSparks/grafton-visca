//! Unified movement detection algorithms using GAT-based probes.
//!
//! This module contains the actual movement detection logic that works
//! with both blocking and async implementations through the probe traits.

use std::time::Instant;

use crate::error::Error;

use super::movement_probe::{
    positions_equal_within_tolerance, positions_equal_within_tolerance_separate,
    zoom_equal_within_tolerance, MovementDetectionConfig,
};

#[cfg(feature = "async")]
use super::movement_probe::{FocusProbe, MovementProbe, ZoomProbe};

// ============= Async Versions (feature-gated) =============
#[cfg(feature = "async")]
/// Wait for pan/tilt movement to complete.
pub async fn wait_for_pan_tilt_completion<P>(
    probe: &P,
    config: &MovementDetectionConfig,
) -> Result<(), Error>
where
    P: MovementProbe,
{
    let start = Instant::now();

    // Wait for movement to potentially start
    probe.sleep(config.startup_delay).await;

    let initial_position = probe.get_position().await?;
    let mut last_position = initial_position;
    let mut stable_count = 0;
    let mut has_moved = false;
    let mut oscillation_detector = Vec::new();

    loop {
        if start.elapsed() > config.timeout {
            if config.debug {
                log::debug!("Pan/tilt movement timed out");
            }
            return Err(Error::Timeout);
        }

        let current_position = probe.get_position().await?;

        if !has_moved
            && (!positions_equal_within_tolerance_separate(
                initial_position,
                current_position,
                config.tolerance_pan_start,
                config.tolerance_tilt_start,
            ))
        {
            has_moved = true;
            if config.debug {
                log::debug!("Pan/tilt movement started");
            }
        }

        if positions_equal_within_tolerance_separate(
            last_position,
            current_position,
            config.tolerance_pan_stable,
            config.tolerance_tilt_stable,
        ) {
            stable_count += 1;
            if stable_count >= config.stability_threshold {
                if has_moved {
                    if config.debug {
                        log::debug!("Pan/tilt movement completed (stable position)");
                    }
                    return Ok(());
                } else if stable_count >= config.stability_threshold * 2 {
                    if config.debug {
                        log::debug!("Pan/tilt position stable (no movement detected)");
                    }
                    return Ok(());
                }
            }
        } else {
            stable_count = 0;

            // Oscillation detection
            oscillation_detector.push(current_position);
            if oscillation_detector.len() > config.oscillation_sample_size {
                oscillation_detector.remove(0);

                // Check for oscillation pattern
                if oscillation_detector.len() >= config.oscillation_min_samples {
                    let recent = &oscillation_detector
                        [oscillation_detector.len() - config.oscillation_min_samples..];
                    let mut oscillating = true;
                    for i in 0..3 {
                        if !positions_equal_within_tolerance(
                            recent[i * 2],
                            recent[0],
                            config.tolerance_pan_tilt_oscillation,
                        ) || !positions_equal_within_tolerance(
                            recent[i * 2 + 1],
                            recent[1],
                            config.tolerance_pan_tilt_oscillation,
                        ) {
                            oscillating = false;
                            break;
                        }
                    }
                    if oscillating {
                        if config.debug {
                            log::warn!("Detected oscillation in pan/tilt, treating as complete");
                        }
                        return Ok(());
                    }
                }
            }
        }

        // Early exit if no movement detected after initial delay
        if !has_moved && start.elapsed() > config.no_movement_timeout {
            if config.debug {
                log::debug!("No pan/tilt movement detected, camera likely already at position");
            }
            return Ok(());
        }

        last_position = current_position;
        probe.sleep(config.poll_interval).await;
    }
}

#[cfg(feature = "async")]
/// Wait for zoom movement to complete.
pub async fn wait_for_zoom_completion<P>(
    probe: &P,
    config: &MovementDetectionConfig,
) -> Result<(), Error>
where
    P: ZoomProbe,
{
    let start = Instant::now();

    probe.sleep(config.startup_delay).await;

    let initial_zoom = probe.get_zoom().await?;
    let mut last_zoom = initial_zoom;
    let mut stable_count = 0;
    let mut has_moved = false;

    loop {
        if start.elapsed() > config.timeout {
            if config.debug {
                log::debug!("Zoom movement timed out");
            }
            return Err(Error::Timeout);
        }

        let current_zoom = probe.get_zoom().await?;

        if !has_moved
            && !zoom_equal_within_tolerance(initial_zoom, current_zoom, config.tolerance_zoom_start)
        {
            has_moved = true;
            if config.debug {
                log::debug!("Zoom movement started");
            }
        }

        if zoom_equal_within_tolerance(last_zoom, current_zoom, config.tolerance_zoom_stable) {
            stable_count += 1;
            if stable_count >= config.stability_threshold {
                if has_moved {
                    if config.debug {
                        log::debug!("Zoom movement completed (stable position)");
                    }
                    return Ok(());
                } else if stable_count >= config.stability_threshold * 2 {
                    if config.debug {
                        log::debug!("Zoom position stable (no movement detected)");
                    }
                    return Ok(());
                }
            }
        } else {
            stable_count = 0;
        }

        // Early exit if no movement detected
        if !has_moved && start.elapsed() > config.no_movement_timeout {
            if config.debug {
                log::debug!("No zoom movement detected, camera likely already at position");
            }
            return Ok(());
        }

        last_zoom = current_zoom;
        probe.sleep(config.poll_interval).await;
    }
}

#[cfg(feature = "async")]
/// Wait for all movements (pan/tilt, zoom, focus) to complete.
///
/// This uses the existing detection algorithms in parallel, providing
/// a single source of truth for stability/oscillation heuristics.
/// The overall latency is the max of the three waits, not the sum.
pub async fn wait_for_all_movements<M, Z, F>(
    movement_probe: &M,
    zoom_probe: &Z,
    focus_probe: &F,
    config: &MovementDetectionConfig,
) -> Result<(), Error>
where
    M: MovementProbe,
    Z: ZoomProbe,
    F: FocusProbe,
{
    // Use try_join to run all three in parallel
    let pan_tilt_future = wait_for_pan_tilt_completion(movement_probe, config);
    let zoom_future = wait_for_zoom_completion(zoom_probe, config);
    let focus_future = wait_for_focus_completion(focus_probe, config);

    // Wait for all three to complete
    match futures::try_join!(pan_tilt_future, zoom_future, focus_future) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(feature = "async")]
/// Wait for focus movement to complete.
pub async fn wait_for_focus_completion<P>(
    probe: &P,
    config: &MovementDetectionConfig,
) -> Result<(), Error>
where
    P: FocusProbe,
{
    let start = Instant::now();

    probe.sleep(config.startup_delay).await;

    let initial_focus = probe.get_focus().await?;
    let mut last_focus = initial_focus;
    let mut stable_count = 0;
    let mut has_moved = false;

    loop {
        if start.elapsed() > config.timeout {
            if config.debug {
                log::debug!("Focus movement timed out");
            }
            return Err(Error::Timeout);
        }

        let current_focus = probe.get_focus().await?;

        if !has_moved
            && (initial_focus as i32 - current_focus as i32).abs()
                > config.tolerance_focus_start as i32
        {
            has_moved = true;
            if config.debug {
                log::debug!("Focus movement started");
            }
        }

        if (last_focus as i32 - current_focus as i32).abs() <= config.tolerance_focus_stable as i32
        {
            stable_count += 1;
            if stable_count >= config.stability_threshold {
                if has_moved {
                    if config.debug {
                        log::debug!("Focus movement completed");
                    }
                    return Ok(());
                } else if stable_count >= config.stability_threshold * 2 {
                    if config.debug {
                        log::debug!("Focus stable (no movement)");
                    }
                    return Ok(());
                }
            }
        } else {
            stable_count = 0;
        }

        // Early exit if no movement detected
        if !has_moved && start.elapsed() > config.no_movement_timeout {
            if config.debug {
                log::debug!("No focus movement detected");
            }
            return Ok(());
        }

        last_focus = current_focus;
        probe.sleep(config.poll_interval).await;
    }
}

// ============= Native Blocking Versions =============
// These are truly native blocking implementations that don't use async/await or futures.
// They work directly with the Camera type and its blocking methods.

use super::{movement_probe::PanTiltPosition, Camera};
use crate::{capabilities::Profile, transport::UnifiedTransport};

/// Native blocking implementation for pan/tilt movement detection.
/// This works directly with the Camera without any async machinery.
pub fn wait_for_pan_tilt_completion_blocking<P, T>(
    camera: &Camera<P, T>,
    config: &MovementDetectionConfig,
) -> Result<(), Error>
where
    P: Profile,
    T: UnifiedTransport,
{
    use crate::camera::methods::inquiry::PanTiltInquiryOpsBlocking;

    let start = Instant::now();

    // Wait for movement to potentially start
    std::thread::sleep(config.startup_delay);

    let initial_position = {
        let (pan, tilt) = camera.get_pan_tilt_position()?;
        PanTiltPosition { pan, tilt }
    };
    let mut last_position = initial_position;
    let mut stable_count = 0;
    let mut has_moved = false;
    let mut oscillation_detector = Vec::new();

    loop {
        if start.elapsed() > config.timeout {
            if config.debug {
                log::debug!("Pan/tilt movement timed out");
            }
            return Err(Error::Timeout);
        }

        let current_position = {
            let (pan, tilt) = camera.get_pan_tilt_position()?;
            PanTiltPosition { pan, tilt }
        };

        if !has_moved
            && (!positions_equal_within_tolerance_separate(
                initial_position,
                current_position,
                config.tolerance_pan_start,
                config.tolerance_tilt_start,
            ))
        {
            has_moved = true;
            if config.debug {
                log::debug!("Pan/tilt movement started");
            }
        }

        if positions_equal_within_tolerance_separate(
            last_position,
            current_position,
            config.tolerance_pan_stable,
            config.tolerance_tilt_stable,
        ) {
            stable_count += 1;
            if stable_count >= config.stability_threshold {
                if has_moved {
                    if config.debug {
                        log::debug!("Pan/tilt movement completed (stable position)");
                    }
                    return Ok(());
                } else if stable_count >= config.stability_threshold * 2 {
                    if config.debug {
                        log::debug!("Pan/tilt position stable (no movement detected)");
                    }
                    return Ok(());
                }
            }
        } else {
            stable_count = 0;

            // Oscillation detection
            oscillation_detector.push(current_position);
            if oscillation_detector.len() > config.oscillation_sample_size {
                oscillation_detector.remove(0);

                // Check for oscillation pattern
                if oscillation_detector.len() >= config.oscillation_min_samples {
                    let recent = &oscillation_detector
                        [oscillation_detector.len() - config.oscillation_min_samples..];
                    let mut oscillating = true;
                    for i in 0..3 {
                        if !positions_equal_within_tolerance(
                            recent[i * 2],
                            recent[0],
                            config.tolerance_pan_tilt_oscillation,
                        ) || !positions_equal_within_tolerance(
                            recent[i * 2 + 1],
                            recent[1],
                            config.tolerance_pan_tilt_oscillation,
                        ) {
                            oscillating = false;
                            break;
                        }
                    }
                    if oscillating {
                        if config.debug {
                            log::warn!("Detected oscillation in pan/tilt, treating as complete");
                        }
                        return Ok(());
                    }
                }
            }
        }

        // Early exit if no movement detected after initial delay
        if !has_moved && start.elapsed() > config.no_movement_timeout {
            if config.debug {
                log::debug!("No pan/tilt movement detected, camera likely already at position");
            }
            return Ok(());
        }

        last_position = current_position;
        std::thread::sleep(config.poll_interval);
    }
}

/// Native blocking implementation for zoom movement detection.
pub fn wait_for_zoom_completion_blocking<P, T>(
    camera: &Camera<P, T>,
    config: &MovementDetectionConfig,
) -> Result<(), Error>
where
    P: Profile,
    T: UnifiedTransport,
{
    use crate::camera::methods::inquiry::InquiryOpsBlocking;

    let start = Instant::now();
    std::thread::sleep(config.startup_delay);

    let initial_zoom = camera.get_zoom_position()?;
    let mut last_zoom = initial_zoom;
    let mut stable_count = 0;
    let mut has_moved = false;

    loop {
        if start.elapsed() > config.timeout {
            if config.debug {
                log::debug!("Zoom movement timed out");
            }
            return Err(Error::Timeout);
        }

        let current_zoom = camera.get_zoom_position()?;

        if !has_moved
            && !zoom_equal_within_tolerance(initial_zoom, current_zoom, config.tolerance_zoom_start)
        {
            has_moved = true;
            if config.debug {
                log::debug!("Zoom movement started");
            }
        }

        if zoom_equal_within_tolerance(last_zoom, current_zoom, config.tolerance_zoom_stable) {
            stable_count += 1;
            if stable_count >= config.stability_threshold {
                if has_moved {
                    if config.debug {
                        log::debug!("Zoom movement completed (stable position)");
                    }
                    return Ok(());
                } else if stable_count >= config.stability_threshold * 2 {
                    if config.debug {
                        log::debug!("Zoom position stable (no movement detected)");
                    }
                    return Ok(());
                }
            }
        } else {
            stable_count = 0;
        }

        // Early exit if no movement detected
        if !has_moved && start.elapsed() > config.no_movement_timeout {
            if config.debug {
                log::debug!("No zoom movement detected, camera likely already at position");
            }
            return Ok(());
        }

        last_zoom = current_zoom;
        std::thread::sleep(config.poll_interval);
    }
}

/// Native blocking implementation for focus movement detection.
pub fn wait_for_focus_completion_blocking<P, T>(
    camera: &Camera<P, T>,
    config: &MovementDetectionConfig,
) -> Result<(), Error>
where
    P: Profile,
    T: UnifiedTransport,
{
    use crate::camera::methods::inquiry::InquiryOpsBlocking;

    let start = Instant::now();
    std::thread::sleep(config.startup_delay);

    let initial_focus = camera.get_focus_position()?;
    let mut last_focus = initial_focus;
    let mut stable_count = 0;
    let mut has_moved = false;

    loop {
        if start.elapsed() > config.timeout {
            if config.debug {
                log::debug!("Focus movement timed out");
            }
            return Err(Error::Timeout);
        }

        let current_focus = camera.get_focus_position()?;

        if !has_moved
            && (initial_focus as i32 - current_focus as i32).abs()
                > config.tolerance_focus_start as i32
        {
            has_moved = true;
            if config.debug {
                log::debug!("Focus movement started");
            }
        }

        if (last_focus as i32 - current_focus as i32).abs() <= config.tolerance_focus_stable as i32
        {
            stable_count += 1;
            if stable_count >= config.stability_threshold {
                if has_moved {
                    if config.debug {
                        log::debug!("Focus movement completed");
                    }
                    return Ok(());
                } else if stable_count >= config.stability_threshold * 2 {
                    if config.debug {
                        log::debug!("Focus stable (no movement)");
                    }
                    return Ok(());
                }
            }
        } else {
            stable_count = 0;
        }

        // Early exit if no movement detected
        if !has_moved && start.elapsed() > config.no_movement_timeout {
            if config.debug {
                log::debug!("No focus movement detected");
            }
            return Ok(());
        }

        last_focus = current_focus;
        std::thread::sleep(config.poll_interval);
    }
}

/// Native blocking implementation for all movements detection.
pub fn wait_for_all_movements_blocking<P, T>(
    camera: &Camera<P, T>,
    config: &MovementDetectionConfig,
) -> Result<(), Error>
where
    P: Profile,
    T: UnifiedTransport,
{
    // In blocking mode, we run these sequentially
    // This is the nature of blocking code - no parallelism without threads
    wait_for_pan_tilt_completion_blocking(camera, config)?;
    wait_for_zoom_completion_blocking(camera, config)?;
    wait_for_focus_completion_blocking(camera, config)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "async")]
    use std::future::Future;
    #[cfg(feature = "async")]
    use std::pin::Pin;
    #[cfg(feature = "async")]
    use std::sync::{Arc, Mutex};
    #[cfg(feature = "async")]
    use std::task::{Context, Poll};
    use std::time::Duration;

    // Mock probe implementations for testing
    #[cfg(feature = "async")]
    struct MockMovementProbe {
        positions: Arc<Mutex<Vec<PanTiltPosition>>>,
        current_index: Arc<Mutex<usize>>,
    }

    #[cfg(feature = "async")]
    impl MockMovementProbe {
        fn new(positions: Vec<PanTiltPosition>) -> Self {
            Self {
                positions: Arc::new(Mutex::new(positions)),
                current_index: Arc::new(Mutex::new(0)),
            }
        }
    }

    // Ready future that immediately completes
    #[cfg(feature = "async")]
    #[allow(clippy::expect_used)] // Test code - panic is acceptable
    struct ReadyFuture<T>(Option<T>);

    #[cfg(feature = "async")]
    #[allow(clippy::expect_used)] // Test code - panic is acceptable
    impl<T: Unpin> Future for ReadyFuture<T> {
        type Output = T;

        fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Ready(self.0.take().expect("Future polled twice"))
        }
    }

    #[cfg(feature = "async")]
    #[allow(clippy::unwrap_used)] // Test code - unwrap is acceptable
    impl MovementProbe for MockMovementProbe {
        type Sleep<'a> = ReadyFuture<()>;
        type PositionFuture<'a> = ReadyFuture<Result<PanTiltPosition, Error>>;

        fn get_position(&self) -> Self::PositionFuture<'_> {
            let positions = self.positions.lock().unwrap();
            let mut index = self.current_index.lock().unwrap();
            let pos = if *index < positions.len() {
                positions[*index]
            } else {
                positions[positions.len() - 1]
            };
            *index += 1;
            ReadyFuture(Some(Ok(pos)))
        }

        fn sleep(&self, _duration: Duration) -> Self::Sleep<'_> {
            ReadyFuture(Some(()))
        }
    }

    #[cfg(feature = "async")]
    struct MockZoomProbe {
        positions: Arc<Mutex<Vec<u16>>>,
        current_index: Arc<Mutex<usize>>,
    }

    #[cfg(feature = "async")]
    impl MockZoomProbe {
        fn new(positions: Vec<u16>) -> Self {
            Self {
                positions: Arc::new(Mutex::new(positions)),
                current_index: Arc::new(Mutex::new(0)),
            }
        }
    }

    #[cfg(feature = "async")]
    #[allow(clippy::unwrap_used)] // Test code - unwrap is acceptable
    impl ZoomProbe for MockZoomProbe {
        type Sleep<'a> = ReadyFuture<()>;
        type ZoomFuture<'a> = ReadyFuture<Result<u16, Error>>;

        fn get_zoom(&self) -> Self::ZoomFuture<'_> {
            let positions = self.positions.lock().unwrap();
            let mut index = self.current_index.lock().unwrap();
            let pos = if *index < positions.len() {
                positions[*index]
            } else {
                positions[positions.len() - 1]
            };
            *index += 1;
            ReadyFuture(Some(Ok(pos)))
        }

        fn sleep(&self, _duration: Duration) -> Self::Sleep<'_> {
            ReadyFuture(Some(()))
        }
    }

    #[cfg(feature = "async")]
    struct MockFocusProbe {
        positions: Arc<Mutex<Vec<u16>>>,
        current_index: Arc<Mutex<usize>>,
    }

    #[cfg(feature = "async")]
    impl MockFocusProbe {
        fn new(positions: Vec<u16>) -> Self {
            Self {
                positions: Arc::new(Mutex::new(positions)),
                current_index: Arc::new(Mutex::new(0)),
            }
        }
    }

    #[cfg(feature = "async")]
    #[allow(clippy::unwrap_used)] // Test code - unwrap is acceptable
    impl FocusProbe for MockFocusProbe {
        type Sleep<'a> = ReadyFuture<()>;
        type FocusFuture<'a> = ReadyFuture<Result<u16, Error>>;

        fn get_focus(&self) -> Self::FocusFuture<'_> {
            let positions = self.positions.lock().unwrap();
            let mut index = self.current_index.lock().unwrap();
            let pos = if *index < positions.len() {
                positions[*index]
            } else {
                positions[positions.len() - 1]
            };
            *index += 1;
            ReadyFuture(Some(Ok(pos)))
        }

        fn sleep(&self, _duration: Duration) -> Self::Sleep<'_> {
            ReadyFuture(Some(()))
        }
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_pan_tilt_movement_detected_and_completed() {
        // Simulate camera moving from position (100, 50) to (200, 100)
        let positions = vec![
            PanTiltPosition { pan: 100, tilt: 50 }, // Initial
            PanTiltPosition { pan: 100, tilt: 50 }, // Still at initial
            PanTiltPosition { pan: 120, tilt: 60 }, // Movement starts
            PanTiltPosition { pan: 150, tilt: 75 }, // Moving
            PanTiltPosition { pan: 180, tilt: 90 }, // Moving
            PanTiltPosition {
                pan: 200,
                tilt: 100,
            }, // Reached target
            PanTiltPosition {
                pan: 200,
                tilt: 100,
            }, // Stable
            PanTiltPosition {
                pan: 200,
                tilt: 100,
            }, // Stable
            PanTiltPosition {
                pan: 200,
                tilt: 100,
            }, // Stable
        ];

        let probe = MockMovementProbe::new(positions);
        let config = MovementDetectionConfig::default();

        let result = wait_for_pan_tilt_completion(&probe, &config).await;
        assert!(result.is_ok());
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_pan_tilt_no_movement() {
        // Camera doesn't move at all
        let positions = vec![
            PanTiltPosition { pan: 100, tilt: 50 },
            PanTiltPosition { pan: 100, tilt: 50 },
            PanTiltPosition { pan: 100, tilt: 50 },
            PanTiltPosition { pan: 100, tilt: 50 },
            PanTiltPosition { pan: 100, tilt: 50 },
            PanTiltPosition { pan: 100, tilt: 50 },
        ];

        let probe = MockMovementProbe::new(positions);
        let mut config = MovementDetectionConfig::default();
        config.no_movement_timeout = Duration::from_millis(100);

        let result = wait_for_pan_tilt_completion(&probe, &config).await;
        assert!(result.is_ok()); // Should succeed due to no_movement_timeout
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_pan_tilt_oscillation_detection() {
        // Simulate oscillation between two positions
        let positions = vec![
            PanTiltPosition { pan: 100, tilt: 50 }, // Initial
            PanTiltPosition { pan: 120, tilt: 60 }, // Movement starts
            PanTiltPosition {
                pan: 200,
                tilt: 100,
            }, // Position A
            PanTiltPosition { pan: 195, tilt: 95 }, // Position B (oscillating)
            PanTiltPosition {
                pan: 200,
                tilt: 100,
            }, // Position A
            PanTiltPosition { pan: 195, tilt: 95 }, // Position B
            PanTiltPosition {
                pan: 200,
                tilt: 100,
            }, // Position A
            PanTiltPosition { pan: 195, tilt: 95 }, // Position B
            PanTiltPosition {
                pan: 200,
                tilt: 100,
            }, // Position A
            PanTiltPosition { pan: 195, tilt: 95 }, // Position B
        ];

        let probe = MockMovementProbe::new(positions);
        let mut config = MovementDetectionConfig::default();
        config.tolerance_pan_tilt_oscillation = 10;
        config.oscillation_min_samples = 6;

        let result = wait_for_pan_tilt_completion(&probe, &config).await;
        assert!(result.is_ok()); // Should detect oscillation and complete
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_zoom_movement_detected_and_completed() {
        // Simulate zoom from 1000 to 5000
        let positions = vec![
            1000, // Initial
            1000, // Still at initial
            1500, // Movement starts
            2500, // Moving
            3500, // Moving
            4500, // Moving
            5000, // Reached target
            5000, // Stable
            5000, // Stable
            5000, // Stable
        ];

        let probe = MockZoomProbe::new(positions);
        let config = MovementDetectionConfig::default();

        let result = wait_for_zoom_completion(&probe, &config).await;
        assert!(result.is_ok());
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_zoom_no_movement() {
        // Zoom doesn't change
        let positions = vec![2000, 2000, 2000, 2000, 2000, 2000];

        let probe = MockZoomProbe::new(positions);
        let mut config = MovementDetectionConfig::default();
        config.no_movement_timeout = Duration::from_millis(100);

        let result = wait_for_zoom_completion(&probe, &config).await;
        assert!(result.is_ok());
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_focus_movement_detected_and_completed() {
        // Simulate focus adjustment
        let positions = vec![
            3000, // Initial
            3000, // Still at initial
            3100, // Movement starts
            3300, // Moving
            3500, // Moving
            3700, // Moving
            4000, // Reached target
            4000, // Stable
            4000, // Stable
            4000, // Stable
        ];

        let probe = MockFocusProbe::new(positions);
        let config = MovementDetectionConfig::default();

        let result = wait_for_focus_completion(&probe, &config).await;
        assert!(result.is_ok());
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_all_movements_parallel() {
        // Test that all movements are detected in parallel
        let movement_positions = vec![
            PanTiltPosition { pan: 100, tilt: 50 },
            PanTiltPosition { pan: 150, tilt: 75 },
            PanTiltPosition {
                pan: 200,
                tilt: 100,
            },
            PanTiltPosition {
                pan: 200,
                tilt: 100,
            },
            PanTiltPosition {
                pan: 200,
                tilt: 100,
            },
        ];

        let zoom_positions = vec![1000, 2000, 3000, 3000, 3000];

        let focus_positions = vec![5000, 5500, 6000, 6000, 6000];

        let movement_probe = MockMovementProbe::new(movement_positions);
        let zoom_probe = MockZoomProbe::new(zoom_positions);
        let focus_probe = MockFocusProbe::new(focus_positions);
        let config = MovementDetectionConfig::default();

        let result =
            wait_for_all_movements(&movement_probe, &zoom_probe, &focus_probe, &config).await;
        assert!(result.is_ok());
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_timeout_error() {
        // Simulate movement that never stabilizes
        // We need enough positions to exceed the timeout iterations
        let mut positions = vec![];
        for i in 0..1000 {
            positions.push(PanTiltPosition {
                pan: 100 + i as i16 * 10,
                tilt: 50 + i as i16 * 5,
            });
        }

        let probe = MockMovementProbe::new(positions);
        let mut config = MovementDetectionConfig::default();
        config.timeout = Duration::from_millis(0); // Immediate timeout
        config.startup_delay = Duration::from_millis(0);
        config.poll_interval = Duration::from_millis(0);

        let result = wait_for_pan_tilt_completion(&probe, &config).await;
        assert!(matches!(result, Err(Error::Timeout)));
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_custom_tolerances() {
        // Test with custom tolerance values
        let positions = vec![
            PanTiltPosition { pan: 100, tilt: 50 },
            PanTiltPosition { pan: 102, tilt: 51 }, // Small movement within custom tolerance
            PanTiltPosition { pan: 103, tilt: 52 },
            PanTiltPosition { pan: 104, tilt: 52 },
            PanTiltPosition { pan: 104, tilt: 52 },
        ];

        let probe = MockMovementProbe::new(positions);
        let mut config = MovementDetectionConfig::default();
        config.tolerance_pan_start = 10; // Movement must exceed 10 to be detected
        config.tolerance_tilt_start = 10;
        config.tolerance_pan_stable = 5; // Position must be stable within 5
        config.tolerance_tilt_stable = 5;
        config.no_movement_timeout = Duration::from_millis(100);

        let result = wait_for_pan_tilt_completion(&probe, &config).await;
        assert!(result.is_ok()); // Should complete via no_movement_timeout since movement is within start tolerance
    }

    // Tests for helper functions
    #[test]
    fn test_positions_equal_within_tolerance() {
        let pos1 = PanTiltPosition { pan: 100, tilt: 50 };
        let pos2 = PanTiltPosition { pan: 102, tilt: 48 };

        assert!(positions_equal_within_tolerance(pos1, pos2, 2));
        assert!(!positions_equal_within_tolerance(pos1, pos2, 1));

        let pos3 = PanTiltPosition { pan: 105, tilt: 55 };
        assert!(positions_equal_within_tolerance(pos1, pos3, 5));
        assert!(!positions_equal_within_tolerance(pos1, pos3, 4));
    }

    #[test]
    fn test_positions_equal_within_tolerance_separate() {
        let pos1 = PanTiltPosition { pan: 100, tilt: 50 };
        let pos2 = PanTiltPosition { pan: 103, tilt: 52 };

        // Different tolerances for pan and tilt
        assert!(positions_equal_within_tolerance_separate(pos1, pos2, 3, 2));
        assert!(!positions_equal_within_tolerance_separate(pos1, pos2, 2, 2));
        assert!(!positions_equal_within_tolerance_separate(pos1, pos2, 3, 1));
    }

    #[test]
    fn test_zoom_equal_within_tolerance() {
        assert!(zoom_equal_within_tolerance(1000, 1005, 5));
        assert!(!zoom_equal_within_tolerance(1000, 1006, 5));
        assert!(zoom_equal_within_tolerance(5000, 4990, 10));
        assert!(!zoom_equal_within_tolerance(5000, 4989, 10));
    }

    #[test]
    fn test_movement_detection_config_default() {
        let config = MovementDetectionConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.poll_interval, Duration::from_millis(100));
        assert_eq!(config.startup_delay, Duration::from_millis(200));
        assert_eq!(config.stability_threshold, 3);
        assert!(!config.debug);
        assert_eq!(config.tolerance_pan_start, 5);
        assert_eq!(config.tolerance_tilt_start, 5);
        assert_eq!(config.tolerance_pan_stable, 2);
        assert_eq!(config.tolerance_tilt_stable, 2);
        assert_eq!(config.tolerance_pan_tilt_oscillation, 10);
        assert_eq!(config.tolerance_zoom_start, 20);
        assert_eq!(config.tolerance_zoom_stable, 10);
        assert_eq!(config.tolerance_focus_start, 10);
        assert_eq!(config.tolerance_focus_stable, 5);
        assert_eq!(config.oscillation_sample_size, 10);
        assert_eq!(config.oscillation_min_samples, 6);
        assert_eq!(config.no_movement_timeout, Duration::from_secs(2));
    }
}
