//! Structured capabilities response for runtime feature discovery.
//!
//! This module provides a unified `Capabilities` struct that exposes
//! camera capabilities at runtime, complementing the compile-time
//! trait-based capability system.

use std::ops::RangeInclusive;

use super::profile_metadata::InquirySupport;
use crate::command::exposure::ExposureMode;

/// Structured capabilities response for runtime feature discovery.
///
/// This struct provides a runtime-queryable representation of all camera
/// capabilities. It's populated from the compile-time trait constants and
/// cached after camera initialization for efficient access.
///
/// # Example
/// ```ignore
/// let camera = Connect::builder()
///     .tcp("192.168.0.10")
///     .with_default_port()
///     .open::<PtzOpticsG2>()?;
///
/// let caps = camera.capabilities();
///
/// // Use capabilities for UI configuration
/// println!("Pan speed range: {:?}", caps.pan_speed);
/// println!("Zoom range: 0x0000-0x{:04X}", caps.zoom_range_optical.end());
///
/// if caps.has_digital_zoom {
///     println!("Digital zoom supported up to 0x{:04X}",
///              caps.zoom_range_digital.unwrap().end());
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct Capabilities {
    // Camera identification
    /// Model name of the camera.
    pub model_name: String,

    /// Default camera ID for VISCA addressing.
    pub default_camera_id: u8,

    // Network configuration
    /// Default TCP port for this camera model.
    pub default_tcp_port: u16,

    /// Default UDP port for this camera model.
    pub default_udp_port: u16,

    // Pan/Tilt capabilities
    /// Whether camera supports pan/tilt movement.
    pub has_pan_tilt: bool,

    /// Valid pan speed range (typically 1-24).
    pub pan_speed: RangeInclusive<u8>,

    /// Valid tilt speed range (typically 1-20).
    pub tilt_speed: RangeInclusive<u8>,

    /// Pan position range in VISCA units.
    pub pan_range: RangeInclusive<i16>,

    /// Tilt position range in VISCA units.
    pub tilt_range: RangeInclusive<i16>,

    /// Pan position range in degrees.
    pub pan_range_degrees: RangeInclusive<f32>,

    /// Tilt position range in degrees.
    pub tilt_range_degrees: RangeInclusive<f32>,

    /// Whether camera can pan and tilt simultaneously.
    pub pan_tilt_simultaneous: bool,

    // Zoom capabilities
    /// Whether camera supports zoom operations.
    pub has_zoom: bool,

    /// Whether camera supports digital zoom beyond optical.
    pub has_digital_zoom: bool,

    /// Optical zoom range in VISCA units (0x0000 to max).
    pub zoom_range_optical: RangeInclusive<u16>,

    /// Digital zoom range if supported (optical_max to digital_max).
    pub zoom_range_digital: Option<RangeInclusive<u16>>,

    /// Valid zoom speed range (typically 0-7).
    pub zoom_speed: RangeInclusive<u8>,

    /// Whether camera supports direct zoom positioning.
    pub supports_direct_zoom: bool,

    /// Whether camera supports variable speed zoom.
    pub supports_variable_zoom: bool,

    /// Conversion factor from magnification to VISCA units.
    ///
    /// This value represents the number of VISCA units per 1x of magnification.
    /// For example, a 20x camera with `OPTICAL_ZOOM_MAX = 0x4000` has
    /// `zoom_magnification_to_units ≈ 862.3` because `0x4000 / 19 ≈ 862.3`.
    ///
    /// Use with helper methods:
    /// - [`zoom_units_to_magnification`](Self::zoom_units_to_magnification) to convert VISCA units to magnification
    /// - [`magnification_to_zoom_units`](Self::magnification_to_zoom_units) to convert magnification to VISCA units
    /// - [`max_optical_zoom`](Self::max_optical_zoom) to get the maximum optical zoom magnification
    pub zoom_magnification_to_units: f32,

    // Focus capabilities
    /// Whether camera supports focus control.
    pub has_focus: bool,

    /// Whether camera supports auto-focus mode.
    pub has_auto_focus: bool,

    /// Whether camera supports one-push auto-focus.
    pub has_one_push_focus: bool,

    /// Focus position range in VISCA units.
    pub focus_range: RangeInclusive<u16>,

    /// Valid focus speed range.
    pub focus_speed: RangeInclusive<u8>,

    /// Whether camera supports focus zone selection.
    pub has_focus_zone: bool,

    /// Whether camera supports auto focus sensitivity adjustment.
    pub has_af_sensitivity: bool,

    /// Whether camera supports the focus near limit inquiry command.
    pub has_focus_near_limit_inquiry: bool,

    // Exposure capabilities
    /// Whether camera supports exposure control.
    pub has_exposure: bool,

    /// Whether camera supports direct iris control.
    pub has_iris_control: bool,

    /// Supported exposure modes for this profile.
    pub exposure_modes: Vec<ExposureMode>,

    /// Whether camera supports backlight compensation.
    pub has_backlight_comp: bool,

    /// Whether camera supports WDR (Wide Dynamic Range).
    pub has_wdr: bool,

    /// Whether camera supports exposure compensation.
    pub has_exposure_comp: bool,

    /// Iris range in VISCA units, if iris control is supported.
    pub iris_range: Option<RangeInclusive<u16>>,

    /// Gain range in VISCA units.
    pub gain_range: RangeInclusive<u8>,

    /// Number of supported shutter speeds.
    pub shutter_speed_count: usize,

    // White balance capabilities
    /// Whether camera supports white balance control.
    pub has_white_balance: bool,

    /// Whether camera supports one-push white balance.
    pub has_one_push_wb: bool,

    /// Whether camera supports color temperature control.
    pub has_color_temp: bool,

    /// Color temperature range if supported (in Kelvin).
    pub color_temp_range: Option<RangeInclusive<u16>>,

    /// RG tuning range if supported.
    pub rg_tuning_range: Option<RangeInclusive<i8>>,

    /// BG tuning range if supported.
    pub bg_tuning_range: Option<RangeInclusive<i8>>,

    /// Whether camera supports manual RGB gain control (red/blue gain inquiries).
    pub has_rgb_gain: bool,

    /// Number of white balance modes supported.
    pub wb_mode_count: usize,

    // Image processing capabilities
    /// Whether camera supports image processing features.
    pub has_image_processing: bool,

    /// Brightness adjustment range.
    pub brightness_range: RangeInclusive<u8>,

    /// Contrast adjustment range.
    pub contrast_range: RangeInclusive<u8>,

    /// Sharpness adjustment range.
    pub sharpness_range: RangeInclusive<u8>,

    /// Saturation adjustment range if supported.
    pub saturation_range: Option<RangeInclusive<u8>>,

    /// Hue adjustment range if supported.
    pub hue_range: Option<RangeInclusive<u8>>,

    /// Whether camera supports image flip.
    pub supports_flip: bool,

    /// Whether camera supports image mirror.
    pub supports_mirror: bool,

    /// Whether camera supports noise reduction.
    pub has_noise_reduction: bool,

    /// Whether camera supports 2D noise reduction.
    pub has_2d_nr: bool,

    /// Whether camera supports 3D noise reduction.
    pub has_3d_nr: bool,

    /// Whether camera supports picture effect modes (negative, B&W, sepia, etc.).
    pub has_picture_effect: bool,

    /// Whether camera supports gamma curve control.
    pub has_gamma: bool,

    /// Whether camera supports luminance (brightness) control.
    pub has_luminance: bool,

    /// Whether camera supports tally light control.
    pub has_tally: bool,

    // Preset capabilities
    /// Whether camera supports preset positions.
    pub has_presets: bool,

    /// Maximum number of preset positions.
    pub max_presets: u8,

    /// Preset recall speed range.
    pub preset_speed_range: RangeInclusive<u8>,

    /// Whether camera supports preset tours.
    pub supports_preset_tour: bool,

    /// Whether camera supports preset thumbnails.
    pub supports_preset_thumbnail: bool,

    // Power capabilities
    /// Whether camera supports power control.
    pub has_power: bool,

    /// Whether camera supports standby mode.
    pub supports_standby: bool,

    /// Whether camera supports wake-on-LAN.
    pub supports_wake_on_lan: bool,

    /// Time required for power-on sequence (in seconds).
    pub power_on_time_secs: u64,

    // Special capabilities
    /// Whether camera has ND filter support.
    pub has_nd_filter: bool,

    /// ND filter type if supported.
    pub nd_filter_type: Option<String>,

    /// Whether camera supports motion sync.
    pub has_motion_sync: bool,

    /// Maximum motion sync speed if supported.
    pub max_motion_sync_speed: Option<u8>,

    /// Whether camera supports direct menu control.
    pub has_direct_menu_control: bool,

    /// Whether camera supports variable speed control.
    pub has_variable_speed: bool,

    // Protocol features
    /// Level of VISCA inquiry command support for this camera.
    ///
    /// Use this instead of matching on `ProfileGroup` to determine whether
    /// inquiry commands are available. See [`InquirySupport`] for details.
    pub inquiry_support: InquirySupport,

    /// Whether camera sends operation complete messages.
    pub supports_operation_complete: bool,
}

