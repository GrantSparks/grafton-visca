//! Core profile metadata trait for camera identification and protocol configuration.

use std::time::Duration;

/// Level of VISCA inquiry command support for a camera profile.
///
/// VISCA cameras vary in their support for inquiry (status read-back) commands.
/// This enum lets consumers determine inquiry support per-profile without
/// matching on [`ProfileGroup`](crate::camera::profiles::ProfileGroup).
///
/// # Hardware Evidence
///
/// PTZOptics G2/G3/30X cameras have been confirmed via hardware testing to support
/// the complete VISCA inquiry command set, including all exposure, focus, image
/// processing, and color inquiries documented in the PTZOptics VISCA command list.
/// See [issue #498](https://github.com/GrantSparks/grafton-visca/issues/498) for
/// the full test results against a PTZOptics PT20X-NDI (G2 profile).
///
/// # Usage
///
/// ```
/// use grafton_visca::capabilities::InquirySupport;
/// use grafton_visca::profiles::ProfileId;
///
/// let profile = ProfileId::PtzOpticsG2;
/// match profile.inquiry_support() {
///     InquirySupport::Full => {
///         // Safe to use all inquiry commands
///     }
///     InquirySupport::Partial => {
///         // Basic inquiries work; advanced ones may not
///     }
///     InquirySupport::None => {
///         // Camera does not support inquiry commands
///     }
///     _ => {
///         // Future inquiry support categories should be handled conservatively
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum InquirySupport {
    /// Full inquiry support — all documented VISCA inquiry commands work correctly.
    ///
    /// Cameras with full support respond to every inquiry command in their VISCA
    /// documentation, including exposure mode, shutter, iris, focus mode,
    /// backlight, image processing settings, and block inquiries.
    ///
    /// Confirmed profiles: PTZOptics G2, PTZOptics G3, PTZOptics 30X,
    /// Sony FR7, Sony BRC-H900.
    Full,

    /// Partial inquiry support — basic inquiries work, but some may not respond.
    ///
    /// Position queries (zoom, pan/tilt, focus) and basic status queries work,
    /// but some feature-specific inquiries (exposure mode, image processing, etc.)
    /// may time out or return errors. Consumers should handle inquiry failures
    /// gracefully for cameras at this level.
    Partial,

    /// No inquiry command support.
    ///
    /// The camera does not respond to any VISCA inquiry commands.
    /// All inquiry operations will fail.
    None,
}

/// Core trait that all camera profiles must implement.
///
/// This trait provides essential metadata about the camera model including
/// its name, default address, and timing requirements. Protocol framing is
/// determined at compile-time through the Envelope associated type.
pub trait ProfileMetadata {
    /// Built-in profile identifier, when this profile is one of the registry
    /// backed profiles shipped by the crate.
    const PROFILE_ID: Option<crate::camera::profiles::ProfileId> = None;

    /// Camera model name for display/logging.
    const MODEL_NAME: &'static str;

    /// Default camera ID (usually 1).
    /// This is the VISCA address identifier for the camera.
    const DEFAULT_CAMERA_ID: u8;

    /// Protocol envelope type for compile-time protocol selection.
    /// This determines whether commands use raw VISCA or Sony encapsulation.
    type Envelope: crate::transport::Envelope;

    /// Maximum time to wait for command acknowledgment.
    const ACK_TIMEOUT: Duration;

    /// Maximum time to wait for command completion.
    const COMPLETION_TIMEOUT: Duration;

    /// Time camera is busy after certain operations.
    /// Some cameras need a delay after operations like preset recall.
    const BUSY_TIMEOUT: Duration = Duration::from_millis(0);

    /// Level of VISCA inquiry command support for this camera.
    ///
    /// Determines whether consumers can use inquiry commands to read camera state,
    /// and to what extent. See [`InquirySupport`] for detailed descriptions of
    /// each level.
    ///
    /// Default: [`InquirySupport::Full`] — most VISCA cameras support all inquiries.
    const INQUIRY_SUPPORT: InquirySupport = InquirySupport::Full;

    /// Whether this camera sends operation complete messages (0x51) after movements.
    ///
    /// Most VISCA-compliant cameras (Sony, Canon, Panasonic, PtzOptics, etc.)
    /// send completion messages when pan/tilt/zoom/focus operations finish.
    /// When true, movement detection can use event-driven completion instead of polling.
    const SUPPORTS_OPERATION_COMPLETE: bool = false;

