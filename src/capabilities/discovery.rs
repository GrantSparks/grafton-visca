//! Structured capabilities response for runtime feature discovery.
//!
//! This module provides a unified `Capabilities` struct that exposes
//! camera capabilities at runtime, complementing the compile-time
//! trait-based capability system.

use std::{borrow::Cow, ops::RangeInclusive, time::Duration};

use super::{
    profile_metadata::InquirySupport, ProfileTypedSupport, TypedSupportSet, TypedSupportSurface,
};
use crate::{command::exposure::ExposureMode, WhiteBalanceMode};

/// Owned runtime representation of one supported shutter setting.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct RuntimeShutterSpeed {
    /// Human-readable shutter label, such as `1/60`.
    pub label: String,
    /// VISCA protocol value.
    pub value: u16,
}

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
    /// Built-in registry identity, or `None` for a downstream runtime profile.
    pub profile_id: Option<crate::camera::profiles::ProfileId>,

    /// Model name of the camera.
    pub model_name: String,

    /// Default camera ID for VISCA addressing.
    pub default_camera_id: u8,

    // Network configuration
    /// Validated mirror of the profile's default TCP transport port.
    ///
    /// Runtime builders fill `None` from their transport facts; an explicit
    /// `Some` value is treated as an assertion and must match exactly.
    pub default_tcp_port: Option<u16>,

    /// Validated mirror of the profile's default UDP transport port.
    ///
    /// Runtime builders fill `None` from their transport facts; an explicit
    /// `Some` value is treated as an assertion and must match exactly.
    pub default_udp_port: Option<u16>,

    // Pan/Tilt capabilities
    /// Whether camera supports pan/tilt movement.
    pub has_pan_tilt: bool,

    /// Valid pan speed range (typically 1-24).
    pub pan_speed: RangeInclusive<u8>,

    /// Valid tilt speed range (typically 1-20, up to 24 where documented).
    pub tilt_speed: RangeInclusive<u8>,

    /// Pan position range in VISCA units.
    pub pan_range: RangeInclusive<i32>,

    /// Tilt position range in VISCA units.
    pub tilt_range: RangeInclusive<i32>,

    /// Pan position range in degrees.
    pub pan_range_degrees: RangeInclusive<f32>,

    /// Tilt position range in degrees.
    pub tilt_range_degrees: RangeInclusive<f32>,

    /// Whether camera can pan and tilt simultaneously.
    pub pan_tilt_simultaneous: bool,

    /// Mechanical recovery time after a preset movement.
    pub preset_recovery_time: Duration,

    // Zoom capabilities
    /// Whether camera supports zoom operations.
    pub has_zoom: bool,

    /// Whether profile metadata reports digital zoom beyond optical.
    ///
    /// This is not permission to call typed digital zoom APIs; use
    /// [`supports_typed`](Self::supports_typed) with
    /// [`TypedSupportSurface::DigitalZoomToggle`] or
    /// [`TypedSupportSurface::DigitalZoomRange`] for that.
    pub has_digital_zoom: bool,

    /// Optical zoom range in VISCA units (0x0000 to max).
    pub zoom_range_optical: RangeInclusive<u16>,

    /// Digital zoom range if supported (optical_max to digital_max).
    pub zoom_range_digital: Option<RangeInclusive<u16>>,

    /// Valid zoom speed range (typically 0-7).
    pub zoom_speed: RangeInclusive<u8>,

    /// Whether profile metadata reports direct zoom positioning.
    ///
    /// This is not permission to call typed direct zoom APIs; use
    /// [`supports_typed`](Self::supports_typed) with
    /// [`TypedSupportSurface::DirectZoom`] for that.
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

    /// Whether profile metadata reports one-push auto-focus.
    ///
    /// This is not permission to call the typed one-push focus API; use
    /// [`supports_typed`](Self::supports_typed) with
    /// [`TypedSupportSurface::OnePushFocus`] for that.
    pub has_one_push_focus: bool,

    /// Focus position range in VISCA units.
    pub focus_range: RangeInclusive<u16>,

    /// Valid focus speed range.
    pub focus_speed: RangeInclusive<u8>,

    /// Whether profile metadata reports focus-zone selection.
    ///
    /// This is not permission to call typed focus zone APIs; use
    /// [`supports_typed`](Self::supports_typed) with
    /// [`TypedSupportSurface::FocusZone`] for that. The separately gated
    /// [`TypedSupportSurface::FocusZoneInquiry`] controls the matching status
    /// response.
    pub has_focus_zone: bool,

    /// Whether profile metadata reports the focus-zone inquiry response.
    ///
    /// This is not permission to call the typed inquiry accessor; use
    /// [`supports_typed`](Self::supports_typed) with
    /// [`TypedSupportSurface::FocusZoneInquiry`] as well.
    pub has_focus_zone_inquiry: bool,

    /// Whether profile metadata reports auto focus sensitivity adjustment.
    ///
    /// This is not permission to call typed AF sensitivity APIs; use
    /// [`supports_typed`](Self::supports_typed) with
    /// [`TypedSupportSurface::AutoFocusSensitivity`] for that.
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

    /// Exposure compensation range, if supported.
    pub exposure_comp_range: Option<RangeInclusive<i8>>,

    /// Exact profile exposure-compensation range constant, even when disabled.
    ///
    /// [`exposure_comp_range`](Self::exposure_comp_range) is the active view;
    /// validated profiles require it to be either this exact range or `None`
    /// according to [`has_exposure_comp`](Self::has_exposure_comp).
    pub exposure_comp_profile_range: RangeInclusive<i8>,

    /// Iris range in VISCA units, if iris control is supported.
    pub iris_range: Option<RangeInclusive<u16>>,

    /// Gain range in VISCA units.
    pub gain_range: RangeInclusive<u8>,

    /// Exact supported shutter-speed labels and VISCA values.
    pub shutter_speeds: Vec<RuntimeShutterSpeed>,

    /// VISCA exposure bright range, if supported.
    ///
    /// This is the exposure brightness/bright-direct surface, not image
    /// luminance.
    pub exposure_brightness_range: Option<RangeInclusive<u16>>,

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

    /// Red gain range if supported.
    pub red_gain_range: Option<RangeInclusive<u8>>,

    /// Blue gain range if supported.
    pub blue_gain_range: Option<RangeInclusive<u8>>,

    /// Exact supported white-balance modes.
    pub white_balance_modes: Vec<WhiteBalanceMode>,

    // Image processing capabilities
    /// Whether camera supports image processing features.
    pub has_image_processing: bool,

    /// Contrast adjustment range, if supported.
    pub contrast_range: Option<RangeInclusive<u8>>,

    /// Sharpness adjustment range, if supported.
    pub sharpness_range: Option<RangeInclusive<u8>>,

    /// Saturation adjustment range if supported.
    pub saturation_range: Option<RangeInclusive<u8>>,

    /// Hue adjustment range if supported.
    pub hue_range: Option<RangeInclusive<u8>>,

    /// Image luminance range if supported.
    pub luminance_range: Option<RangeInclusive<u8>>,

    /// Gamma curve range if supported.
    pub gamma_range: Option<RangeInclusive<u8>>,

    /// Whether camera supports image flip.
    pub supports_flip: bool,

    /// Whether camera supports image mirror.
    pub supports_mirror: bool,

    /// Whether camera supports hue adjustment.
    pub supports_hue: bool,

    /// Whether flip and mirror share the combined VISCA command.
    pub uses_combined_flip_command: bool,

    /// Whether flip changes require an explicit settings-save command.
    pub requires_settings_save_for_flip: bool,

    /// Whether camera supports noise reduction.
    pub has_noise_reduction: bool,

    /// Whether camera supports 2D noise reduction.
    pub has_2d_nr: bool,

    /// Whether camera supports 3D noise reduction.
    pub has_3d_nr: bool,

    /// Whether camera supports source-backed picture effects (Off and Black & White).
    /// Model-specific values remain available through `PictureEffectMode::Unknown`.
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

    /// Required delay after recalling a preset.
    pub preset_recall_delay: Duration,

    /// Whether camera supports stored preset names.
    pub supports_preset_names: bool,

    /// Maximum preset-name length, zero when names are unsupported.
    pub max_preset_name_length: usize,

    // Power capabilities
    /// Whether camera supports power control.
    pub has_power: bool,

    /// Whether camera supports standby mode.
    pub supports_standby: bool,

    /// Whether camera supports wake-on-LAN.
    pub supports_wake_on_lan: bool,

    /// Exact time required for the power-on sequence.
    pub power_on_time: Duration,

    /// Exact time required to enter standby.
    pub standby_time: Duration,

    /// Whether settings survive power-off.
    pub retains_settings_on_power_off: bool,

    /// Whether power-on automatically returns to home.
    pub home_on_power_up: bool,

    // Special capabilities
    /// Whether camera has ND filter support.
    pub has_nd_filter: bool,

    /// Exact typed ND filter operating mode.
    pub nd_filter_mode: super::NdFilterMode,

    /// Explicit ND step count when supplied by the profile.
    pub nd_filter_steps: Option<u8>,

    /// Whether camera supports motion sync.
    pub has_motion_sync: bool,

    /// Maximum motion sync speed if supported.
    pub max_motion_sync_speed: Option<u8>,

    /// Exact profile motion-sync maximum constant, even when disabled.
    pub max_motion_sync_speed_profile: u8,

    /// Whether camera supports direct menu control.
    pub has_direct_menu_control: bool,

    /// Whether camera supports variable speed control.
    pub has_variable_speed: bool,

    /// Whether camera has source-backed USB audio support.
    ///
    /// This is not itself permission to use the typed USB-audio API; check
    /// [`supports_typed`](Self::supports_typed) with
    /// [`TypedSupportSurface::UsbAudio`] as well.
    pub has_usb_audio: bool,

    // Protocol features
    /// Level of VISCA inquiry command support for this camera.
    ///
    /// Use this instead of matching on `ProfileGroup` to determine whether
    /// inquiry commands are available. See [`InquirySupport`] for details.
    pub inquiry_support: InquirySupport,

    /// Whether camera sends operation complete messages.
    pub supports_operation_complete: bool,

    /// Optional typed API surfaces this profile is permitted to expose.
    ///
    /// This is the runtime permission source for dyn-api optional typed
    /// operations. Metadata fields such as [`has_digital_zoom`](Self::has_digital_zoom)
    /// and [`supports_direct_zoom`](Self::supports_direct_zoom) remain physical
    /// or protocol discovery facts and are not typed API permission checks.
    #[cfg_attr(feature = "schemars", schemars(with = "Vec<TypedSupportSurface>"))]
    pub typed_support: TypedSupportSet,
}

