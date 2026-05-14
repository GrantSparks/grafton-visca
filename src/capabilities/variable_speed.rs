//! Variable speed mode metadata trait for camera profiles.
//!
//! The FR7 supports switching between 24-step and 50-step speed modes.

/// Metadata for a camera profile's variable speed mode capability.
///
/// This trait supplies runtime discovery defaults. It does not mean the typed
/// variable speed control API is available for a profile; use
/// [`crate::capabilities::HasVariableSpeed`] for that compile-time support
/// marker.
pub trait VariableSpeedMetadata {
    /// Whether the camera supports variable speed mode switching.
    ///
    /// Defaults to false. Only Sony FR7 overrides this to true.
    const SUPPORTS_VARIABLE_SPEED: bool = false;
}
