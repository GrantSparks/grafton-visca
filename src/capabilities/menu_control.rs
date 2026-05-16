//! Menu control capability trait for cameras with on-screen display (OSD) menu.

/// Trait for cameras that support menu control.
///
/// This trait indicates that a camera supports on-screen menu navigation
/// and control via VISCA commands. Most modern VISCA cameras support basic
/// menu control (display on/off, navigation, select/cancel), while some
/// cameras like the Sony FR7 support advanced direct menu control.
pub trait MenuCapability {
    /// Whether the camera supports advanced direct menu control.
    ///
    /// Sony FR7 supports additional menu control commands beyond basic navigation.
    const SUPPORTS_DIRECT_CONTROL: bool = false;
}

/// Marker trait for cameras that support direct menu control.
///
/// This trait is implemented for camera profiles that support advanced
/// direct menu control commands beyond basic navigation. Currently,
/// only the Sony FR7 supports this feature.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not declare direct menu control support",
    label = "profile `{Self}` does not implement `HasDirectMenuControl`",
    note = "see the built-in marker matrix in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support"
)]
pub trait HasDirectMenuControl: MenuCapability {}