impl Capabilities {
    /// Creates a conservative runtime-only capability inventory.
    ///
    /// No transport, control domain, inquiry, completion, cancellation, or
    /// optional typed surface is granted. Callers explicitly populate the
    /// documented facts, then pass the value to [`ProfileSpec::builder`](crate::ProfileSpec::builder).
    ///
    /// # Errors
    ///
    /// Returns an error for an empty model name or invalid camera ID.
    pub fn runtime_baseline(
        model_name: impl Into<String>,
        default_camera_id: u8,
    ) -> Result<Self, crate::Error> {
        let model_name = model_name.into();
        if model_name.trim().is_empty() {
            return Err(crate::Error::InvalidRequest(
                "runtime capability model name must not be empty".into(),
            ));
        }
        crate::CameraId::new(default_camera_id)?;

        Ok(Self {
            profile_id: None,
            model_name,
            default_camera_id,
            default_tcp_port: None,
            default_udp_port: None,
            has_pan_tilt: false,
            pan_speed: 0..=0,
            tilt_speed: 0..=0,
            pan_range: 0..=0,
            tilt_range: 0..=0,
            pan_range_degrees: 0.0..=0.0,
            tilt_range_degrees: 0.0..=0.0,
            pan_tilt_simultaneous: false,
            preset_recovery_time: Duration::ZERO,
            has_zoom: false,
            has_digital_zoom: false,
            zoom_range_optical: 0..=0,
            zoom_range_digital: None,
            zoom_speed: 0..=0,
            supports_direct_zoom: false,
            supports_variable_zoom: false,
            zoom_magnification_to_units: 1.0,
            has_focus: false,
            has_auto_focus: false,
            has_one_push_focus: false,
            focus_range: 0..=0,
            focus_speed: 0..=0,
            has_focus_zone: false,
            has_focus_zone_inquiry: false,
            has_af_sensitivity: false,
            has_focus_near_limit_inquiry: false,
            has_exposure: false,
            has_iris_control: false,
            exposure_modes: Vec::new(),
            has_backlight_comp: false,
            has_wdr: false,
            has_exposure_comp: false,
            exposure_comp_range: None,
            exposure_comp_profile_range: -7..=7,
            iris_range: None,
            gain_range: 0..=0,
            shutter_speeds: Vec::new(),
            exposure_brightness_range: None,
            has_white_balance: false,
            has_one_push_wb: false,
            has_color_temp: false,
            color_temp_range: None,
            rg_tuning_range: None,
            bg_tuning_range: None,
            has_rgb_gain: false,
            red_gain_range: None,
            blue_gain_range: None,
            white_balance_modes: Vec::new(),
            has_image_processing: false,
            contrast_range: None,
            sharpness_range: None,
            saturation_range: None,
            hue_range: None,
            luminance_range: None,
            gamma_range: None,
            supports_flip: false,
            supports_mirror: false,
            supports_hue: false,
            uses_combined_flip_command: false,
            requires_settings_save_for_flip: false,
            has_noise_reduction: false,
            has_2d_nr: false,
            has_3d_nr: false,
            has_picture_effect: false,
            has_gamma: false,
            has_luminance: false,
            has_tally: false,
            has_presets: false,
            max_presets: 0,
            preset_speed_range: 0..=0,
            supports_preset_tour: false,
            supports_preset_thumbnail: false,
            preset_recall_delay: Duration::ZERO,
            supports_preset_names: false,
            max_preset_name_length: 0,
            has_power: false,
            supports_standby: false,
            supports_wake_on_lan: false,
            power_on_time: Duration::ZERO,
            standby_time: Duration::ZERO,
            retains_settings_on_power_off: false,
            home_on_power_up: false,
            has_nd_filter: false,
            nd_filter_mode: super::NdFilterMode::None,
            nd_filter_steps: None,
            has_motion_sync: false,
            max_motion_sync_speed: None,
            max_motion_sync_speed_profile: 24,
            has_direct_menu_control: false,
            has_variable_speed: false,
            has_usb_audio: false,
            inquiry_support: InquirySupport::None,
            supports_operation_complete: false,
            typed_support: TypedSupportSet::empty(),
        })
    }