    /// Minimum time spacing between consecutive inquiry sends.
    ///
    /// Some cameras (e.g., PTZOptics) cannot process inquiries faster than
    /// ~125-150ms apart. Setting this enforces a minimum delay between sends.
    ///
    /// Camera-specific values:
    /// - PTZOptics: 150ms (safe end of observed 125-150ms range)
    /// - Sony: 35ms (per VISCA spec: ~33ms = 2 video frames at 60fps)
    /// - Generic: 0ms (assume no limitation)
    ///
    /// Default: Duration::ZERO (no artificial spacing)
    const MIN_INQUIRY_SPACING: Duration = Duration::from_millis(0);

    /// Minimum time spacing between consecutive command sends (any kind).
    ///
    /// Consumer PTZ cameras have small internal command buffers that can
    /// overflow when commands are sent back-to-back at wire speed. This
    /// causes spurious 0x02 Syntax Error responses and dropped completions.
    ///
    /// This interval is enforced at the transport layer for all sends
    /// (commands and inquiries alike), so callers don't need to manage
    /// timing themselves. A value of Duration::ZERO disables pacing.
    ///
    /// Camera-specific values:
    /// - PTZOptics: 100ms (conservative for firmware buffer limitations)
    /// - Sony Professional: 35ms (per VISCA spec timing)
    /// - Generic: 0ms (assume no limitation)
    ///
    /// Default: Duration::ZERO (no artificial spacing)
    const MIN_COMMAND_SPACING: Duration = Duration::from_millis(0);
}

/// Marker trait indicating that a profile supports standard TCP construction.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not support TCP transport construction",
    label = "profile `{Self}` does not implement `SupportsTcp`",
    note = "use a transport supported by the selected profile; built-in profile transport support is registry-backed"
)]
pub trait SupportsTcp {
    /// Default TCP port for this profile.
    const DEFAULT_TCP_PORT: u16;
}

/// Marker trait indicating that a profile supports standard UDP construction.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not support UDP transport construction",
    label = "profile `{Self}` does not implement `SupportsUdp`",
    note = "use a transport supported by the selected profile; built-in profile transport support is registry-backed"
)]
pub trait SupportsUdp {
    /// Default UDP port for this profile.
    const DEFAULT_UDP_PORT: u16;
}

/// Marker trait indicating that a profile supports standard serial construction.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not support serial transport construction",
    label = "profile `{Self}` does not implement `SupportsSerial`",
    note = "serial support is modeled separately from IP transport support; use only source-backed serial profiles"
)]
pub trait SupportsSerial {}

// Marker traits for compile-time capability detection.
// These traits have no methods - they just mark a type as having a capability.

macro_rules! profile_capability_marker {
    (
        $(#[$doc:meta])*
        $vis:vis trait $trait_name:ident {
            message: $message:literal,
            label: $label:literal,
            note: $note:literal $(,)?
        }
    ) => {
        $(#[$doc])*
        #[diagnostic::on_unimplemented(
            message = $message,
            label = $label,
            note = $note
        )]
        $vis trait $trait_name {}
    };
}

/// Marker trait indicating support for pan/tilt movement.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not declare pan/tilt support",
    label = "profile `{Self}` does not implement `HasPanTilt`",
    note = "built-in marker support is documented in docs/camera_profile_support.md; add the marker bound only when every selected profile supports this typed surface"
)]
pub trait HasPanTilt {}

/// Marker trait indicating support for zoom control.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not declare zoom support",
    label = "profile `{Self}` does not implement `HasZoom`",
    note = "built-in marker support is documented in docs/camera_profile_support.md; add the marker bound only when every selected profile supports this typed surface"
)]
pub trait HasZoom {}

/// Marker trait indicating support for focus control.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not declare focus support",
    label = "profile `{Self}` does not implement `HasFocus`",
    note = "built-in marker support is documented in docs/camera_profile_support.md; add the marker bound only when every selected profile supports this typed surface"
)]
pub trait HasFocus {}

/// Marker trait indicating support for exposure control.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not declare exposure support",
    label = "profile `{Self}` does not implement `HasExposure`",
    note = "built-in marker support is documented in docs/camera_profile_support.md; add the marker bound only when every selected profile supports this typed surface"
)]
pub trait HasExposure {}

