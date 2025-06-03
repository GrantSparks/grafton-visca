/// Conversion utilities for camera position values
use crate::camera_constants::ptzoptics_g2::*;

/// Convert normalized pan value (-1.0 to 1.0) to VISCA units
pub fn pan_normalized_to_visca(normalized: f32) -> i16 {
    (normalized * PAN_MAX as f32) as i16
}

/// Convert VISCA pan units to normalized value (-1.0 to 1.0)
pub fn pan_visca_to_normalized(visca: i16) -> f32 {
    visca as f32 / PAN_MAX as f32
}

/// Convert pan degrees to VISCA units
pub fn pan_degrees_to_visca(degrees: f32) -> i16 {
    let normalized = degrees / (PAN_RANGE_DEGREES / 2.0);
    pan_normalized_to_visca(normalized.clamp(-1.0, 1.0))
}

/// Convert VISCA pan units to degrees
pub fn pan_visca_to_degrees(visca: i16) -> f32 {
    let normalized = pan_visca_to_normalized(visca);
    normalized * (PAN_RANGE_DEGREES / 2.0)
}

/// Convert tilt normalized value (-1.0 to 1.0) to VISCA units
pub fn tilt_normalized_to_visca(normalized: f32) -> i16 {
    (normalized * TILT_MAX as f32) as i16
}

/// Convert VISCA tilt units to normalized value (-1.0 to 1.0)
pub fn tilt_visca_to_normalized(visca: i16) -> f32 {
    visca as f32 / TILT_MAX as f32
}

/// Convert tilt degrees to VISCA units
pub fn tilt_degrees_to_visca(degrees: f32) -> i16 {
    let normalized = degrees / (TILT_RANGE_DEGREES / 2.0);
    tilt_normalized_to_visca(normalized.clamp(-1.0, 1.0))
}

/// Convert VISCA tilt units to degrees
pub fn tilt_visca_to_degrees(visca: i16) -> f32 {
    let normalized = tilt_visca_to_normalized(visca);
    normalized * (TILT_RANGE_DEGREES / 2.0)
}

/// Convert zoom normalized value (0.0 to 1.0) to VISCA units
pub fn zoom_normalized_to_visca(normalized: f32) -> u16 {
    (normalized.clamp(0.0, 1.0) * ZOOM_MAX as f32) as u16
}

/// Convert VISCA zoom units to normalized value (0.0 to 1.0)
pub fn zoom_visca_to_normalized(visca: u16) -> f32 {
    visca as f32 / ZOOM_MAX as f32
}
