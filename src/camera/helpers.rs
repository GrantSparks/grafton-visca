//! Helper methods for intelligent camera movement completion detection.

use std::time::{Duration, Instant};

use crate::{
    camera::Camera,
    capabilities::Profile,
    error::Error,
    transport::UnifiedTransport,
    units::{Degrees, Normalized},
};

/// Configuration for movement detection.
#[derive(Debug, Clone, Copy)]
pub struct MovementDetectionConfig {
    /// How long to wait for movement to start after issuing command
    pub startup_delay: Duration,
    /// Minimum time between position polls
    pub poll_interval: Duration,
    /// Number of consecutive stable readings required to confirm stop
    pub stability_threshold: u32,
    /// Maximum position change to be considered "stable" (for noise tolerance)
    pub position_tolerance: i16,
    /// Maximum zoom change to be considered "stable"
    pub zoom_tolerance: u16,
    /// Timeout for the entire operation
    pub timeout: Duration,
    /// Whether to log debug information
    pub debug: bool,
}

impl Default for MovementDetectionConfig {
    fn default() -> Self {
        Self {
            startup_delay: Duration::from_millis(200),
            poll_interval: Duration::from_millis(100),
            stability_threshold: 3,
            position_tolerance: 5, // ~0.3 degrees for most cameras
            zoom_tolerance: 50,    // Small zoom tolerance
            timeout: Duration::from_secs(30),
            debug: false,
        }
    }
}

impl MovementDetectionConfig {
    /// Config for fast movements
    pub fn fast() -> Self {
        Self {
            startup_delay: Duration::from_millis(100),
            poll_interval: Duration::from_millis(50),
            stability_threshold: 2,
            ..Default::default()
        }
    }

    /// Config for slow/precise movements
    pub fn precise() -> Self {
        Self {
            startup_delay: Duration::from_millis(300),
            poll_interval: Duration::from_millis(150),
            stability_threshold: 5,
            position_tolerance: 2,
            zoom_tolerance: 20,
            ..Default::default()
        }
    }

    /// Config for very small movements
    pub fn micro() -> Self {
        Self {
            startup_delay: Duration::from_millis(500),
            poll_interval: Duration::from_millis(200),
            stability_threshold: 4,
            position_tolerance: 1,
            zoom_tolerance: 10,
            timeout: Duration::from_secs(10),
            ..Default::default()
        }
    }
}

/// Helper methods for camera movement operations.
pub trait MovementHelpers: Sized {
    /// Wait for pan/tilt movement to complete with default config.
    fn wait_for_pan_tilt_completion(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementDetectionConfig {
            timeout,
            ..Default::default()
        };
        self.wait_for_pan_tilt_completion_with_config(&config)
    }

    /// Wait for pan/tilt movement to complete with custom config.
    fn wait_for_pan_tilt_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error>;

    /// Wait for zoom movement to complete with default config.
    fn wait_for_zoom_completion(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementDetectionConfig {
            timeout,
            ..Default::default()
        };
        self.wait_for_zoom_completion_with_config(&config)
    }

    /// Wait for zoom movement to complete with custom config.
    fn wait_for_zoom_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error>;

    /// Wait for focus to complete with default config.
    fn wait_for_focus_completion(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementDetectionConfig {
            timeout,
            ..Default::default()
        };
        self.wait_for_focus_completion_with_config(&config)
    }

    /// Wait for focus to complete with custom config.
    fn wait_for_focus_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error>;

    /// Move to position and wait for completion.
    fn move_to_position_and_wait(
        &self,
        pan: Degrees,
        tilt: Degrees,
        timeout: Duration,
    ) -> Result<(), Error>;

    /// Set zoom and wait for completion.
    fn set_zoom_and_wait(&self, zoom: Normalized, timeout: Duration) -> Result<(), Error>;

    /// Check if camera is currently moving.
    fn is_moving(&self) -> Result<bool, Error>;

    /// Wait for all movements to complete.
    fn wait_for_all_movements(&self, timeout: Duration) -> Result<(), Error>;
}

/// Async helper methods for camera movement operations.
#[cfg(feature = "async")]
pub trait MovementHelpersAsync: Sized {
    /// Wait for pan/tilt movement to complete with default config.
    async fn wait_for_pan_tilt_completion(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementDetectionConfig {
            timeout,
            ..Default::default()
        };
        self.wait_for_pan_tilt_completion_with_config(&config).await
    }