impl Capabilities {
    /// Creates a new Capabilities struct from a camera profile.
    ///
    /// This method extracts all capability information from the compile-time
    /// trait constants and creates a runtime-queryable struct.
    pub fn from_profile<P>() -> Self
    where
        P: crate::capabilities::Profile,
    {
        // Extract pan/tilt capabilities
        let pan_min_deg = P::PAN_RANGE.start as f32 / P::PAN_DEGREES_TO_UNITS;
        let pan_max_deg = (P::PAN_RANGE.end - 1) as f32 / P::PAN_DEGREES_TO_UNITS;
        let tilt_min_deg = P::TILT_RANGE.start as f32 / P::TILT_DEGREES_TO_UNITS;
        let tilt_max_deg = (P::TILT_RANGE.end - 1) as f32 / P::TILT_DEGREES_TO_UNITS;

        let pan_speed = 1..=P::MAX_PAN_SPEED;
        let tilt_speed = 1..=P::MAX_TILT_SPEED;
        let pan_range = P::PAN_RANGE.start..=(P::PAN_RANGE.end - 1);
        let tilt_range = P::TILT_RANGE.start..=(P::TILT_RANGE.end - 1);
        let pan_range_degrees = pan_min_deg..=pan_max_deg;
        let tilt_range_degrees = tilt_min_deg..=tilt_max_deg;
        let pan_tilt_simultaneous = P::PAN_TILT_SIMULTANEOUS;

        // Extract zoom capabilities
        let has_digital_zoom = P::DIGITAL_ZOOM_MAX.is_some();
        let zoom_range_digital = P::DIGITAL_ZOOM_MAX.map(|max| P::OPTICAL_ZOOM_MAX..=max);

        // Extract focus capabilities
        let focus_range = P::FOCUS_NEAR_LIMIT..=P::FOCUS_FAR_LIMIT;
        let focus_speed = 0..=7; // Standard VISCA focus speed range

        // Extract exposure capabilities
        let has_iris_control = P::IRIS_RANGE.is_some();
        let exposure_modes = P::EXPOSURE_MODES.to_vec();
        let iris_range = P::IRIS_RANGE
            .as_ref()
            .map(|range| range.start..=(range.end - 1));
        let gain_range = P::GAIN_RANGE.start..=(P::GAIN_RANGE.end - 1);

        // Extract white balance capabilities
        let color_temp_range = P::COLOR_TEMP_RANGE.as_ref().map(|r| r.start..=(r.end - 1));
        let rg_tuning_range = P::RG_TUNING_RANGE.as_ref().map(|r| r.start..=(r.end - 1));
        let bg_tuning_range = P::BG_TUNING_RANGE.as_ref().map(|r| r.start..=(r.end - 1));

        // Extract image processing capabilities
        let brightness_range = P::BRIGHTNESS_RANGE.start..=(P::BRIGHTNESS_RANGE.end - 1);
        let contrast_range = P::CONTRAST_RANGE.start..=(P::CONTRAST_RANGE.end - 1);
        let sharpness_range = P::SHARPNESS_RANGE.start..=(P::SHARPNESS_RANGE.end - 1);
        let saturation_range = P::SATURATION_RANGE.as_ref().map(|r| r.start..=(r.end - 1));
        let hue_range = P::HUE_RANGE.as_ref().map(|r| r.start..=(r.end - 1));

        // Extract preset capabilities
        let preset_speed_range = P::PRESET_SPEED_RANGE.start..=(P::PRESET_SPEED_RANGE.end - 1);

        // Extract ND filter capabilities from the NdFilter trait
        let has_nd_filter = !matches!(P::ND_MODE, crate::capabilities::NdFilterMode::None);
        let nd_filter_type = match P::ND_MODE {
            crate::capabilities::NdFilterMode::None => None,
            crate::capabilities::NdFilterMode::Fixed(_) => Some("Fixed ND filter".to_string()),
            crate::capabilities::NdFilterMode::Stepped(_) => Some("Stepped ND filter".to_string()),
            crate::capabilities::NdFilterMode::Variable => Some("Variable ND filter".to_string()),
        };

        // Extract motion sync capabilities from the MotionSync trait
        let has_motion_sync = P::SUPPORTS_MOTION_SYNC;
        let max_motion_sync_speed = if has_motion_sync {
            Some(P::MAX_MOTION_SYNC_SPEED)
        } else {
            None
        };

        Self {
            // Camera identification
            model_name: P::MODEL_NAME.to_string(),
            default_camera_id: P::DEFAULT_CAMERA_ID,

            // Network configuration
            default_tcp_port: P::DEFAULT_TCP_PORT,
            default_udp_port: P::DEFAULT_UDP_PORT,

            // Pan/Tilt capabilities
            has_pan_tilt: true, // All cameras in Profile have pan/tilt
            pan_speed,
            tilt_speed,
            pan_range,
            tilt_range,
            pan_range_degrees,
            tilt_range_degrees,
            pan_tilt_simultaneous,

            // Zoom capabilities
            has_zoom: true, // All cameras have zoom
            has_digital_zoom,
            zoom_range_optical: 0x0000..=P::OPTICAL_ZOOM_MAX,
            zoom_range_digital,
            zoom_speed: P::ZOOM_SPEED_RANGE.start..=(P::ZOOM_SPEED_RANGE.end - 1),
            supports_direct_zoom: P::SUPPORTS_DIRECT_ZOOM,
            supports_variable_zoom: P::SUPPORTS_VARIABLE_ZOOM,
            zoom_magnification_to_units: P::ZOOM_MAGNIFICATION_TO_UNITS,

            // Focus capabilities
            has_focus: true, // All cameras have focus
            has_auto_focus: P::SUPPORTS_AUTO_FOCUS,
            has_one_push_focus: P::SUPPORTS_ONE_PUSH_FOCUS,
            focus_range,
            focus_speed,
            has_focus_zone: P::SUPPORTS_FOCUS_ZONE,
            has_af_sensitivity: P::SUPPORTS_AF_SENSITIVITY,
            has_focus_near_limit_inquiry: P::SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY,

            // Exposure capabilities
            has_exposure: true, // All cameras have exposure control
            has_iris_control,
            exposure_modes,
            has_backlight_comp: P::SUPPORTS_BACKLIGHT_COMP,
            has_wdr: P::SUPPORTS_WDR,
            has_exposure_comp: P::SUPPORTS_EXPOSURE_COMP,
            iris_range,
            gain_range,
            shutter_speed_count: P::SHUTTER_SPEEDS.len(),

            // White balance capabilities
            has_white_balance: true, // All cameras have white balance
            has_one_push_wb: P::SUPPORTS_ONE_PUSH_WB,
            has_color_temp: P::SUPPORTS_COLOR_TEMP,
            color_temp_range,
            rg_tuning_range,
            bg_tuning_range,
            has_rgb_gain: P::SUPPORTS_RGB_GAIN,
            wb_mode_count: P::WB_MODES.len(),

            // Image processing capabilities
            has_image_processing: true, // All cameras have some image processing
            brightness_range,
            contrast_range,
            sharpness_range,
            saturation_range,
            hue_range,
            supports_flip: P::SUPPORTS_FLIP,
            supports_mirror: P::SUPPORTS_MIRROR,
            has_noise_reduction: P::SUPPORTS_NOISE_REDUCTION,
            has_2d_nr: P::SUPPORTS_2D_NR,
            has_3d_nr: P::SUPPORTS_3D_NR,
            has_picture_effect: P::SUPPORTS_PICTURE_EFFECT,
            has_gamma: P::SUPPORTS_GAMMA,
            has_luminance: P::SUPPORTS_LUMINANCE,
            has_tally: P::SUPPORTS_TALLY,

            // Preset capabilities
            has_presets: true, // All cameras have presets
            max_presets: P::MAX_PRESETS,
            preset_speed_range,
            supports_preset_tour: P::SUPPORTS_PRESET_TOUR,
            supports_preset_thumbnail: P::SUPPORTS_PRESET_THUMBNAIL,

            // Power capabilities
            has_power: true, // All cameras have power control
            supports_standby: P::SUPPORTS_STANDBY,
            supports_wake_on_lan: P::SUPPORTS_WAKE_ON_LAN,
            power_on_time_secs: P::POWER_ON_TIME.as_secs(),

            // Special capabilities
            has_nd_filter,
            nd_filter_type,
            has_motion_sync,
            max_motion_sync_speed,
            has_direct_menu_control: P::SUPPORTS_DIRECT_CONTROL,
            has_variable_speed: P::SUPPORTS_VARIABLE_SPEED,

            // Protocol features
            inquiry_support: P::INQUIRY_SUPPORT,
            supports_operation_complete: P::SUPPORTS_OPERATION_COMPLETE,
        }
    }

