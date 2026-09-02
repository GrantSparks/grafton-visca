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
/// PTZOptics G2/G3/30X cameras have baseline VISCA inquiry support confirmed by
/// hardware testing. Model-specific optional inquiries still require their own
/// source-backed typed-support gate; a profile-wide `Full` value is never a
/// fallback permission for an optional accessor. See
/// [issue #498](https://github.com/GrantSparks/grafton-visca/issues/498) for
/// the tested G2 baseline.
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
///         // Baseline inquiries are available; optional accessors still
///         // require their profile-specific typed gates.
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
    /// Full baseline inquiry support for the documented profile family.
    ///
    /// Cameras in this category support the baseline VISCA inquiry set used by
    /// their profile. Optional model-specific inquiries remain independently
    /// gated by typed support markers and runtime capability facts.
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

    /// Exact command completion deadlines for the five request categories.
    const COMMAND_TIMEOUTS: crate::CommandTimeouts;

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

    /// Exact per-axis position-inquiry support used by motion observation.
    ///
    /// Runtime profiles must carry this fact in their [`ProfileSpec`](crate::ProfileSpec).
    /// Static profiles may override this associated constant directly; built-in
    /// registry profiles do so from their per-axis protocol facts. The default is
    /// intentionally conservative for profiles that only declare the coarse
    /// [`Self::INQUIRY_SUPPORT`] level: no inquiry support means no position polling,
    /// while any inquiry support permits the three baseline PTZ position
    /// inquiries. Iris and ND-filter inquiries remain conservative unless a
    /// profile overrides this constant with explicit scalar facts.
    const POSITION_INQUIRY_SUPPORT: crate::profile::PositionInquirySupport =
        match Self::INQUIRY_SUPPORT {
            InquirySupport::None => {
                crate::profile::PositionInquirySupport::new(false, false, false)
            }
            InquirySupport::Full | InquirySupport::Partial => {
                crate::profile::PositionInquirySupport::new(true, true, true)
            }
        };

    /// Whether this camera sends operation complete messages (0x51) after movements.
    ///
    /// Most VISCA-compliant cameras (Sony, Canon, Panasonic, PtzOptics, etc.)
    /// send completion messages when pan/tilt/zoom/focus operations finish.
    /// When true, movement detection can use event-driven completion instead of polling.
    const SUPPORTS_OPERATION_COMPLETE: bool = false;

    /// Whether the camera accepts the standard VISCA socket-cancel command.
    ///
    /// This controls cancellation after a command has been sent. Commands that
    /// are still queued can always be removed locally without camera support.
    const SUPPORTS_COMMAND_CANCEL: bool = true;

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

    /// Whether this profile has source-backed USB audio support.
    ///
    /// The typed [`crate::capabilities::TypedSupportSurface::UsbAudio`]
    /// permission remains the authority for exposing a static operation; this
    /// fact lets dynamic validation reject a profile that claims the typed
    /// surface without the required model capability. Downstream profiles
    /// remain conservative unless they opt in explicitly.
    const SUPPORTS_USB_AUDIO: bool = false;
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
    note = "built-in marker support is registry-backed; downstream profiles must implement this marker explicitly only for a source-backed typed image noun"
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

// Specialized blanket implementations for baseline marker traits whose
// metadata traits intrinsically establish the baseline surface. Image metadata
// is intentionally different: every `Profile` supplies `ImageProcessing`
// metadata, including profiles with no image noun at all. Its marker is emitted
// by the built-in registry from a source-backed base-support fact, and custom
// profiles opt in explicitly.

impl<T: ProfileMetadata + crate::capabilities::PanTilt> HasPanTilt for T {}
impl<T: ProfileMetadata + crate::capabilities::Zoom> HasZoom for T {}
impl<T: ProfileMetadata + crate::capabilities::Focus> HasFocus for T {}
impl<T: ProfileMetadata + crate::capabilities::Exposure> HasExposure for T {}
impl<T: ProfileMetadata + crate::capabilities::WhiteBalance> HasWhiteBalance for T {}
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
        const COMMAND_TIMEOUTS: crate::CommandTimeouts = crate::CommandTimeouts::new(
            Duration::from_secs(5),
            Duration::from_secs(30),
            Duration::from_secs(60),
            Duration::from_secs(300),
            Duration::from_secs(5),
        );
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
