//! Variable speed mode capability trait for Sony FR7.
//!
//! The FR7 supports switching between 24-step and 50-step speed modes.

/// Trait for cameras that support variable speed mode control.
///
/// Only the Sony FR7 currently supports this feature, allowing
/// switching between 24-step (standard) and 50-step (fine) speed modes.
pub trait VariableSpeed {
    /// Whether the camera supports variable speed mode switching.
    ///
    /// Defaults to false. Only Sony FR7 overrides this to true.
    const SUPPORTS_VARIABLE_SPEED: bool = false;
}
