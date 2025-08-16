//! Camera feature enumeration for documentation and logging.
//!
//! This module provides an enumeration of all camera features that can be
//! used for logging, documentation, and human-readable output.

/// Enumeration of all camera features that can be queried.
///
/// This enum provides a comprehensive list of all features that may be
/// supported by different camera models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CameraFeature {
    // Movement features
    /// Pan and tilt movement control
    PanTilt,
    /// Zoom control
    Zoom,
    /// Motion sync for coordinated movement
    MotionSync,

    // Focus features
    /// Manual and automatic focus control
    Focus,
    /// Focus lock/unlock
    FocusLock,

    // Exposure features
    /// Exposure mode and settings
    Exposure,
    /// Gain control
    Gain,
    /// Shutter speed control
    Shutter,
    /// Iris/aperture control
    Iris,
    /// Backlight compensation
    Backlight,

    // Image features
    /// White balance control
    WhiteBalance,
    /// Image processing (brightness, contrast, etc.)
    ImageProcessing,
    /// Color saturation control
    ColorSaturation,
    /// Gamma correction
    Gamma,
    /// Black and white mode
    BlackWhiteMode,
    /// Noise reduction
    NoiseReduction,
    /// Sharpness control
    Sharpness,

    // Camera control features
    /// Power on/off control
    Power,
    /// Preset positions
    Presets,
    /// Tally light control
    Tally,
    /// Image freeze
    ImageFreeze,
    /// Image flip
    ImageFlip,
    /// Picture effects (negative, sepia, B&W, etc.)
    PictureEffect,

    // Advanced features
    /// ND filter control (Sony FR7)
    NDFilter,
    /// Variable speed mode (Sony FR7)
    VariableSpeedMode,
    /// Menu control (Sony FR7)
    MenuControl,
    /// Privacy mode
    Privacy,

    // System features
    /// System reset
    SystemReset,
    /// Command cancel
    CommandCancel,
    /// Ndi streaming features (PtzOptics)
    Ndi,
}

impl CameraFeature {
    /// Get a human-readable name for the feature.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::PanTilt => "Pan/Tilt",
            Self::Zoom => "Zoom",
            Self::MotionSync => "Motion Sync",
            Self::Focus => "Focus",
            Self::FocusLock => "Focus Lock",
            Self::Exposure => "Exposure",
            Self::Gain => "Gain",
            Self::Shutter => "Shutter",
            Self::Iris => "Iris",
            Self::Backlight => "Backlight",
            Self::WhiteBalance => "White Balance",
            Self::ImageProcessing => "Image Processing",
            Self::ColorSaturation => "Color Saturation",
            Self::Gamma => "Gamma",
            Self::BlackWhiteMode => "Black & White Mode",
            Self::NoiseReduction => "Noise Reduction",
            Self::Sharpness => "Sharpness",
            Self::Power => "Power",
            Self::Presets => "Presets",
            Self::Tally => "Tally Light",
            Self::ImageFreeze => "Image Freeze",
            Self::ImageFlip => "Image Flip",
            Self::NDFilter => "ND Filter",
            Self::VariableSpeedMode => "Variable Speed Mode",
            Self::MenuControl => "Menu Control",
            Self::Privacy => "Privacy Mode",
            Self::SystemReset => "System Reset",
            Self::CommandCancel => "Command Cancel",
            Self::PictureEffect => "Picture Effect",
            Self::Ndi => "Ndi Streaming",
        }
    }

    /// Get a description of the feature.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::PanTilt => "Control camera pan and tilt movement",
            Self::Zoom => "Control camera zoom in/out",
            Self::MotionSync => "Coordinate pan, tilt, and zoom movements",
            Self::Focus => "Control manual and automatic focus",
            Self::FocusLock => "Lock/unlock focus position",
            Self::Exposure => "Control exposure mode and settings",
            Self::Gain => "Adjust camera gain/ISO",
            Self::Shutter => "Control shutter speed",
            Self::Iris => "Control iris/aperture",
            Self::Backlight => "Enable/disable backlight compensation",
            Self::WhiteBalance => "Adjust white balance settings",
            Self::ImageProcessing => "Adjust brightness, contrast, and other image parameters",
            Self::ColorSaturation => "Adjust color saturation level",
            Self::Gamma => "Adjust gamma correction",
            Self::BlackWhiteMode => "Enable/disable black and white mode",
            Self::NoiseReduction => "Configure noise reduction settings",
            Self::Sharpness => "Adjust image sharpness",
            Self::Power => "Control camera power on/off",
            Self::Presets => "Save and recall preset positions",
            Self::Tally => "Control tally light indicators",
            Self::ImageFreeze => "Freeze/unfreeze current image",
            Self::ImageFlip => "Flip image horizontally or vertically",
            Self::NDFilter => "Control neutral density filter",
            Self::VariableSpeedMode => "Enable variable speed pan/tilt/zoom",
            Self::MenuControl => "Navigate camera on-screen menu",
            Self::Privacy => "Enable/disable privacy mode",
            Self::SystemReset => "Reset camera to factory defaults",
            Self::CommandCancel => "Cancel pending commands",
            Self::PictureEffect => "Apply picture effects (negative, sepia, B&W, etc.)",
            Self::Ndi => "Control Ndi streaming settings",
        }
    }
}
