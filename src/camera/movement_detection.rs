//! Unified movement detection algorithms using GAT-based probes.
//!
//! This module contains the actual movement detection logic that works
//! with both blocking and async implementations through the probe traits.

use std::time::Instant;

use crate::error::Error;

use super::movement_probe::{
    positions_equal_within_tolerance, zoom_equal_within_tolerance, FocusProbe,
    MovementDetectionConfig, MovementProbe, ZoomProbe,
};

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

        if !has_moved && !positions_equal_within_tolerance(initial_position, current_position, 5) {
            has_moved = true;
            if config.debug {
                log::debug!("Pan/tilt movement started");
            }
        }

        if positions_equal_within_tolerance(last_position, current_position, 2) {
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
            if oscillation_detector.len() > 10 {
                oscillation_detector.remove(0);

                // Check for oscillation pattern
                if oscillation_detector.len() >= 6 {
                    let recent = &oscillation_detector[oscillation_detector.len() - 6..];
                    let mut oscillating = true;
                    for i in 0..3 {
                        if !positions_equal_within_tolerance(recent[i * 2], recent[0], 10)
                            || !positions_equal_within_tolerance(recent[i * 2 + 1], recent[1], 10)
                        {
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
        if !has_moved && start.elapsed() > std::time::Duration::from_secs(2) {
            if config.debug {
                log::debug!("No pan/tilt movement detected, camera likely already at position");
            }
            return Ok(());
        }

        last_position = current_position;
        probe.sleep(config.poll_interval).await;
    }
}

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

        if !has_moved && !zoom_equal_within_tolerance(initial_zoom, current_zoom, 20) {
            has_moved = true;
            if config.debug {
                log::debug!("Zoom movement started");
            }
        }

        if zoom_equal_within_tolerance(last_zoom, current_zoom, 10) {
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
        if !has_moved && start.elapsed() > std::time::Duration::from_secs(2) {
            if config.debug {
                log::debug!("No zoom movement detected, camera likely already at position");
            }
            return Ok(());
        }

        last_zoom = current_zoom;
        probe.sleep(config.poll_interval).await;
    }
}

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

        if !has_moved && (initial_focus as i32 - current_focus as i32).abs() > 10 {
            has_moved = true;
            if config.debug {
                log::debug!("Focus movement started");
            }
        }

        if (last_focus as i32 - current_focus as i32).abs() <= 5 {
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
        if !has_moved && start.elapsed() > std::time::Duration::from_secs(2) {
            if config.debug {
                log::debug!("No focus movement detected");
            }
            return Ok(());
        }

        last_focus = current_focus;
        probe.sleep(config.poll_interval).await;
    }
}
