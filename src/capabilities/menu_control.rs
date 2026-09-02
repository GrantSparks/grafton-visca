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

macro_rules! define_direct_menu_marker_entry {
    (
        DirectMenu,
        $marker:ident,
        $marker_doc:literal,
        $diagnostic:literal
    ) => {
        #[doc = $marker_doc]
        #[diagnostic::on_unimplemented(
            message = $diagnostic,
            label = "profile does not implement the required typed-support marker",
            note = "see the generated marker tables in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support"
        )]
        pub trait $marker: MenuCapability {}
    };
    (
        $surface:ident,
        $marker:ident,
        $marker_doc:literal,
        $diagnostic:literal
    ) => {};
}

macro_rules! define_direct_menu_marker {
    (
        [
            $(
                {
                    surface: $surface:ident,
                    marker: $marker:ident,
                    bit: $bit:literal,
                    wire: $wire:literal,
                    area: $area:literal,
                    api: $api:literal,
                    surface_doc: $surface_doc:literal,
                    marker_doc: $marker_doc:literal,
                    diagnostic: $diagnostic:literal,
                },
            )*
        ]
    ) => {
        $(define_direct_menu_marker_entry!(
            $surface,
            $marker,
            $marker_doc,
            $diagnostic
        );)*
    };
}

super::typed_support_registry::typed_support_registry!(define_direct_menu_marker);