/// Marker trait indicating support for white balance.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not declare white-balance support",
    label = "profile `{Self}` does not implement `HasWhiteBalance`",
    note = "built-in marker support is documented in docs/camera_profile_support.md; add the marker bound only when every selected profile supports this typed surface"
)]
pub trait HasWhiteBalance {}

/// Marker trait indicating support for image processing.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not declare image-processing support",
    label = "profile `{Self}` does not implement `HasImageProcessing`",
    note = "built-in marker support is documented in docs/camera_profile_support.md; add the marker bound only when every selected profile supports this typed surface"
)]
pub trait HasImageProcessing {}

/// Marker trait indicating support for presets.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not declare preset support",
    label = "profile `{Self}` does not implement `HasPresets`",
    note = "built-in marker support is documented in docs/camera_profile_support.md; add the marker bound only when every selected profile supports this typed surface"
)]
pub trait HasPresets {}

/// Marker trait indicating support for power control.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not declare power-control support",
    label = "profile `{Self}` does not implement `HasPower`",
    note = "built-in marker support is documented in docs/camera_profile_support.md; add the marker bound only when every selected profile supports this typed surface"
)]
pub trait HasPower {}

/// Marker trait indicating support for menu control.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not declare menu-control support",
    label = "profile `{Self}` does not implement `HasMenuControl`",
    note = "built-in marker support is documented in docs/camera_profile_support.md; add the marker bound only when every selected profile supports this typed surface"
)]
pub trait HasMenuControl {}

profile_capability_marker! {
    /// Marker trait indicating typed Motion Sync API support.
    pub trait HasMotionSync {
        message: "profile `{Self}` does not declare typed Motion Sync support",
        label: "profile `{Self}` does not implement `HasMotionSync`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; use optional accessors, runtime feature detection, or a split trait instead of requiring this marker in a heterogeneous dyn-erased camera aggregate",
    }
}

profile_capability_marker! {
    /// Marker trait indicating typed variable speed API support.
    pub trait HasVariableSpeed {
        message: "profile `{Self}` does not declare typed variable-speed support",
        label: "profile `{Self}` does not implement `HasVariableSpeed`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; use optional accessors, runtime feature detection, or a split trait instead of requiring this marker in a heterogeneous dyn-erased camera aggregate",
    }
}

profile_capability_marker! {
    /// Marker trait indicating typed ND filter API support.
    pub trait HasNdFilter {
        message: "profile `{Self}` does not declare typed ND filter support",
        label: "profile `{Self}` does not implement `HasNdFilter`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; use optional accessors, runtime feature detection, or a split trait instead of requiring this marker in a heterogeneous dyn-erased camera aggregate",
    }
}

profile_capability_marker! {
    /// Marker trait indicating typed tally light API support.
    pub trait HasTally {
        message: "profile `{Self}` does not declare typed tally-light support",
        label: "profile `{Self}` does not implement `HasTally`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; do not require `HasTally` in a heterogeneous dyn-erased camera aggregate unless every profile supports tally",
    }
}