    /// Wait for pan/tilt movement to complete with custom config.
    async fn wait_for_pan_tilt_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error>;

    /// Wait for zoom movement to complete with default config.
    async fn wait_for_zoom_completion(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementDetectionConfig {
            timeout,
            ..Default::default()
        };
        self.wait_for_zoom_completion_with_config(&config).await
    }

    /// Wait for zoom movement to complete with custom config.
    async fn wait_for_zoom_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error>;

    /// Wait for focus to complete with default config.
    async fn wait_for_focus_completion(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementDetectionConfig {
            timeout,
            ..Default::default()
        };
        self.wait_for_focus_completion_with_config(&config).await
    }

    /// Wait for focus to complete with custom config.
    async fn wait_for_focus_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error>;

    /// Move to position and wait for completion.
    async fn move_to_position_and_wait(
        &self,
        pan: Degrees,
        tilt: Degrees,
        timeout: Duration,
    ) -> Result<(), Error>;

    /// Set zoom and wait for completion.
    async fn set_zoom_and_wait(&self, zoom: Normalized, timeout: Duration) -> Result<(), Error>;

    /// Check if camera is currently moving.
    async fn is_moving(&self) -> Result<bool, Error>;

    /// Wait for all movements to complete.
    async fn wait_for_all_movements(&self, timeout: Duration) -> Result<(), Error>;
}

// Helper function to check if positions are within tolerance
fn positions_equal_within_tolerance(pos1: (i16, i16), pos2: (i16, i16), tolerance: i16) -> bool {
    (pos1.0 - pos2.0).abs() <= tolerance && (pos1.1 - pos2.1).abs() <= tolerance
}

fn zoom_equal_within_tolerance(zoom1: u16, zoom2: u16, tolerance: u16) -> bool {
    if zoom1 > zoom2 {
        zoom1 - zoom2 <= tolerance
    } else {
        zoom2 - zoom1 <= tolerance
    }
}

// Blocking implementation
impl<P: Profile, T: UnifiedTransport> MovementHelpers for Camera<P, T> {
    fn wait_for_pan_tilt_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error> {
        use crate::camera::methods::inquiry::PanTiltInquiryOpsBlocking;

        let start = Instant::now();

        // Wait for movement to potentially start
        std::thread::sleep(config.startup_delay);

        // Get initial position after startup delay
        let initial_position = self.get_pan_tilt_position()?;
        let mut last_position = initial_position;
        let mut stable_count = 0;
        let mut has_moved = false;
        let mut oscillation_detector = Vec::new();

        loop {
            // Check timeout
            if start.elapsed() > config.timeout {
                if config.debug {
                    log::debug!("Pan/tilt movement timed out");
                }
                return Err(Error::Timeout);
            }

            // Poll current position
            let current_position = self.get_pan_tilt_position()?;

            // Check if camera has moved from initial position
            if !has_moved
                && !positions_equal_within_tolerance(
                    initial_position,
                    current_position,
                    config.position_tolerance,
                )
            {
                has_moved = true;
                if config.debug {
                    log::debug!(
                        "Pan/tilt movement detected: {initial_position:?} -> {current_position:?}"
                    );
                }
            }

            // Check for stability
            if positions_equal_within_tolerance(
                last_position,
                current_position,
                config.position_tolerance,
            ) {
                stable_count += 1;
                if stable_count >= config.stability_threshold {
                    // Extra check: ensure we're not in oscillation
                    if oscillation_detector.len() >= 2 {
                        let recent_positions: Vec<_> =
                            oscillation_detector.iter().rev().take(3).collect();

                        // Check if we're oscillating between positions
                        let unique_positions: std::collections::HashSet<_> =
                            recent_positions.iter().collect();

                        if unique_positions.len() == 1 {
                            // Truly stable
                            if config.debug {
                                log::debug!("Pan/tilt movement complete at: {current_position:?}");
                            }
                            return Ok(());
                        }
                    } else {
                        // Not enough history, consider it stable
                        if config.debug {
                            log::debug!("Pan/tilt movement complete at: {current_position:?}");
                        }
                        return Ok(());
                    }
                }
            } else {
                stable_count = 0;
                oscillation_detector.push(current_position);
                if oscillation_detector.len() > 10 {
                    oscillation_detector.remove(0);
                }
            }

            // If camera never moved and we've waited enough, consider it complete
            if !has_moved && start.elapsed() > Duration::from_secs(2) {
                if config.debug {
                    log::debug!("No pan/tilt movement detected, camera likely already at position");
                }
                return Ok(());
            }

            last_position = current_position;
            std::thread::sleep(config.poll_interval);
        }
    }