    /// Returns true if the camera supports all VISCA inquiry commands.
    ///
    /// This is a convenience method that checks [`InquirySupport::Full`].
    /// Consumers should use this instead of matching on `ProfileGroup` to
    /// determine whether inquiry commands are safe to use without fallbacks.
    #[must_use]
    pub fn has_full_inquiry_support(&self) -> bool {
        self.inquiry_support == InquirySupport::Full
    }

    /// Returns true if the camera has all basic features.
    ///
    /// Basic features include: pan/tilt, zoom, focus, exposure, white balance,
    /// image processing, presets, and power control.
    pub fn has_basic_features(&self) -> bool {
        self.has_pan_tilt
            && self.has_zoom
            && self.has_focus
            && self.has_exposure
            && self.has_white_balance
            && self.has_image_processing
            && self.has_presets
            && self.has_power
    }

    /// Returns true if the camera has advanced features.
    ///
    /// Advanced features include: digital zoom, one-push focus/WB, WDR,
    /// color temperature control, preset tours, etc.
    pub fn has_advanced_features(&self) -> bool {
        self.has_digital_zoom
            || self.has_one_push_focus
            || self.has_one_push_wb
            || self.has_wdr
            || self.has_color_temp
            || self.supports_preset_tour
            || self.has_nd_filter
            || self.has_motion_sync
    }

