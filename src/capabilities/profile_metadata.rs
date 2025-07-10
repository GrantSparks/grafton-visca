//! Core profile metadata trait for camera identification and protocol configuration.

use std::time::Duration;

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
}

/// Protocol style determines how VISCA commands are framed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolStyle {
    /// Raw VISCA protocol (PTZOptics, generic cameras).
    /// Commands are sent as-is without additional framing.
    RawVisca,

    /// Sony 8-byte encapsulated protocol with sequence numbers.
    /// Commands are wrapped in an 8-byte header with optional sequence tracking.
    SonyEncapsulated {
        /// Whether to use sequence numbers for command tracking.
        use_sequence: bool,
    },
}

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

    /// Get a human-readable summary of camera capabilities.
    fn capability_summary(&self) -> String {
        let mut capabilities = Vec::new();

        if self.supports_pan_tilt() {
            capabilities.push("Pan/Tilt");
        }
        if self.supports_zoom() {
            capabilities.push("Zoom");
        }
        if self.supports_focus() {
            capabilities.push("Focus");
        }
        if self.supports_exposure() {
            capabilities.push("Exposure");
        }
        if self.supports_white_balance() {
            capabilities.push("White Balance");
        }
        if self.supports_image_processing() {
            capabilities.push("Image Processing");
        }
        if self.supports_presets() {
            capabilities.push("Presets");
        }
        if self.supports_power() {
            capabilities.push("Power");
        }
        if self.supports_nd_filter() {
            capabilities.push("ND Filter");
        }

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
}

// Blanket implementation for all types with ProfileMetadata
impl<T: ProfileMetadata> ProfileIntrospection for T {}

// Note: We can't add specialization for specific capabilities due to
// Rust's orphan rule and lack of trait specialization. The introspection
// methods will need to return false by default in the trait definition.

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