    fn wait_for_zoom_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error> {
        use crate::camera::methods::inquiry::InquiryOpsBlocking;

        let start = Instant::now();

        // Wait for movement to potentially start
        std::thread::sleep(config.startup_delay);

        let initial_zoom = self.get_zoom_position()?;
        let mut last_zoom = initial_zoom;
        let mut stable_count = 0;
        let mut has_moved = false;

        loop {
            if start.elapsed() > config.timeout {
                return Err(Error::Timeout);
            }

            let current_zoom = self.get_zoom_position()?;

            if !has_moved
                && !zoom_equal_within_tolerance(initial_zoom, current_zoom, config.zoom_tolerance)
            {
                has_moved = true;
                if config.debug {
                    log::debug!("Zoom movement detected: {initial_zoom} -> {current_zoom}");
                }
            }

            if zoom_equal_within_tolerance(last_zoom, current_zoom, config.zoom_tolerance) {
                stable_count += 1;
                if stable_count >= config.stability_threshold {
                    if config.debug {
                        log::debug!("Zoom movement complete at: {current_zoom}");
                    }
                    return Ok(());
                }
            } else {
                stable_count = 0;
            }

            if !has_moved && start.elapsed() > Duration::from_secs(2) {
                if config.debug {
                    log::debug!("No zoom movement detected, camera likely already at position");
                }
                return Ok(());
            }

            last_zoom = current_zoom;
            std::thread::sleep(config.poll_interval);
        }
    }

    fn wait_for_focus_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error> {
        use crate::camera::methods::inquiry::InquiryOpsBlocking;

        let start = Instant::now();

        // Focus can be slow to start
        std::thread::sleep(config.startup_delay);

        let initial_focus = self.get_focus_position()?;
        let mut last_focus = initial_focus;
        let mut stable_count = 0;
        let mut has_moved = false;

        loop {
            if start.elapsed() > config.timeout {
                return Err(Error::Timeout);
            }

            let current_focus = self.get_focus_position()?;

            if !has_moved && (current_focus as i32 - initial_focus as i32).abs() > 10 {
                has_moved = true;
                if config.debug {
                    log::debug!("Focus movement detected: {initial_focus} -> {current_focus}");
                }
            }

            if (current_focus as i32 - last_focus as i32).abs() <= 10 {
                stable_count += 1;
                if stable_count >= config.stability_threshold {
                    if config.debug {
                        log::debug!("Focus adjustment complete at: {current_focus}");
                    }
                    return Ok(());
                }
            } else {
                stable_count = 0;
            }

            if !has_moved && start.elapsed() > Duration::from_secs(2) {
                if config.debug {
                    log::debug!("No focus movement detected");
                }
                return Ok(());
            }

            last_focus = current_focus;
            std::thread::sleep(config.poll_interval);
        }
    }

    fn move_to_position_and_wait(
        &self,
        pan: Degrees,
        tilt: Degrees,
        timeout: Duration,
    ) -> Result<(), Error> {
        use crate::camera::methods::pan_tilt::PanTiltOpsBlocking;
        use crate::types::SpeedLevel;

        self.pan_tilt_absolute(pan, tilt, SpeedLevel::Medium)?;
        MovementHelpers::wait_for_pan_tilt_completion(self, timeout)?;
        Ok(())
    }

    fn set_zoom_and_wait(&self, zoom: Normalized, timeout: Duration) -> Result<(), Error> {
        use crate::camera::methods::zoom::ZoomOpsBlocking;

        self.zoom_absolute(zoom)?;
        MovementHelpers::wait_for_zoom_completion(self, timeout)?;
        Ok(())
    }