    /// Creates a new Capabilities struct from a camera profile.
    ///
    /// This method extracts all capability information from the compile-time
    /// trait constants and creates a runtime-queryable struct.
    pub fn from_profile<P>() -> Self
    where
        P: crate::capabilities::Profile,
    {
        // Extract pan/tilt capabilities
        let pan_start_deg = P::PAN_RANGE.min() as f32 / P::PAN_DEGREES_TO_UNITS;
        let pan_end_deg = P::PAN_RANGE.max() as f32 / P::PAN_DEGREES_TO_UNITS;
        let tilt_start_deg = P::TILT_RANGE.min() as f32 / P::TILT_DEGREES_TO_UNITS;
        let tilt_end_deg = P::TILT_RANGE.max() as f32 / P::TILT_DEGREES_TO_UNITS;
        // A profile may use a negative degree-to-unit scale when its raw axis
        // polarity is opposite the library's degree convention. Capabilities
        // always expose ordered logical-degree ranges.
        let pan_min_deg = pan_start_deg.min(pan_end_deg);
        let pan_max_deg = pan_start_deg.max(pan_end_deg);
        let tilt_min_deg = tilt_start_deg.min(tilt_end_deg);
        let tilt_max_deg = tilt_start_deg.max(tilt_end_deg);

        let pan_speed = 1..=P::MAX_PAN_SPEED;
        let tilt_speed = 1..=P::MAX_TILT_SPEED;
        let pan_range = P::PAN_RANGE.as_inclusive();
        let tilt_range = P::TILT_RANGE.as_inclusive();
        let pan_range_degrees = pan_min_deg..=pan_max_deg;
        let tilt_range_degrees = tilt_min_deg..=tilt_max_deg;
        let pan_tilt_simultaneous = P::PAN_TILT_SIMULTANEOUS;
        let preset_recovery_time = P::PRESET_RECOVERY_TIME;

        // Extract zoom capabilities
        let has_digital_zoom = P::DIGITAL_ZOOM_MAX.is_some();
        let zoom_range_digital = P::DIGITAL_ZOOM_MAX.map(|max| P::OPTICAL_ZOOM_MAX..=max);

        // Extract focus capabilities
        let focus_range = P::FOCUS_NEAR_LIMIT..=P::FOCUS_FAR_LIMIT;
        let focus_speed = 0..=P::MAX_FOCUS_SPEED;

        // Extract exposure capabilities
        let has_iris_control = P::IRIS_RANGE.is_some();
        let exposure_modes = P::EXPOSURE_MODES.to_vec();
        let shutter_speeds = P::SHUTTER_SPEEDS
            .iter()
            .map(|speed| RuntimeShutterSpeed {
                label: speed.label.to_owned(),
                value: speed.value,
            })
            .collect();
        let iris_range = P::IRIS_RANGE.map(|range| range.as_inclusive());
        let gain_range = P::GAIN_RANGE.as_inclusive();
        let exposure_brightness_range = P::BRIGHTNESS_RANGE.map(|range| range.as_inclusive());
        let exposure_comp_range =
            P::SUPPORTS_EXPOSURE_COMP.then(|| P::EXPOSURE_COMP_RANGE.as_inclusive());

        // Extract white balance capabilities
        let color_temp_range = P::COLOR_TEMP_RANGE.map(|range| range.as_inclusive());
        let rg_tuning_range = P::RG_TUNING_RANGE.map(|range| range.as_inclusive());
        let bg_tuning_range = P::BG_TUNING_RANGE.map(|range| range.as_inclusive());
        let red_gain_range = P::RED_GAIN_RANGE.map(|range| range.as_inclusive());
        let blue_gain_range = P::BLUE_GAIN_RANGE.map(|range| range.as_inclusive());

        // Extract image processing capabilities
        let contrast_range = P::CONTRAST_RANGE.map(|range| range.as_inclusive());
        let sharpness_range = P::SHARPNESS_RANGE.map(|range| range.as_inclusive());
        let saturation_range = P::SATURATION_RANGE.map(|range| range.as_inclusive());
        let hue_range = P::HUE_RANGE.map(|range| range.as_inclusive());
        let luminance_range = P::LUMINANCE_RANGE.map(|range| range.as_inclusive());
        let gamma_range = P::GAMMA_RANGE.map(|range| range.as_inclusive());
        // Built-ins override this metadata constant from the same registry
        // fact that emits `HasImageProcessing`; downstream profiles default to
        // deny and opt into the matching runtime and static contracts together.
        let has_image_processing = P::SUPPORTS_IMAGE_PROCESSING;

        // Extract preset capabilities
        let preset_speed_range = P::PRESET_SPEED_RANGE.as_inclusive();

        // Extract ND filter capabilities from profile metadata.
        let has_nd_filter = !matches!(P::ND_MODE, crate::capabilities::NdFilterMode::None);

        // Extract Motion Sync capabilities from profile metadata.
        let has_motion_sync = P::SUPPORTS_MOTION_SYNC;
        let max_motion_sync_speed = if has_motion_sync {
            Some(P::MAX_MOTION_SYNC_SPEED)
        } else {
            None
        };

        Self {
            // Camera identification
            profile_id: P::PROFILE_ID,
            model_name: P::MODEL_NAME.to_string(),
            default_camera_id: P::DEFAULT_CAMERA_ID,

            // Network configuration
            default_tcp_port: P::PROFILE_ID.and_then(|profile| profile.default_tcp_port()),
            default_udp_port: P::PROFILE_ID.and_then(|profile| profile.default_udp_port()),

            // Pan/Tilt capabilities
            has_pan_tilt: true, // All cameras in Profile have pan/tilt
            pan_speed,
            tilt_speed,
            pan_range,
            tilt_range,
            pan_range_degrees,
            tilt_range_degrees,
            pan_tilt_simultaneous,
            preset_recovery_time,

            // Zoom capabilities
            has_zoom: true, // All cameras have zoom
            has_digital_zoom,
            zoom_range_optical: 0x0000..=P::OPTICAL_ZOOM_MAX,
            zoom_range_digital,
            zoom_speed: P::ZOOM_SPEED_RANGE.as_inclusive(),
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
            has_focus_zone_inquiry: P::SUPPORTS_FOCUS_ZONE_INQUIRY,
            has_af_sensitivity: P::SUPPORTS_AF_SENSITIVITY,
            has_focus_near_limit_inquiry: P::SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY,

            // Exposure capabilities
            has_exposure: true, // All cameras have exposure control
            has_iris_control,
            exposure_modes,
            has_backlight_comp: P::SUPPORTS_BACKLIGHT_COMP,
            has_wdr: P::SUPPORTS_WDR,
            has_exposure_comp: P::SUPPORTS_EXPOSURE_COMP,
            exposure_comp_range,
            exposure_comp_profile_range: P::EXPOSURE_COMP_RANGE.as_inclusive(),
            iris_range,
            gain_range,
            shutter_speeds,
            exposure_brightness_range,

            // White balance capabilities
            has_white_balance: true, // All cameras have white balance
            has_one_push_wb: P::SUPPORTS_ONE_PUSH_WB,
            has_color_temp: P::SUPPORTS_COLOR_TEMP,
            color_temp_range,
            rg_tuning_range,
            bg_tuning_range,
            has_rgb_gain: P::SUPPORTS_RGB_GAIN,
            red_gain_range,
            blue_gain_range,
            white_balance_modes: P::WB_MODES.to_vec(),

            // Image processing capabilities
            has_image_processing,
            contrast_range,
            sharpness_range,
            saturation_range,
            hue_range,
            luminance_range,
            gamma_range,
            supports_flip: P::SUPPORTS_FLIP,
            supports_mirror: P::SUPPORTS_MIRROR,
            supports_hue: P::SUPPORTS_HUE,
            uses_combined_flip_command: P::USES_COMBINED_FLIP_COMMAND,
            requires_settings_save_for_flip: P::REQUIRES_SETTINGS_SAVE_FOR_FLIP,
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
            preset_recall_delay: P::PRESET_RECALL_DELAY,
            supports_preset_names: P::SUPPORTS_PRESET_NAMES,
            max_preset_name_length: P::MAX_PRESET_NAME_LENGTH,

            // Power capabilities
            has_power: true, // All cameras have power control
            supports_standby: P::SUPPORTS_STANDBY,
            supports_wake_on_lan: P::SUPPORTS_WAKE_ON_LAN,
            power_on_time: P::POWER_ON_TIME,
            standby_time: P::STANDBY_TIME,
            retains_settings_on_power_off: P::RETAINS_SETTINGS_ON_POWER_OFF,
            home_on_power_up: P::HOME_ON_POWER_UP,

            // Special capabilities
            has_nd_filter,
            nd_filter_mode: P::ND_MODE,
            nd_filter_steps: P::ND_STEPS,
            has_motion_sync,
            max_motion_sync_speed,
            max_motion_sync_speed_profile: P::MAX_MOTION_SYNC_SPEED,
            has_direct_menu_control: P::SUPPORTS_DIRECT_CONTROL,
            has_variable_speed: P::SUPPORTS_VARIABLE_SPEED,
            has_usb_audio: P::SUPPORTS_USB_AUDIO,

            // Protocol features
            inquiry_support: P::INQUIRY_SUPPORT,
            supports_operation_complete: P::SUPPORTS_OPERATION_COMPLETE,

            typed_support: <P as ProfileTypedSupport>::TYPED_SUPPORT,
        }
    }

