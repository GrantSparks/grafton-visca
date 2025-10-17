//! Core profile metadata trait for camera identification and protocol configuration.

use std::time::Duration;

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
    type Envelope: crate::transport::envelope::Envelope;

    /// Maximum time to wait for command acknowledgment.
    const ACK_TIMEOUT: Duration;

    /// Maximum time to wait for command completion.
    const COMPLETION_TIMEOUT: Duration;

    /// Time camera is busy after certain operations.
    /// Some cameras need a delay after operations like preset recall.
    const BUSY_TIMEOUT: Duration = Duration::from_millis(0);

    /// Whether this camera supports VISCA inquiry commands.
    const SUPPORTS_INQUIRY: bool = true;

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

/// Marker trait indicating support for motion sync.
pub trait HasMotionSync {}

/// Marker trait indicating support for variable speed.
pub trait HasVariableSpeed {}

/// Marker trait indicating support for ND filter.
pub trait HasNdFilter {}

// Specific feature marker traits
/// Marker trait indicating support for exposure compensation.
pub trait HasExposureCompensation {}

/// Marker trait indicating support for color temperature control.
pub trait HasColorTemperature {}

/// Marker trait indicating support for RGB gain control.
pub trait HasRGBGain {}

/// Marker trait indicating support for hue control.
pub trait HasHue {}

/// Marker trait indicating support for luminance control.
pub trait HasLuminance {}

/// Marker trait indicating support for backlight compensation.
pub trait HasBacklightCompensation {}

/// Marker trait indicating support for WDR (Wide Dynamic Range).
pub trait HasWDR {}

/// Marker trait indicating support for auto focus.
pub trait HasAutoFocus {}

/// Marker trait indicating support for one push focus.
pub trait HasOnePushFocus {}

/// Marker trait indicating support for one push white balance.
pub trait HasOnePushWhiteBalance {}

/// Marker trait indicating support for auto exposure.
pub trait HasAutoExposure {}

/// Marker trait indicating support for tally light control.
pub trait HasTally {}

/// Marker trait indicating support for picture effect modes.
pub trait HasPictureEffect {}

// Specialized blanket implementations for each marker trait.
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
impl<T: ProfileMetadata + crate::capabilities::MotionSync> HasMotionSync for T {}
impl<T: ProfileMetadata + crate::capabilities::VariableSpeed> HasVariableSpeed for T {}
impl<T: ProfileMetadata + crate::capabilities::NdFilter> HasNdFilter for T {}
impl<T: ProfileMetadata + crate::capabilities::Tally> HasTally for T {}

// Note: Unlike the capability marker traits (HasPanTilt, HasZoom, etc.) which have
// blanket implementations, these specific feature marker traits must be manually
// implemented for each camera profile that supports them. This is because Rust
// doesn't support const equality in trait bounds in stable Rust.
//
// Example implementation in camera profiles:
// impl HasExposureCompensation for PtzOpticsG2 {}
// impl HasAutoExposure for PtzOpticsG2 {}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCamera;

    impl ProfileMetadata for TestCamera {
        const MODEL_NAME: &'static str = "Test Camera";
        const DEFAULT_CAMERA_ID: u8 = 1;
        type Envelope = crate::transport::envelope::RawVisca;
        const ACK_TIMEOUT: Duration = Duration::from_millis(100);
        const COMPLETION_TIMEOUT: Duration = Duration::from_millis(5000);
        const DEFAULT_TCP_PORT: u16 = 5678;
        const DEFAULT_UDP_PORT: u16 = 1259;
    }

    #[test]
    fn test_envelope_type() {
        // Test that we can create an envelope from the profile's associated type
        use crate::transport::builder::AddressingMode;
        use crate::transport::envelope::Envelope;

        let envelope = <TestCamera as ProfileMetadata>::Envelope::new(AddressingMode::Ip);
        // The fact this compiles proves the envelope type is correct
        let _envelope_clone = envelope; // Move to prove it's sized
    }
}