    /// Returns true if the camera supports the requested exposure mode.
    #[must_use]
    pub fn supports_exposure_mode(&self, mode: ExposureMode) -> bool {
        self.exposure_modes.contains(&mode)
    }

    /// Returns the maximum optical zoom magnification (e.g., 20.0 for 20x).
    ///
    /// This is calculated from the optical zoom range and the magnification
    /// conversion factor.
    ///
    /// # Example
    /// ```ignore
    /// let caps = Capabilities::from_profile::<PtzOpticsG2>();
    /// let max_zoom = caps.max_optical_zoom(); // ~20.0 for a 20x camera
    /// println!("Max optical zoom: {:.1}x", max_zoom);
    /// ```
    #[must_use]
    pub fn max_optical_zoom(&self) -> f32 {
        self.zoom_units_to_magnification(*self.zoom_range_optical.end())
    }

    /// Returns the maximum combined zoom magnification (optical + digital).
    ///
    /// If the camera supports digital zoom, this returns the maximum digital
    /// zoom magnification. Otherwise, it returns the maximum optical zoom.
    ///
    /// # Example
    /// ```ignore
    /// let caps = Capabilities::from_profile::<PtzOpticsG2>();
    /// let max_combined = caps.max_combined_zoom();
    /// if caps.has_digital_zoom {
    ///     println!("Max combined zoom: {:.1}x (includes digital)", max_combined);
    /// }
    /// ```
    #[must_use]
    pub fn max_combined_zoom(&self) -> f32 {
        match &self.zoom_range_digital {
            Some(range) => self.zoom_units_to_magnification(*range.end()),
            None => self.max_optical_zoom(),
        }
    }