// Specific feature marker traits
profile_capability_marker! {
    /// Marker trait indicating support for exposure compensation.
    pub trait HasExposureCompensation {
        message: "profile `{Self}` does not declare exposure-compensation support",
        label: "profile `{Self}` does not implement `HasExposureCompensation`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for VISCA exposure brightness control and inquiry.
    pub trait HasBrightnessControl {
        message: "profile `{Self}` does not declare exposure brightness support",
        label: "profile `{Self}` does not implement `HasBrightnessControl`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for focus lock.
    ///
    /// Focus lock prevents any focus changes while enabled, useful for
    /// maintaining consistent focus during recording. This is a vendor-specific
    /// feature primarily supported by PtzOptics cameras.
    pub trait HasFocusLock {
        message: "profile `{Self}` does not declare focus-lock support",
        label: "profile `{Self}` does not implement `HasFocusLock`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for Push AF (Push Auto Focus).
    ///
    /// Push AF temporarily activates auto focus while the button is pressed,
    /// then returns to the previous focus mode. This is a vendor-specific
    /// feature primarily supported by Sony cameras.
    pub trait HasPushAutoFocus {
        message: "profile `{Self}` does not declare Push AF support",
        label: "profile `{Self}` does not implement `HasPushAutoFocus`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for direct absolute zoom positioning.
    ///
    /// Profiles without this marker may still support continuous tele/wide zoom,
    /// but the high-level typed API will not expose absolute zoom positioning.
    pub trait HasDirectZoom {
        message: "profile `{Self}` does not declare direct absolute zoom support",
        label: "profile `{Self}` does not implement `HasDirectZoom`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for the VISCA digital zoom on/off command.
    ///
    /// This marks the explicit enable/disable opcode, not necessarily absolute
    /// zoom positioning into a digital zoom range.
    pub trait HasDigitalZoomToggle {
        message: "profile `{Self}` does not declare VISCA digital zoom toggle support",
        label: "profile `{Self}` does not implement `HasDigitalZoomToggle`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for absolute zoom positions beyond the
    /// optical zoom range.
    pub trait HasDigitalZoomRange {
        message: "profile `{Self}` does not declare optical-plus-digital zoom range support",
        label: "profile `{Self}` does not implement `HasDigitalZoomRange`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for direct iris control and iris-priority
    /// exposure mode.
    pub trait HasIrisControl {
        message: "profile `{Self}` does not declare iris control support",
        label: "profile `{Self}` does not implement `HasIrisControl`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for standard one-push auto focus.
    pub trait HasOnePushFocus {
        message: "profile `{Self}` does not declare one-push focus support",
        label: "profile `{Self}` does not implement `HasOnePushFocus`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for PTZOptics snap focus.
    ///
    /// Snap focus is modeled separately from standard one-push AF because the
    /// vendor command is not assumed to be semantically identical.
    pub trait HasPtzOpticsSnapFocus {
        message: "profile `{Self}` does not declare PTZOptics snap focus support",
        label: "profile `{Self}` does not implement `HasPtzOpticsSnapFocus`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for focus zone selection.
    pub trait HasFocusZone {
        message: "profile `{Self}` does not declare focus-zone support",
        label: "profile `{Self}` does not implement `HasFocusZone`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for auto-focus sensitivity adjustment.
    pub trait HasAutoFocusSensitivity {
        message: "profile `{Self}` does not declare auto-focus sensitivity support",
        label: "profile `{Self}` does not implement `HasAutoFocusSensitivity`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for the focus near-limit inquiry command.
    pub trait HasFocusNearLimitInquiry {
        message: "profile `{Self}` does not declare focus near-limit inquiry support",
        label: "profile `{Self}` does not implement `HasFocusNearLimitInquiry`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for backlight compensation control.
    pub trait HasBacklightCompensation {
        message: "profile `{Self}` does not declare backlight compensation support",
        label: "profile `{Self}` does not implement `HasBacklightCompensation`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for wide dynamic range control.
    pub trait HasWideDynamicRange {
        message: "profile `{Self}` does not declare wide dynamic range support",
        label: "profile `{Self}` does not implement `HasWideDynamicRange`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for color-temperature white balance control.
    pub trait HasColorTemperature {
        message: "profile `{Self}` does not declare color-temperature support",
        label: "profile `{Self}` does not implement `HasColorTemperature`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for manual red/blue gain control.
    pub trait HasRgbGain {
        message: "profile `{Self}` does not declare RGB gain support",
        label: "profile `{Self}` does not implement `HasRgbGain`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for red/blue tuning control.
    pub trait HasRgbTuning {
        message: "profile `{Self}` does not declare RGB tuning support",
        label: "profile `{Self}` does not implement `HasRgbTuning`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for one-push white balance mode and trigger.
    pub trait HasOnePushWhiteBalance {
        message: "profile `{Self}` does not declare one-push white-balance support",
        label: "profile `{Self}` does not implement `HasOnePushWhiteBalance`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for auto-tracking white balance mode.
    pub trait HasAutoTrackingWhiteBalance {
        message: "profile `{Self}` does not declare auto-tracking white-balance support",
        label: "profile `{Self}` does not implement `HasAutoTrackingWhiteBalance`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for auto white-balance sensitivity control.
    pub trait HasAutoWhiteBalanceSensitivity {
        message: "profile `{Self}` does not declare auto white-balance sensitivity support",
        label: "profile `{Self}` does not implement `HasAutoWhiteBalanceSensitivity`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for vertical image flip control.
    pub trait HasImageFlip {
        message: "profile `{Self}` does not declare image flip support",
        label: "profile `{Self}` does not implement `HasImageFlip`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for horizontal image mirror control.
    pub trait HasImageMirror {
        message: "profile `{Self}` does not declare image mirror support",
        label: "profile `{Self}` does not implement `HasImageMirror`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for the combined image flip mode command.
    pub trait HasCombinedImageFlip {
        message: "profile `{Self}` does not declare combined image flip mode support",
        label: "profile `{Self}` does not implement `HasCombinedImageFlip`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for saturation control and inquiry.
    pub trait HasSaturationControl {
        message: "profile `{Self}` does not declare saturation control support",
        label: "profile `{Self}` does not implement `HasSaturationControl`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for contrast control and inquiry.
    pub trait HasContrastControl {
        message: "profile `{Self}` does not declare contrast control support",
        label: "profile `{Self}` does not implement `HasContrastControl`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for sharpness control and inquiry.
    pub trait HasSharpnessControl {
        message: "profile `{Self}` does not declare sharpness control support",
        label: "profile `{Self}` does not implement `HasSharpnessControl`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for hue control and inquiry.
    pub trait HasHueControl {
        message: "profile `{Self}` does not declare hue control support",
        label: "profile `{Self}` does not implement `HasHueControl`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for luminance control and inquiry.
    pub trait HasLuminanceControl {
        message: "profile `{Self}` does not declare luminance control support",
        label: "profile `{Self}` does not implement `HasLuminanceControl`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for gamma control and inquiry.
    pub trait HasGammaControl {
        message: "profile `{Self}` does not declare gamma control support",
        label: "profile `{Self}` does not implement `HasGammaControl`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for aggregate noise-reduction inquiry.
    pub trait HasNoiseReduction {
        message: "profile `{Self}` does not declare aggregate noise-reduction inquiry support",
        label: "profile `{Self}` does not implement `HasNoiseReduction`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for 2D noise-reduction control and inquiry.
    pub trait HasNoiseReduction2D {
        message: "profile `{Self}` does not declare 2D noise-reduction support",
        label: "profile `{Self}` does not implement `HasNoiseReduction2D`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for 3D noise-reduction control and inquiry.
    pub trait HasNoiseReduction3D {
        message: "profile `{Self}` does not declare 3D noise-reduction support",
        label: "profile `{Self}` does not implement `HasNoiseReduction3D`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

profile_capability_marker! {
    /// Marker trait indicating support for picture-effect control and inquiry.
    pub trait HasPictureEffect {
        message: "profile `{Self}` does not declare picture-effect support",
        label: "profile `{Self}` does not implement `HasPictureEffect`",
        note: "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support",
    }
}

// Specialized blanket implementations for baseline marker traits.
// These automatically implement the marker trait for any type that implements
// both ProfileMetadata and the corresponding capability trait.

impl<T: ProfileMetadata + crate::capabilities::PanTilt> HasPanTilt for T {}
impl<T: ProfileMetadata + crate::capabilities::Zoom> HasZoom for T {}
impl<T: ProfileMetadata + crate::capabilities::Focus> HasFocus for T {}
impl<T: ProfileMetadata + crate::capabilities::Exposure> HasExposure for T {}
impl<T: ProfileMetadata + crate::capabilities::WhiteBalance> HasWhiteBalance for T {}
impl<T: ProfileMetadata + crate::capabilities::ImageProcessing> HasImageProcessing for T {}
impl<T: ProfileMetadata + crate::capabilities::Presets> HasPresets for T {}
impl<T: ProfileMetadata + crate::capabilities::Power> HasPower for T {}
impl<T: ProfileMetadata + crate::capabilities::MenuCapability> HasMenuControl for T {}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCamera;

    impl ProfileMetadata for TestCamera {
        const MODEL_NAME: &'static str = "Test Camera";
        const DEFAULT_CAMERA_ID: u8 = 1;
        type Envelope = crate::transport::RawVisca;
        const ACK_TIMEOUT: Duration = Duration::from_millis(100);
        const COMPLETION_TIMEOUT: Duration = Duration::from_millis(5000);
    }

    #[test]
    fn test_envelope_type() {
        // Test that we can create an envelope from the profile's associated type
        use crate::transport::{AddressingMode, Envelope};

        let envelope = <TestCamera as ProfileMetadata>::Envelope::new(AddressingMode::Ip);
        // The fact this compiles proves the envelope type is correct
        let _envelope_clone = envelope; // Move to prove it's sized
    }
}
