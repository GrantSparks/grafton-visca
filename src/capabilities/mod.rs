//! Evidence-backed profile permissions and runtime capability records.
//!
//! `Has*` markers describe source-backed compile-time guarantees. Runtime
//! callers inspect [`crate::capabilities::Capabilities::supports`] and the optional domain records;
//! a permission does not manufacture numeric facts for an absent domain.

mod discovery;
mod domains;
mod encodings;
pub(crate) mod exposure;
pub(crate) mod pan_tilt;
mod permits;
mod profile_metadata;
#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) use permits::permit_gate;
pub use permits::{gate, Permits};
mod typed_support;
pub(crate) mod typed_support_registry;
mod types;
pub(crate) mod zoom;

pub use discovery::Capabilities;
#[cfg(test)]
pub(crate) use discovery::SurfaceMetadata;
pub use domains::{
    EndpointQuantization, ExposureCaps, FocusCaps, FocusNearEnd, ImageCaps, MenuCaps, PanTiltCaps,
    PanTiltGeometry, PowerCaps, PowerStatusQuery, PresetCaps, RgbGainRanges, RgbTuningRanges,
    TiltRanges, WhiteBalanceCaps, ZoomCaps,
};
pub(crate) use encodings::encoding_selection;
pub use encodings::*;
pub use exposure::ShutterSpeedEntry;
pub use pan_tilt::PanTiltWireCodec;
pub use profile_metadata::{
    Domain, DomainSet, HasExposure, HasFocus, HasFocusSettlement, HasImage, HasMenu, HasPanTilt,
    HasPanTiltSettlement, HasPower, HasPresetRecallSettlement, HasPresets, HasWhiteBalance,
    HasZoom, SettlementSupport, SupportsSerial, SupportsTcp, SupportsUdp, TransportSet,
};
pub(crate) use typed_support::typed_surface;
pub use typed_support::{TypedSupportSet, TypedSupportSurface};
pub use types::*;
pub use zoom::ZoomScale;

macro_rules! export_typed_support_markers {
    ([ $( {
        surface: $surface:ident,
        marker: $marker:ident,
                    $(encodings: [$($encoding_variant:ident => $encoding_tag:literal),+ $(,)?],)?
        surface_doc: $surface_doc:literal,
        marker_doc: $marker_doc:literal,
        diagnostic: $diagnostic:literal,
    }, )* ]) => {
        $(pub use typed_support::$marker;)*
    };
}

typed_support_registry::typed_support_registry!(export_typed_support_markers);