    /// Convert VISCA zoom units to magnification (e.g., 0x2000 → ~10x).
    ///
    /// # Arguments
    /// * `units` - Zoom position in VISCA units
    ///
    /// # Returns
    /// The zoom magnification, where 1.0 is no zoom and higher values
    /// represent greater magnification.
    ///
    /// # Example
    /// ```ignore
    /// let caps = Capabilities::from_profile::<PtzOpticsG2>();
    /// let magnification = caps.zoom_units_to_magnification(0x2000);
    /// println!("Current zoom: {:.1}x", magnification); // ~10x
    /// ```
    #[must_use]
    pub fn zoom_units_to_magnification(&self, units: u16) -> f32 {
        1.0 + (units as f32 / self.zoom_magnification_to_units)
    }

    /// Convert magnification to VISCA zoom units (e.g., 10x → ~0x2000).
    ///
    /// # Arguments
    /// * `magnification` - Desired zoom magnification (must be >= 1.0)
    ///
    /// # Returns
    /// The zoom position in VISCA units. Values below 1.0 are clamped to 0 units.
    ///
    /// # Example
    /// ```ignore
    /// let caps = Capabilities::from_profile::<PtzOpticsG2>();
    /// let units = caps.magnification_to_zoom_units(10.0);
    /// println!("10x zoom = 0x{:04X} units", units); // ~0x2000
    /// ```
    #[must_use]
    pub fn magnification_to_zoom_units(&self, magnification: f32) -> u16 {
        if magnification <= 1.0 {
            return 0;
        }
        ((magnification - 1.0) * self.zoom_magnification_to_units) as u16
    }