    /// Returns true if this profile permits the requested typed API surface.
    #[must_use]
    pub const fn supports_typed(&self, surface: TypedSupportSurface) -> bool {
        self.typed_support.contains(surface)
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
    /// # Errors
    /// Returns an error if the magnification is not finite, is below 1.0, or
    /// maps past the documented optical or optical-plus-digital zoom range.
    ///
    /// # Example
    /// ```ignore
    /// let caps = Capabilities::from_profile::<PtzOpticsG2>();
    /// let units = caps.magnification_to_zoom_units(10.0)?;
    /// println!("10x zoom = 0x{:04X} units", units); // ~0x2000
    /// ```
    pub fn magnification_to_zoom_units(&self, magnification: f32) -> Result<u16, crate::Error> {
        if !magnification.is_finite() {
            return Err(crate::Error::InvalidParameter {
                parameter: "zoom magnification",
                value: Cow::Owned(magnification.to_string()),
                reason: Cow::Borrowed("value must be finite"),
            });
        }

        if magnification < 1.0 {
            return Err(crate::Error::InvalidParameter {
                parameter: "zoom magnification",
                value: Cow::Owned(magnification.to_string()),
                reason: Cow::Borrowed("value must be at least 1.0x"),
            });
        }

        let max_units = self
            .zoom_range_digital
            .as_ref()
            .map_or(*self.zoom_range_optical.end(), |range| *range.end());
        let units =
            (f64::from(magnification - 1.0) * f64::from(self.zoom_magnification_to_units)).round();

        if !units.is_finite() || units > f64::from(max_units) {
            return Err(crate::Error::InvalidParameter {
                parameter: "zoom magnification",
                value: Cow::Owned(magnification.to_string()),
                reason: Cow::Owned(format!(
                    "resulting zoom units exceed documented maximum {max_units:#06X}"
                )),
            });
        }

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Ok(units as u16)
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
            lines.push(format!("ND Filter: {:?}", self.nd_filter_mode));
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::camera::profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    };
    use crate::capabilities::{
        exposure::ShutterSpeed, CapabilityRange, Exposure, Focus, ImageProcessing, MenuCapability,
        MotionSyncMetadata, NdFilterMetadata, PanTilt, Power, Presets, ProfileMetadata,
        ProfileTypedSupport, Tally, VariableSpeedMetadata, WhiteBalance, Zoom,
    };
    use crate::command::exposure::ExposureMode;
    use crate::transport::RawVisca;
    use crate::{ProfileSpec, WhiteBalanceMode};

    use super::*;

    const SYNTHETIC_EXPOSURE_MODES: &[ExposureMode] = &[ExposureMode::Auto];
    const SYNTHETIC_SHUTTER_SPEEDS: &[ShutterSpeed] = &[ShutterSpeed::new("1/60", 0x01)];
    const SYNTHETIC_WB_MODES: &[WhiteBalanceMode] = &[WhiteBalanceMode::Auto];

    #[derive(Debug, Default, Clone, Copy)]
    struct MetadataEnabledNoTypedSupport;

    impl ProfileMetadata for MetadataEnabledNoTypedSupport {
        const MODEL_NAME: &'static str = "Metadata Enabled Without Typed Support";
        const DEFAULT_CAMERA_ID: u8 = 1;
        type Envelope = RawVisca;
        const ACK_TIMEOUT: Duration = Duration::from_millis(100);
        const COMMAND_TIMEOUTS: crate::CommandTimeouts = crate::CommandTimeouts::new(
            Duration::from_secs(5),
            Duration::from_secs(30),
            Duration::from_secs(60),
            Duration::from_secs(300),
            Duration::from_secs(5),
        );
    }

    impl PanTilt for MetadataEnabledNoTypedSupport {
        const PAN_RANGE: CapabilityRange<i32> = CapabilityRange::<i32>::new(-1700, 1700);
        const TILT_RANGE: CapabilityRange<i32> = CapabilityRange::<i32>::new(-300, 900);
        const MAX_PAN_SPEED: u8 = 24;
        const MAX_TILT_SPEED: u8 = 20;
        const PAN_DEGREES_TO_UNITS: f32 = 10.0;
        const TILT_DEGREES_TO_UNITS: f32 = 10.0;
    }

    impl Zoom for MetadataEnabledNoTypedSupport {
        const OPTICAL_ZOOM_MAX: u16 = 0x4000;
        const DIGITAL_ZOOM_MAX: Option<u16> = Some(0x7000);
        const ZOOM_SPEED_RANGE: CapabilityRange<u8> = CapabilityRange::<u8>::new(0, 7);
        const SUPPORTS_DIRECT_ZOOM: bool = true;
        const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 862.3;
    }

    impl Focus for MetadataEnabledNoTypedSupport {
        const FOCUS_NEAR_LIMIT: u16 = 0x1000;
        const FOCUS_FAR_LIMIT: u16 = 0xF000;
        const SUPPORTS_AUTO_FOCUS: bool = true;
        const SUPPORTS_ONE_PUSH_FOCUS: bool = true;
        const SUPPORTS_FOCUS_ZONE: bool = true;
        const SUPPORTS_AF_SENSITIVITY: bool = true;
    }

    impl Exposure for MetadataEnabledNoTypedSupport {
        const EXPOSURE_MODES: &'static [ExposureMode] = SYNTHETIC_EXPOSURE_MODES;
        const IRIS_RANGE: Option<CapabilityRange<u16>> = None;
        const SHUTTER_SPEEDS: &'static [ShutterSpeed] = SYNTHETIC_SHUTTER_SPEEDS;
        const GAIN_RANGE: CapabilityRange<u8> = CapabilityRange::<u8>::new(0, 15);
        const SUPPORTS_BACKLIGHT_COMP: bool = false;
    }

    impl WhiteBalance for MetadataEnabledNoTypedSupport {
        const WB_MODES: &'static [WhiteBalanceMode] = SYNTHETIC_WB_MODES;
        const SUPPORTS_ONE_PUSH_WB: bool = false;
        const RG_TUNING_RANGE: Option<CapabilityRange<i8>> = None;
        const BG_TUNING_RANGE: Option<CapabilityRange<i8>> = None;
    }

    impl ImageProcessing for MetadataEnabledNoTypedSupport {
        const CONTRAST_RANGE: Option<CapabilityRange<u8>> = None;
        const SHARPNESS_RANGE: Option<CapabilityRange<u8>> = None;
        const SATURATION_RANGE: Option<CapabilityRange<u8>> = None;
        const SUPPORTS_FLIP: bool = false;
        const SUPPORTS_MIRROR: bool = false;
    }

    impl Presets for MetadataEnabledNoTypedSupport {
        const MAX_PRESETS: u8 = 6;
        const PRESET_SPEED_RANGE: CapabilityRange<u8> = CapabilityRange::<u8>::new(1, 23);
        const SUPPORTS_PRESET_TOUR: bool = false;
    }

    impl Power for MetadataEnabledNoTypedSupport {
        const POWER_ON_TIME: Duration = Duration::from_secs(5);
        const SUPPORTS_STANDBY: bool = false;
    }

    impl MenuCapability for MetadataEnabledNoTypedSupport {}
    impl Tally for MetadataEnabledNoTypedSupport {}
    impl MotionSyncMetadata for MetadataEnabledNoTypedSupport {}
    impl NdFilterMetadata for MetadataEnabledNoTypedSupport {}
    impl VariableSpeedMetadata for MetadataEnabledNoTypedSupport {}

    impl ProfileTypedSupport for MetadataEnabledNoTypedSupport {
        const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::EMPTY;
    }

    #[test]
    fn test_capabilities_from_ptzoptics_g2() {
        let caps = Capabilities::from_profile::<PtzOpticsG2>();

        assert_eq!(
            caps.profile_id,
            Some(crate::profiles::ProfileId::PtzOpticsG2)
        );
        assert_eq!(caps.model_name, "PtzOptics G2");
        assert_eq!(caps.default_camera_id, 1);
        assert_eq!(caps.default_tcp_port, Some(5678));
        assert_eq!(caps.default_udp_port, Some(1259));

        assert!(caps.has_pan_tilt);
        assert_eq!(caps.pan_speed, 1..=24);
        assert_eq!(caps.tilt_speed, 1..=20);

        assert!(caps.has_zoom);
        assert!(!caps.has_digital_zoom);
        assert_eq!(caps.zoom_range_optical, 0x0000..=0x4000);
        assert_eq!(caps.zoom_range_digital, None);

        assert!(caps.has_auto_focus);
        assert!(!caps.has_one_push_focus);
        assert!(caps.has_focus_zone);
        assert!(caps.has_focus_zone_inquiry);
        assert!(!caps.has_af_sensitivity);
        assert!(!caps.has_focus_near_limit_inquiry);
        assert!(caps.has_iris_control);
        assert!(caps.supports_exposure_mode(ExposureMode::Iris));
        assert_eq!(caps.iris_range, Some(0..=12));
        assert_eq!(caps.exposure_comp_range, Some(-7..=7));
        assert_eq!(caps.exposure_comp_profile_range, -7..=7);
        assert!(caps.has_rgb_gain);
        assert_eq!(caps.red_gain_range, Some(0..=255));
        assert_eq!(caps.blue_gain_range, Some(0..=255));

        assert_eq!(caps.max_presets, 127);
        assert!(!caps.supports_preset_tour);
        assert!(!caps.has_nd_filter);
        assert!(!caps.has_motion_sync);
        assert_eq!(caps.max_motion_sync_speed, None);
        assert_eq!(caps.max_motion_sync_speed_profile, 24);
        assert!(!caps.has_variable_speed);
        assert!(caps.has_usb_audio);
        assert_eq!(caps.exposure_brightness_range, Some(0..=17));
        assert!(caps.has_image_processing);
        assert_eq!(caps.contrast_range, Some(0..=14));
        assert_eq!(caps.sharpness_range, Some(0..=15));
        assert_eq!(caps.luminance_range, Some(0..=14));
        assert_eq!(caps.gamma_range, Some(0..=4));
        assert!(caps.has_picture_effect);

        assert!(caps.has_basic_features());
    }

    #[test]
    fn test_capabilities_from_sony_fr7() {
        let caps = Capabilities::from_profile::<SonyFR7>();

        assert_eq!(caps.model_name, "Sony FR7");
        assert_eq!(caps.default_tcp_port, None);
        assert_eq!(caps.default_udp_port, Some(52381));

        assert_eq!(caps.max_presets, 255);
        assert!(caps.supports_preset_tour);
        assert!(!caps.has_one_push_focus);
        assert!(!caps.has_focus_zone);
        assert!(!caps.has_focus_zone_inquiry);
        assert!(caps.has_af_sensitivity);
        assert!(caps.has_focus_near_limit_inquiry);
        assert!(caps.has_rgb_gain);
        assert_eq!(caps.red_gain_range, Some(0..=0xFF));
        assert_eq!(caps.blue_gain_range, Some(0..=0xFF));
        assert_eq!(caps.exposure_comp_range, Some(-7..=7));
        assert!(!caps.has_color_temp);
        assert_eq!(caps.color_temp_range, None);
        assert!(caps.has_nd_filter);
        assert_eq!(
            caps.nd_filter_mode,
            crate::capabilities::NdFilterMode::Variable
        );
        assert!(!caps.has_motion_sync);
        assert_eq!(caps.max_motion_sync_speed, None);
        assert!(caps.has_variable_speed);
        assert!(!caps.has_usb_audio);
        assert_eq!(caps.exposure_brightness_range, None);
        assert!(caps.has_image_processing);
        assert_eq!(caps.contrast_range, Some(0..=14));
        assert_eq!(caps.sharpness_range, Some(0..=14));
        assert_eq!(caps.gamma_range, Some(0..=4));
        assert!(!caps.has_picture_effect);

        assert!(caps.has_advanced_features());
    }

    #[test]
    fn test_typed_support_discovery_for_built_in_profiles() {
        let g2 = Capabilities::from_profile::<PtzOpticsG2>();
        assert_eq!(
            g2.typed_support,
            <PtzOpticsG2 as ProfileTypedSupport>::TYPED_SUPPORT
        );
        assert!(g2.supports_typed(TypedSupportSurface::DirectZoom));
        assert!(g2.supports_typed(TypedSupportSurface::FocusZone));
        assert!(g2.supports_typed(TypedSupportSurface::FocusZoneInquiry));
        assert!(g2.supports_typed(TypedSupportSurface::UsbAudio));
        assert!(g2.supports_typed(TypedSupportSurface::PtzOpticsAntiFlicker));
        assert!(g2.supports_typed(TypedSupportSurface::PtzOpticsSettingsSave));
        assert!(g2.supports_typed(TypedSupportSurface::PtzOpticsPresetRecallSpeed));
        assert!(g2.supports_typed(TypedSupportSurface::PtzOpticsMulticastStreaming));
        assert!(g2.supports_typed(TypedSupportSurface::PtzOpticsNdiQuality));
        assert!(!g2.supports_typed(TypedSupportSurface::DigitalZoomToggle));
        assert!(!g2.supports_typed(TypedSupportSurface::DigitalZoomRange));

        let fr7 = Capabilities::from_profile::<SonyFR7>();
        assert_eq!(
            fr7.typed_support,
            <SonyFR7 as ProfileTypedSupport>::TYPED_SUPPORT
        );
        assert!(fr7.supports_typed(TypedSupportSurface::DirectZoom));
        assert!(fr7.supports_typed(TypedSupportSurface::DigitalZoomToggle));
        assert!(fr7.supports_typed(TypedSupportSurface::DigitalZoomRange));
        assert!(!fr7.supports_typed(TypedSupportSurface::FocusZone));
        assert!(!fr7.supports_typed(TypedSupportSurface::FocusZoneInquiry));
        assert!(!fr7.supports_typed(TypedSupportSurface::UsbAudio));
        assert!(!fr7.supports_typed(TypedSupportSurface::BrightnessControl));
        assert!(!fr7.supports_typed(TypedSupportSurface::PictureEffect));
        assert!(fr7.supports_typed(TypedSupportSurface::AutoFocusSensitivity));
        assert!(fr7.supports_typed(TypedSupportSurface::SonySpotlight));
        assert!(!fr7.supports_typed(TypedSupportSurface::SonyAutoSlowShutter));
    }

    #[test]
    fn sony_vendor_exposure_typed_support_matches_the_model_command_lists() {
        let expected = [
            (
                "Sony FR7",
                Capabilities::from_profile::<SonyFR7>(),
                true,
                false,
            ),
            (
                "Sony BRC-H900",
                Capabilities::from_profile::<SonyBRCH900>(),
                true,
                false,
            ),
            (
                "Sony EVI-H100",
                Capabilities::from_profile::<SonyEVIH100>(),
                false,
                true,
            ),
            (
                "Sony BRC-300",
                Capabilities::from_profile::<SonyBRC300>(),
                false,
                true,
            ),
            (
                "Nearus BRC-300",
                Capabilities::from_profile::<NearusBRC300>(),
                false,
                false,
            ),
        ];

        for (model, caps, spotlight, auto_slow_shutter) in expected {
            assert_eq!(
                caps.supports_typed(TypedSupportSurface::SonySpotlight),
                spotlight,
                "{model} spotlight support"
            );
            assert_eq!(
                caps.supports_typed(TypedSupportSurface::SonyAutoSlowShutter),
                auto_slow_shutter,
                "{model} auto slow-shutter support"
            );
        }
    }

    #[test]
    fn test_metadata_enabled_profile_reports_typed_support_independently() {
        let caps = Capabilities::from_profile::<MetadataEnabledNoTypedSupport>();

        assert!(caps.supports_direct_zoom);
        assert!(caps.has_digital_zoom);
        assert!(caps.has_one_push_focus);
        assert!(caps.has_focus_zone);
        assert!(!caps.has_focus_zone_inquiry);
        assert!(caps.has_af_sensitivity);
        assert!(!caps.has_usb_audio);

        assert!(caps.typed_support.is_empty());
        assert!(!caps.supports_typed(TypedSupportSurface::DirectZoom));
        assert!(!caps.supports_typed(TypedSupportSurface::DigitalZoomToggle));
        assert!(!caps.supports_typed(TypedSupportSurface::DigitalZoomRange));
        assert!(!caps.supports_typed(TypedSupportSurface::OnePushFocus));
        assert!(!caps.supports_typed(TypedSupportSurface::FocusZone));
        assert!(!caps.supports_typed(TypedSupportSurface::FocusZoneInquiry));
        assert!(!caps.supports_typed(TypedSupportSurface::UsbAudio));
        assert!(!caps.supports_typed(TypedSupportSurface::AutoFocusSensitivity));
        assert!(!caps.supports_typed(TypedSupportSurface::PtzOpticsAntiFlicker));
        assert!(!caps.supports_typed(TypedSupportSurface::PtzOpticsSettingsSave));
        assert!(!caps.supports_typed(TypedSupportSurface::PtzOpticsPresetRecallSpeed));
        assert!(!caps.supports_typed(TypedSupportSurface::SonySpotlight));
        assert!(!caps.supports_typed(TypedSupportSurface::SonyAutoSlowShutter));
        assert!(!caps.supports_typed(TypedSupportSurface::PtzOpticsMulticastStreaming));
        assert!(!caps.supports_typed(TypedSupportSurface::PtzOpticsNdiQuality));
    }

    #[test]
    fn source_backed_focus_zone_inquiry_and_usb_audio_boundaries_match_profiles() {
        let g2 = Capabilities::from_profile::<PtzOpticsG2>();
        let g3 = Capabilities::from_profile::<PtzOpticsG3>();
        let thirty_x = Capabilities::from_profile::<PtzOptics30X>();

        for caps in [&g2, &g3, &thirty_x] {
            assert!(caps.has_focus_zone);
            assert!(caps.supports_typed(TypedSupportSurface::FocusZone));
            assert!(caps.has_picture_effect);
            assert!(caps.supports_typed(TypedSupportSurface::PictureEffect));
        }

        for caps in [&g2, &thirty_x] {
            assert!(caps.has_focus_zone_inquiry);
            assert!(caps.supports_typed(TypedSupportSurface::FocusZoneInquiry));
            assert!(caps.has_usb_audio);
            assert!(caps.supports_typed(TypedSupportSurface::UsbAudio));
        }

        assert!(!g3.has_focus_zone_inquiry);
        assert!(!g3.supports_typed(TypedSupportSurface::FocusZoneInquiry));
        assert!(!g3.has_usb_audio);
        assert!(!g3.supports_typed(TypedSupportSurface::UsbAudio));
    }

    #[test]
    fn test_color_temperature_profile_metadata_matches_sources() {
        for caps in [
            Capabilities::from_profile::<PtzOpticsG2>(),
            Capabilities::from_profile::<PtzOpticsG3>(),
            Capabilities::from_profile::<PtzOptics30X>(),
            Capabilities::from_profile::<SonyBRCH900>(),
            Capabilities::from_profile::<SonyEVIH100>(),
        ] {
            assert!(caps.has_color_temp);
            assert_eq!(caps.color_temp_range, Some(2500..=8000));
            assert_eq!(caps.white_balance_modes.len(), 6);
        }

        let fr7 = Capabilities::from_profile::<SonyFR7>();
        assert!(!fr7.has_color_temp);
        assert_eq!(fr7.color_temp_range, None);
        assert_eq!(fr7.white_balance_modes.len(), 6);
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

        assert!(!caps.has_image_processing);
        assert_eq!(caps.exposure_brightness_range, None);
        assert_eq!(caps.exposure_comp_range, None);
        assert_eq!(caps.contrast_range, None);
        assert_eq!(caps.sharpness_range, None);
        assert_eq!(caps.luminance_range, None);
        assert_eq!(caps.gamma_range, None);
        assert!(!caps.has_basic_features());
        // GenericVisca has one-push white balance, which counts as an advanced feature
        assert!(caps.has_one_push_wb);
        assert!(!caps.has_nd_filter);
        assert!(!caps.has_motion_sync);
        assert_eq!(caps.max_motion_sync_speed, None);
        assert!(!caps.has_variable_speed);
        assert!(caps.has_advanced_features());
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn brc300_static_profile_preserves_documented_preset_metadata() {
        let profile =
            ProfileSpec::from_compile_time::<SonyBRC300>().expect("Sony BRC-300 static profile");
        let caps = profile.capabilities();

        assert_eq!(caps.max_presets, 5, "CAM_Memory p is 0 through 5");
        assert_eq!(
            caps.preset_speed_range,
            1..=24,
            "Cmd_PT_M_Speed q is 1 through 24"
        );
    }

    #[test]
    fn brc300_discovery_keeps_preset_facts_separate_from_ptzoptics_typed_support() {
        let caps = Capabilities::from_profile::<SonyBRC300>();

        assert_eq!(caps.max_presets, 5);
        assert_eq!(caps.preset_speed_range, 1..=24);
        assert_eq!(
            caps.typed_support,
            <SonyBRC300 as ProfileTypedSupport>::TYPED_SUPPORT,
            "discovery must mirror the static typed-support registry"
        );
        assert!(
            !caps.supports_typed(TypedSupportSurface::PtzOpticsPresetRecallSpeed),
            "BRC-300 metadata must not grant the distinct PTZOptics typed command"
        );
    }

    #[test]
    fn brc300_capabilities_keep_reverse_axis_degree_ranges_ordered() {
        let caps = Capabilities::from_profile::<SonyBRC300>();

        assert!(caps.pan_range_degrees.start() <= caps.pan_range_degrees.end());
        assert!(caps.tilt_range_degrees.start() <= caps.tilt_range_degrees.end());
        assert_eq!(
            *caps.pan_range_degrees.start(),
            0x08A58 as f32 / -208.0,
            "positive raw pan is the left/negative-degree endpoint"
        );
        assert_eq!(
            *caps.pan_range_degrees.end(),
            -0x08A58 as f32 / -208.0,
            "negative raw pan is the right/positive-degree endpoint"
        );
        assert_eq!(
            *caps.tilt_range_degrees.start(),
            0x493D as f32 / -208.0,
            "positive raw tilt is the up/negative-degree endpoint"
        );
        assert_eq!(
            *caps.tilt_range_degrees.end(),
            -0x186A as f32 / -208.0,
            "negative raw tilt is the down/positive-degree endpoint"
        );
    }

    #[test]
    fn test_unsupported_quality_ranges_are_none() {
        for caps in [
            Capabilities::from_profile::<SonyEVIH100>(),
            Capabilities::from_profile::<SonyBRC300>(),
            Capabilities::from_profile::<NearusBRC300>(),
            Capabilities::from_profile::<GenericVisca>(),
        ] {
            assert_eq!(caps.exposure_brightness_range, None);
            assert_eq!(caps.contrast_range, None);
            assert_eq!(caps.sharpness_range, None);
        }

        assert!(Capabilities::from_profile::<SonyEVIH100>().has_image_processing);
        assert!(!Capabilities::from_profile::<SonyBRC300>().has_image_processing);
    }

    #[test]
    fn test_optional_vendor_feature_metadata_matrix() {
        for caps in [
            Capabilities::from_profile::<PtzOpticsG2>(),
            Capabilities::from_profile::<PtzOpticsG3>(),
            Capabilities::from_profile::<PtzOptics30X>(),
        ] {
            assert!(!caps.has_one_push_focus);
            assert!(!caps.has_nd_filter);
            assert_eq!(caps.nd_filter_mode, crate::capabilities::NdFilterMode::None);
            assert!(!caps.has_motion_sync);
            assert_eq!(caps.max_motion_sync_speed, None);
            assert!(!caps.has_variable_speed);
        }

        let fr7 = Capabilities::from_profile::<SonyFR7>();
        assert!(fr7.has_nd_filter);
        assert_eq!(
            fr7.nd_filter_mode,
            crate::capabilities::NdFilterMode::Variable
        );
        assert!(!fr7.has_one_push_focus);
        assert!(!fr7.has_motion_sync);
        assert_eq!(fr7.max_motion_sync_speed, None);
        assert!(fr7.has_variable_speed);

        for caps in [
            Capabilities::from_profile::<GenericVisca>(),
            Capabilities::from_profile::<SonyBRCH900>(),
            Capabilities::from_profile::<SonyEVIH100>(),
            Capabilities::from_profile::<SonyBRC300>(),
            Capabilities::from_profile::<NearusBRC300>(),
        ] {
            assert!(!caps.has_one_push_focus);
            assert!(!caps.has_nd_filter);
            assert_eq!(caps.nd_filter_mode, crate::capabilities::NdFilterMode::None);
            assert!(!caps.has_motion_sync);
            assert_eq!(caps.max_motion_sync_speed, None);
            assert!(!caps.has_variable_speed);
        }
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
    fn test_zoom_unit_conversions() -> Result<(), crate::Error> {
        let caps = Capabilities::from_profile::<PtzOpticsG2>();

        // 1x magnification = 0 units
        let units = caps.magnification_to_zoom_units(1.0)?;
        assert_eq!(units, 0, "1x magnification should be 0 units");

        // 0 units = 1x magnification
        let mag = caps.zoom_units_to_magnification(0);
        assert!(
            (mag - 1.0).abs() < 0.01,
            "0 units should be 1x magnification"
        );

        // Round-trip conversion
        let original_mag = 10.0;
        let units = caps.magnification_to_zoom_units(original_mag)?;
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
        Ok(())
    }

    #[test]
    fn test_magnification_to_units_edge_cases() {
        let caps = Capabilities::from_profile::<PtzOpticsG2>();

        // Values below 1.0 are invalid and must not silently clamp to wide.
        assert!(caps.magnification_to_zoom_units(0.5).is_err());
        assert!(caps.magnification_to_zoom_units(0.0).is_err());
        assert!(caps.magnification_to_zoom_units(-1.0).is_err());
        assert!(caps.magnification_to_zoom_units(f32::NAN).is_err());
        assert!(caps.magnification_to_zoom_units(f32::INFINITY).is_err());
        assert!(caps
            .magnification_to_zoom_units(caps.max_combined_zoom() + 1.0)
            .is_err());
    }
}
