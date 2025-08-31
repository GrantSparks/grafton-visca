//! Core profile metadata trait for camera identification and protocol configuration.

use std::time::Duration;

use crate::capabilities::CameraFeature;

/// Core trait that all camera profiles must implement.
///
/// This trait provides essential metadata about the camera model including
/// its name, default address, protocol style, and timing requirements.
pub trait ProfileMetadata {
    /// Camera model name for display/logging.
    const MODEL_NAME: &'static str;

    /// Default VISCA address (usually 1).
    const DEFAULT_ADDRESS: u8;

    /// Protocol style affects framing and headers.
    const PROTOCOL_STYLE: ProtocolStyle;

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

/// Protocol style determines how VISCA commands are framed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolStyle {
    /// Raw VISCA protocol (PtzOptics, generic cameras).
    /// Commands are sent as-is without additional framing.
    RawVisca,

    /// Sony 8-byte encapsulated protocol with sequence numbers.
    /// Commands are wrapped in an 8-byte header with optional sequence tracking.
    SonyEncapsulated {
        /// Whether to use sequence numbers for command tracking.
        use_sequence: bool,
    },
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

/// Extension trait for profile introspection.
///
/// This trait provides runtime capability discovery methods. While the
/// primary API uses compile-time trait bounds, these methods can be
/// useful for debugging, logging, or dynamic UI generation.
pub trait ProfileIntrospection: ProfileMetadata {
    /// Check if camera supports pan/tilt movement.
    fn supports_pan_tilt(&self) -> bool {
        false // Default, overridden by blanket impls
    }

    /// Check if camera supports zoom control.
    fn supports_zoom(&self) -> bool {
        false
    }

    /// Check if camera supports focus control.
    fn supports_focus(&self) -> bool {
        false
    }

    /// Check if camera supports exposure control.
    fn supports_exposure(&self) -> bool {
        false
    }

    /// Check if camera supports white balance.
    fn supports_white_balance(&self) -> bool {
        false
    }

    /// Check if camera supports image processing.
    fn supports_image_processing(&self) -> bool {
        false
    }

    /// Check if camera supports presets.
    fn supports_presets(&self) -> bool {
        false
    }

    /// Check if camera supports power control.
    fn supports_power(&self) -> bool {
        false
    }

    /// Check if camera supports ND filter.
    fn supports_nd_filter(&self) -> bool {
        false
    }

    /// Get a list of supported camera features.
    fn supported_features(&self) -> Vec<CameraFeature> {
        let mut features = Vec::new();

        if self.supports_pan_tilt() {
            features.push(CameraFeature::PanTilt);
        }
        if self.supports_zoom() {
            features.push(CameraFeature::Zoom);
        }
        if self.supports_focus() {
            features.push(CameraFeature::Focus);
        }
        if self.supports_exposure() {
            features.push(CameraFeature::Exposure);
        }
        if self.supports_white_balance() {
            features.push(CameraFeature::WhiteBalance);
        }
        if self.supports_image_processing() {
            features.push(CameraFeature::ImageProcessing);
        }
        if self.supports_presets() {
            features.push(CameraFeature::Presets);
        }
        if self.supports_power() {
            features.push(CameraFeature::Power);
        }
        if self.supports_nd_filter() {
            features.push(CameraFeature::NdFilter);
        }

        features
    }

    /// Get a human-readable summary of camera capabilities.
    fn capability_summary(&self) -> String {
        let features = self.supported_features();
        let capabilities: Vec<&str> = features.iter().map(|f| f.name()).collect();

        format!(
            "{} - Protocol: {:?} - Capabilities: {}",
            Self::MODEL_NAME,
            Self::PROTOCOL_STYLE,
            if capabilities.is_empty() {
                "None".to_string()
            } else {
                capabilities.join(", ")
            }
        )
    }

    /// Get a detailed description of supported features.
    fn feature_descriptions(&self) -> Vec<(CameraFeature, &'static str)> {
        self.supported_features()
            .into_iter()
            .map(|feature| (feature, feature.description()))
            .collect()
    }
}

// Blanket implementation for all types with ProfileMetadata
// Note: Since Rust doesn't support trait specialization, the introspection
// methods will need to be implemented manually for each profile based on
// which capability traits it implements.
impl<T: ProfileMetadata> ProfileIntrospection for T {}

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
impl<T: ProfileMetadata + crate::capabilities::MenuControl> HasMenuControl for T {}
impl<T: ProfileMetadata + crate::capabilities::MotionSync> HasMotionSync for T {}
impl<T: ProfileMetadata + crate::capabilities::VariableSpeed> HasVariableSpeed for T {}
impl<T: ProfileMetadata + crate::capabilities::NdFilter> HasNdFilter for T {}

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
        const DEFAULT_ADDRESS: u8 = 1;
        const PROTOCOL_STYLE: ProtocolStyle = ProtocolStyle::RawVisca;
        const ACK_TIMEOUT: Duration = Duration::from_millis(100);
        const COMPLETION_TIMEOUT: Duration = Duration::from_millis(5000);
        const DEFAULT_TCP_PORT: u16 = 5678;
        const DEFAULT_UDP_PORT: u16 = 1259;
    }

    #[test]
    fn test_protocol_style() {
        assert_eq!(TestCamera::PROTOCOL_STYLE, ProtocolStyle::RawVisca);

        let sony_style = ProtocolStyle::SonyEncapsulated { use_sequence: true };
        assert!(matches!(
            sony_style,
            ProtocolStyle::SonyEncapsulated { use_sequence: true }
        ));
    }

    #[test]
    fn test_introspection() {
        let camera = TestCamera;

        // Without implementing capability traits, all return false
        assert!(!camera.supports_pan_tilt());
        assert!(!camera.supports_zoom());
        assert!(!camera.supports_nd_filter());

        // Summary shows no capabilities
        let summary = camera.capability_summary();
        assert!(summary.contains("Test Camera"));
        assert!(summary.contains("RawVisca"));
    }
}