    fn is_moving(&self) -> Result<bool, Error> {
        use crate::camera::methods::inquiry::{InquiryOpsBlocking, PanTiltInquiryOpsBlocking};

        // Take two readings with a short delay
        let pos1_pt = self.get_pan_tilt_position()?;
        let pos1_zoom = self.get_zoom_position()?;
        let pos1_focus = self.get_focus_position()?;

        std::thread::sleep(Duration::from_millis(50));

        let pos2_pt = self.get_pan_tilt_position()?;
        let pos2_zoom = self.get_zoom_position()?;
        let pos2_focus = self.get_focus_position()?;

        // Check if any axis is moving
        let pt_moving = !positions_equal_within_tolerance(pos1_pt, pos2_pt, 2);
        let zoom_moving = !zoom_equal_within_tolerance(pos1_zoom, pos2_zoom, 10);
        let focus_moving = (pos1_focus as i32 - pos2_focus as i32).abs() > 5;

        Ok(pt_moving || zoom_moving || focus_moving)
    }

    fn wait_for_all_movements(&self, timeout: Duration) -> Result<(), Error> {
        let start = Instant::now();
        let check_interval = Duration::from_millis(200);

        // Initial delay to let movements start
        std::thread::sleep(Duration::from_millis(300));

        loop {
            if start.elapsed() > timeout {
                return Err(Error::Timeout);
            }

            if !MovementHelpers::is_moving(self)? {
                // Double-check after a short delay
                std::thread::sleep(Duration::from_millis(100));
                if !MovementHelpers::is_moving(self)? {
                    return Ok(());
                }
            }

            std::thread::sleep(check_interval);
        }
    }
}