    /// Returns a summary of key capabilities as a formatted string.
    pub fn summary(&self) -> String {
        let mut lines = vec![
            format!("Model: {}", self.model_name),
            format!(
                "Pan Range: {:?}° ({:?} units)",
                self.pan_range_degrees, self.pan_range
            ),
            format!(
                "Tilt Range: {:?}° ({:?} units)",
                self.tilt_range_degrees, self.tilt_range
            ),
            format!(
                "Optical Zoom: {:.0}x (0x{:04X} units)",
                self.max_optical_zoom(),
                self.zoom_range_optical.end()
            ),
        ];

        if let Some(digital) = &self.zoom_range_digital {
            lines.push(format!(
                "Digital Zoom: {:.0}x (0x{:04X} units)",
                self.zoom_units_to_magnification(*digital.end()),
                digital.end()
            ));
        }

        lines.push(format!("Max Presets: {}", self.max_presets));

        if self.has_nd_filter {
            lines.push(format!(
                "ND Filter: {}",
                self.nd_filter_type.as_ref().unwrap_or(&"Yes".to_string())
            ));
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use crate::camera::profiles::{GenericVisca, PtzOpticsG2, SonyFR7};

    use super::*;

    #[test]
    fn test_capabilities_from_ptzoptics_g2() {
        let caps = Capabilities::from_profile::<PtzOpticsG2>();

        assert_eq!(caps.model_name, "PtzOptics G2");
        assert_eq!(caps.default_camera_id, 1);
        assert_eq!(caps.default_tcp_port, 5678);
        assert_eq!(caps.default_udp_port, 1259);

        assert!(caps.has_pan_tilt);
        assert_eq!(caps.pan_speed, 1..=24);
        assert_eq!(caps.tilt_speed, 1..=20);

        assert!(caps.has_zoom);
        assert!(!caps.has_digital_zoom);
        assert_eq!(caps.zoom_range_optical, 0x0000..=0x4000);
        assert_eq!(caps.zoom_range_digital, None);

        assert!(caps.has_auto_focus);
        assert!(!caps.has_one_push_focus);
        assert!(!caps.has_focus_zone);
        assert!(!caps.has_af_sensitivity);
        assert!(!caps.has_focus_near_limit_inquiry);
        assert!(!caps.has_iris_control);
        assert!(!caps.supports_exposure_mode(ExposureMode::Iris));
        assert_eq!(caps.iris_range, None);
        assert!(caps.has_rgb_gain);

        assert_eq!(caps.max_presets, 127);
        assert!(!caps.supports_preset_tour);

        assert!(caps.has_basic_features());
    }

    #[test]
    fn test_capabilities_from_sony_fr7() {
        let caps = Capabilities::from_profile::<SonyFR7>();

        assert_eq!(caps.model_name, "Sony FR7");
        assert_eq!(caps.default_tcp_port, 52381);
        assert_eq!(caps.default_udp_port, 52381);

        assert_eq!(caps.max_presets, 255);
        assert!(caps.supports_preset_tour);
        assert!(caps.has_focus_zone);
        assert!(caps.has_af_sensitivity);
        assert!(caps.has_focus_near_limit_inquiry);
        assert!(caps.has_rgb_gain);

        assert!(caps.has_advanced_features());
    }

    #[test]
    fn test_capabilities_from_generic() {
        let caps = Capabilities::from_profile::<GenericVisca>();

        assert_eq!(caps.model_name, "Generic VISCA Camera");
        assert!(!caps.has_digital_zoom);
        assert!(!caps.supports_direct_zoom);
        assert_eq!(caps.max_presets, 6);
        assert!(caps.has_iris_control);
        assert!(caps.supports_exposure_mode(ExposureMode::Iris));

        assert!(caps.has_basic_features());
        // GenericVisca has one-push white balance, which counts as an advanced feature
        assert!(caps.has_one_push_wb);
        assert!(caps.has_advanced_features());
    }

    #[test]
    fn test_capabilities_summary() {
        let caps = Capabilities::from_profile::<PtzOpticsG2>();
        let summary = caps.summary();

        assert!(summary.contains("Model: PtzOptics G2"));
        assert!(summary.contains("Optical Zoom: 20x"));
        assert!(summary.contains("0x4000"));
        assert!(!summary.contains("0x7000"));
        assert!(summary.contains("Max Presets: 127"));
    }

    #[test]
    fn test_zoom_magnification_field() {
        // PtzOpticsG2: 20x optical zoom with ZOOM_MAGNIFICATION_TO_UNITS = 862.3
        let caps = Capabilities::from_profile::<PtzOpticsG2>();
        assert!((caps.zoom_magnification_to_units - 862.3).abs() < 0.01);

        // GenericVisca: ZOOM_MAGNIFICATION_TO_UNITS = 1000.0
        let caps = Capabilities::from_profile::<GenericVisca>();
        assert!((caps.zoom_magnification_to_units - 1000.0).abs() < 0.01);
    }

    #[test]
    fn test_max_optical_zoom() {
        // PtzOpticsG2: 0x4000 / 862.3 + 1 ≈ 20x
        let caps = Capabilities::from_profile::<PtzOpticsG2>();
        let max_zoom = caps.max_optical_zoom();
        assert!(
            (max_zoom - 20.0).abs() < 0.5,
            "PtzOpticsG2 max optical zoom should be ~20x, got {max_zoom}"
        );

        // GenericVisca: 0xFFFF / 1000.0 + 1 ≈ 66.5x
        let caps = Capabilities::from_profile::<GenericVisca>();
        let max_zoom = caps.max_optical_zoom();
        assert!(
            (max_zoom - 66.5).abs() < 1.0,
            "GenericVisca max optical zoom should be ~66.5x, got {max_zoom}"
        );
    }

    #[test]
    fn test_max_combined_zoom() {
        // SonyFR7 has digital zoom (0x7000)
        let caps = Capabilities::from_profile::<SonyFR7>();
        let max_combined = caps.max_combined_zoom();
        let max_optical = caps.max_optical_zoom();
        assert!(
            max_combined > max_optical,
            "Combined zoom should exceed optical zoom for cameras with digital zoom"
        );

        // PtzOpticsG2 has no digital zoom (cameras reject the VISCA command)
        let caps = Capabilities::from_profile::<PtzOpticsG2>();
        assert!(
            !caps.has_digital_zoom,
            "PtzOpticsG2 should not declare digital zoom support"
        );
        let max_combined = caps.max_combined_zoom();
        let max_optical = caps.max_optical_zoom();
        assert!(
            (max_combined - max_optical).abs() < 0.01,
            "Without digital zoom, combined should equal optical"
        );

        // GenericVisca has no digital zoom
        let caps = Capabilities::from_profile::<GenericVisca>();
        let max_combined = caps.max_combined_zoom();
        let max_optical = caps.max_optical_zoom();
        assert!(
            (max_combined - max_optical).abs() < 0.01,
            "Without digital zoom, combined should equal optical"
        );
    }

    #[test]
    fn test_zoom_unit_conversions() {
        let caps = Capabilities::from_profile::<PtzOpticsG2>();

        // 1x magnification = 0 units
        let units = caps.magnification_to_zoom_units(1.0);
        assert_eq!(units, 0, "1x magnification should be 0 units");

        // 0 units = 1x magnification
        let mag = caps.zoom_units_to_magnification(0);
        assert!(
            (mag - 1.0).abs() < 0.01,
            "0 units should be 1x magnification"
        );

        // Round-trip conversion
        let original_mag = 10.0;
        let units = caps.magnification_to_zoom_units(original_mag);
        let recovered_mag = caps.zoom_units_to_magnification(units);
        assert!(
            (recovered_mag - original_mag).abs() < 0.1,
            "Round-trip conversion failed: {original_mag} -> {units} -> {recovered_mag}"
        );

        // Max optical zoom units should give max optical magnification
        let max_units = *caps.zoom_range_optical.end();
        let max_mag = caps.zoom_units_to_magnification(max_units);
        let expected_max = caps.max_optical_zoom();
        assert!(
            (max_mag - expected_max).abs() < 0.01,
            "Max units should give max magnification"
        );
    }

    #[test]
    fn test_magnification_to_units_edge_cases() {
        let caps = Capabilities::from_profile::<PtzOpticsG2>();

        // Values below 1.0 should clamp to 0
        assert_eq!(caps.magnification_to_zoom_units(0.5), 0);
        assert_eq!(caps.magnification_to_zoom_units(0.0), 0);
        assert_eq!(caps.magnification_to_zoom_units(-1.0), 0);
    }
}
