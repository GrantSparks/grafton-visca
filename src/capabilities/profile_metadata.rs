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
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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

    /// Default TCP port for this camera profile.
    const DEFAULT_TCP_PORT: u16;

    /// Default UDP port for this camera profile.
    const DEFAULT_UDP_PORT: u16;

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

// Marker traits for compile-time capability detection.
// These traits have no methods - they just mark a type as having a capability.

/// Marker trait indicating support for pan/tilt movement.
pub trait HasPanTilt {}

/// Marker trait indicating support for zoom control.
pub trait HasZoom {}

/// Marker trait indicating support for focus control.
pub trait HasFocus {}

/// Marker trait indicating support for exposure control.
pub trait HasExposure {}

/// Marker trait indicating support for white balance.
pub trait HasWhiteBalance {}

/// Marker trait indicating support for image processing.
pub trait HasImageProcessing {}

/// Marker trait indicating support for presets.
pub trait HasPresets {}

/// Marker trait indicating support for power control.
pub trait HasPower {}

/// Marker trait indicating support for menu control.
pub trait HasMenuControl {}

/// Marker trait indicating typed Motion Sync API support.
pub trait HasMotionSync {}

/// Marker trait indicating typed variable speed API support.
pub trait HasVariableSpeed {}

/// Marker trait indicating typed ND filter API support.
pub trait HasNdFilter {}

// Specific feature marker traits
/// Marker trait indicating support for exposure compensation.
pub trait HasExposureCompensation {}

/// Marker trait indicating support for focus lock.
///
/// Focus lock prevents any focus changes while enabled, useful for
/// maintaining consistent focus during recording. This is a vendor-specific
/// feature primarily supported by PtzOptics cameras.
pub trait HasFocusLock {}

/// Marker trait indicating support for Push AF (Push Auto Focus).
///
/// Push AF temporarily activates auto focus while the button is pressed,
/// then returns to the previous focus mode. This is a vendor-specific
/// feature primarily supported by Sony cameras.
pub trait HasPushAutoFocus {}

/// Marker trait indicating support for direct absolute zoom positioning.
///
/// Profiles without this marker may still support continuous tele/wide zoom,
/// but the high-level typed API will not expose absolute zoom positioning.
pub trait HasDirectZoom {}

/// Marker trait indicating support for the VISCA digital zoom on/off command.
///
/// This marks the explicit enable/disable opcode, not necessarily absolute
/// zoom positioning into a digital zoom range.
pub trait HasDigitalZoomToggle {}

/// Marker trait indicating support for absolute zoom positions beyond the
/// optical zoom range.
pub trait HasDigitalZoomRange {}

/// Marker trait indicating support for direct iris control and iris-priority
/// exposure mode.
pub trait HasIrisControl {}

/// Marker trait indicating support for standard one-push auto focus.
pub trait HasOnePushFocus {}

/// Marker trait indicating support for PTZOptics snap focus.
///
/// Snap focus is modeled separately from standard one-push AF because the
/// vendor command is not assumed to be semantically identical.
pub trait HasPtzOpticsSnapFocus {}

/// Marker trait indicating support for focus zone selection.
pub trait HasFocusZone {}

/// Marker trait indicating support for auto-focus sensitivity adjustment.
pub trait HasAutoFocusSensitivity {}

/// Marker trait indicating support for the focus near-limit inquiry command.
pub trait HasFocusNearLimitInquiry {}

/// Marker trait indicating support for backlight compensation control.
pub trait HasBacklightCompensation {}

/// Marker trait indicating support for wide dynamic range control.
pub trait HasWideDynamicRange {}

/// Marker trait indicating support for color-temperature white balance control.
pub trait HasColorTemperature {}

/// Marker trait indicating support for manual red/blue gain control.
pub trait HasRgbGain {}

/// Marker trait indicating support for red/blue tuning control.
pub trait HasRgbTuning {}

/// Marker trait indicating support for one-push white balance mode and trigger.
pub trait HasOnePushWhiteBalance {}

/// Marker trait indicating support for auto-tracking white balance mode.
pub trait HasAutoTrackingWhiteBalance {}

/// Marker trait indicating support for auto white-balance sensitivity control.
pub trait HasAutoWhiteBalanceSensitivity {}

/// Marker trait indicating support for vertical image flip control.
pub trait HasImageFlip {}

/// Marker trait indicating support for horizontal image mirror control.
pub trait HasImageMirror {}

/// Marker trait indicating support for the combined image flip mode command.
pub trait HasCombinedImageFlip {}

/// Marker trait indicating support for saturation control and inquiry.
pub trait HasSaturationControl {}

/// Marker trait indicating support for hue control and inquiry.
pub trait HasHueControl {}

/// Marker trait indicating support for luminance control and inquiry.
pub trait HasLuminanceControl {}

/// Marker trait indicating support for gamma control and inquiry.
pub trait HasGammaControl {}

/// Marker trait indicating support for aggregate noise-reduction inquiry.
pub trait HasNoiseReduction {}

/// Marker trait indicating support for 2D noise-reduction control and inquiry.
pub trait HasNoiseReduction2D {}

/// Marker trait indicating support for 3D noise-reduction control and inquiry.
pub trait HasNoiseReduction3D {}

/// Marker trait indicating support for picture-effect control and inquiry.
pub trait HasPictureEffect {}

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
        const DEFAULT_TCP_PORT: u16 = 5678;
        const DEFAULT_UDP_PORT: u16 = 1259;
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