// Async implementation
#[cfg(feature = "async")]
impl<P: Profile, T: UnifiedTransport> MovementHelpersAsync for Camera<P, T> {
    async fn wait_for_pan_tilt_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error> {
        use crate::camera::methods::inquiry::PanTiltInquiryOps;

        let start = Instant::now();

        // Wait for movement to potentially start
        tokio::time::sleep(config.startup_delay).await;

        let initial_position = self.get_pan_tilt_position().await?;
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

            let current_position = self.get_pan_tilt_position().await?;

            if !has_moved
                && !positions_equal_within_tolerance(
                    initial_position,
                    current_position,
                    config.position_tolerance,
                )
            {
                has_moved = true;
                if config.debug {
                    log::debug!(
                        "Pan/tilt movement detected: {initial_position:?} -> {current_position:?}"
                    );
                }
            }

            if positions_equal_within_tolerance(
                last_position,
                current_position,
                config.position_tolerance,
            ) {
                stable_count += 1;
                if stable_count >= config.stability_threshold {
                    // Check for oscillation
                    if oscillation_detector.len() >= 2 {
                        let recent_positions: Vec<_> =
                            oscillation_detector.iter().rev().take(3).collect();

                        let unique_positions: std::collections::HashSet<_> =
                            recent_positions.iter().collect();

                        if unique_positions.len() == 1 {
                            if config.debug {
                                log::debug!("Pan/tilt movement complete at: {current_position:?}");
                            }
                            return Ok(());
                        }
                    } else {
                        if config.debug {
                            log::debug!("Pan/tilt movement complete at: {current_position:?}");
                        }
                        return Ok(());
                    }
                }
            } else {
                stable_count = 0;
                oscillation_detector.push(current_position);
                if oscillation_detector.len() > 10 {
                    oscillation_detector.remove(0);
                }
            }

            if !has_moved && start.elapsed() > Duration::from_secs(2) {
                if config.debug {
                    log::debug!("No pan/tilt movement detected, camera likely already at position");
                }
                return Ok(());
            }

            last_position = current_position;
            tokio::time::sleep(config.poll_interval).await;
        }
    }

    async fn wait_for_zoom_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error> {
        use crate::camera::methods::inquiry::InquiryOps;

        let start = Instant::now();

        tokio::time::sleep(config.startup_delay).await;

        let initial_zoom = self.get_zoom_position().await?;
        let mut last_zoom = initial_zoom;
        let mut stable_count = 0;
        let mut has_moved = false;

        loop {
            if start.elapsed() > config.timeout {
                return Err(Error::Timeout);
            }

            let current_zoom = self.get_zoom_position().await?;

            if !has_moved
                && !zoom_equal_within_tolerance(initial_zoom, current_zoom, config.zoom_tolerance)
            {
                has_moved = true;
                if config.debug {
                    log::debug!("Zoom movement detected: {initial_zoom} -> {current_zoom}");
                }
            }

            if zoom_equal_within_tolerance(last_zoom, current_zoom, config.zoom_tolerance) {
                stable_count += 1;
                if stable_count >= config.stability_threshold {
                    if config.debug {
                        log::debug!("Zoom movement complete at: {current_zoom}");
                    }
                    return Ok(());
                }
            } else {
                stable_count = 0;
            }

            if !has_moved && start.elapsed() > Duration::from_secs(2) {
                if config.debug {
                    log::debug!("No zoom movement detected, camera likely already at position");
                }
                return Ok(());
            }

            last_zoom = current_zoom;
            tokio::time::sleep(config.poll_interval).await;
        }
    }

    async fn wait_for_focus_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error> {
        use crate::camera::methods::inquiry::InquiryOps;

        let start = Instant::now();

        tokio::time::sleep(config.startup_delay).await;

        let initial_focus = self.get_focus_position().await?;
        let mut last_focus = initial_focus;
        let mut stable_count = 0;
        let mut has_moved = false;

        loop {
            if start.elapsed() > config.timeout {
                return Err(Error::Timeout);
            }

            let current_focus = self.get_focus_position().await?;

            if !has_moved && (current_focus as i32 - initial_focus as i32).abs() > 10 {
                has_moved = true;
                if config.debug {
                    log::debug!("Focus movement detected: {initial_focus} -> {current_focus}");
                }
            }

            if (current_focus as i32 - last_focus as i32).abs() <= 10 {
                stable_count += 1;
                if stable_count >= config.stability_threshold {
                    if config.debug {
                        log::debug!("Focus adjustment complete at: {current_focus}");
                    }
                    return Ok(());
                }
            } else {
                stable_count = 0;
            }

            if !has_moved && start.elapsed() > Duration::from_secs(2) {
                if config.debug {
                    log::debug!("No focus movement detected");
                }
                return Ok(());
            }

            last_focus = current_focus;
            tokio::time::sleep(config.poll_interval).await;
        }
    }

    async fn move_to_position_and_wait(
        &self,
        pan: Degrees,
        tilt: Degrees,
        timeout: Duration,
    ) -> Result<(), Error> {
        use crate::camera::methods::pan_tilt::PanTiltOps;
        use crate::types::SpeedLevel;

        self.pan_tilt_absolute(pan, tilt, SpeedLevel::Medium)
            .await?;
        MovementHelpersAsync::wait_for_pan_tilt_completion(self, timeout).await?;
        Ok(())
    }

    async fn set_zoom_and_wait(&self, zoom: Normalized, timeout: Duration) -> Result<(), Error> {
        use crate::camera::methods::zoom::ZoomOps;

        self.zoom_absolute(zoom).await?;
        MovementHelpersAsync::wait_for_zoom_completion(self, timeout).await?;
        Ok(())
    }

    async fn is_moving(&self) -> Result<bool, Error> {
        use crate::camera::methods::inquiry::{InquiryOps, PanTiltInquiryOps};

        let pos1_pt = self.get_pan_tilt_position().await?;
        let pos1_zoom = self.get_zoom_position().await?;
        let pos1_focus = self.get_focus_position().await?;

        tokio::time::sleep(Duration::from_millis(50)).await;

        let pos2_pt = self.get_pan_tilt_position().await?;
        let pos2_zoom = self.get_zoom_position().await?;
        let pos2_focus = self.get_focus_position().await?;

        let pt_moving = !positions_equal_within_tolerance(pos1_pt, pos2_pt, 2);
        let zoom_moving = !zoom_equal_within_tolerance(pos1_zoom, pos2_zoom, 10);
        let focus_moving = (pos1_focus as i32 - pos2_focus as i32).abs() > 5;

        Ok(pt_moving || zoom_moving || focus_moving)
    }

    async fn wait_for_all_movements(&self, timeout: Duration) -> Result<(), Error> {
        let start = Instant::now();
        let check_interval = Duration::from_millis(200);

        tokio::time::sleep(Duration::from_millis(300)).await;

        loop {
            if start.elapsed() > timeout {
                return Err(Error::Timeout);
            }

            if !MovementHelpersAsync::is_moving(self).await? {
                tokio::time::sleep(Duration::from_millis(100)).await;
                if !MovementHelpersAsync::is_moving(self).await? {
                    return Ok(());
                }
            }

            tokio::time::sleep(check_interval).await;
        }
    }
}
