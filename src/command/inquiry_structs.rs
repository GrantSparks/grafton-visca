//! Built-in inquiry support generated from crate-local metadata.
//!
//! Built-in inquiries are intentionally not implemented through the public
//! `ViscaInquiry` derive.  The table below is the source of truth for generated
//! response discriminants, decoded response data, dispatch, zero-sized inquiry
//! commands, canonical bytes, typed query conversions, and invariant metadata.

use std::borrow::Cow;

use super::exposure::{AntiFlickerMode, ExposureMode};
use super::focus::{AutoFocusSensitivity, FocusMode, FocusRange, FocusZone};
use super::image::{NoiseReduction2DMode, SharpnessMode};
use super::resolution::{NdFilterPosition, PictureEffectMode};
use super::response::{BoolConvention, Nibbles, Nibbles4Or8, Payload, Response};
use super::system::{MotionSyncMode, MotionSyncPreset};
use super::white_balance::{AutoWhiteBalanceSensitivity, WhiteBalanceMode};
use crate::capabilities::{PanTilt, PanTiltWireCodec, Profile};
use crate::command::{encode::WireEncode, ResponseParser};
use crate::error::format_payload_hex;
use crate::types::{BroadcastDomain, DefogLevel, ExposureCompensationPosition, NdFilterPreset};
use crate::{CameraId, Error};

// This is deliberately thread-local so unit tests can prove the profile gate
// runs before generated inquiry serialization without racing parallel tests.
#[cfg(test)]
std::thread_local! {
    static GENERATED_INQUIRY_WRITE_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_generated_inquiry_write_count() {
    GENERATED_INQUIRY_WRITE_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn generated_inquiry_write_count() -> usize {
    GENERATED_INQUIRY_WRITE_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
fn increment_generated_inquiry_write_count() {
    GENERATED_INQUIRY_WRITE_COUNT.with(|count| count.set(count.get().saturating_add(1)));
}

/// Type-checked command reference used by test-only invariant metadata.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuiltinInquiryCommand {
    name: &'static str,
    bytes: &'static [u8],
}

#[cfg(test)]
impl BuiltinInquiryCommand {
    pub(crate) const fn name(self) -> &'static str {
        self.name
    }

    pub(crate) const fn bytes(self) -> &'static [u8] {
        self.bytes
    }
}

#[cfg(test)]
pub(crate) trait BuiltinInquiryCommandMarker {
    const METADATA: BuiltinInquiryCommand;
}

/// How a built-in inquiry participates in command generation.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltinInquiryQuery {
    /// Generates a public zero-sized query command with unique request bytes.
    Queryable,
    /// Decodes responses but intentionally does not generate a query command.
    DecodeOnly,
    /// Generates a query command that is an alias of another command's bytes.
    Alias {
        /// Canonical command for the shared request bytes.
        canonical: BuiltinInquiryCommand,
    },
    /// Generates a query command with shared bytes but a different typed
    /// interpretation of the same wire response.
    AlternateTypedInterpretation {
        /// Canonical command for the shared request bytes.
        canonical: BuiltinInquiryCommand,
    },
}

/// Profile-specific response decoding hook used by a built-in inquiry.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltinInquiryProfileDecoder {
    /// The normal generated decoder is profile independent.
    Default,
    /// Decode pan/tilt position through the camera profile's coordinate system.
    PanTiltPosition,
    /// Enforce the source-backed 3D noise-reduction inquiry range for the
    /// exact built-in profile selected by the session.
    NoiseReduction3D,
}

/// Profile gate required before a camera-facing accessor is implemented.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltinInquiryProfileGate {
    /// The accessor is available for every profile.
    Always,
    /// The accessor requires the named profile support marker.
    Capability {
        /// Profile marker trait required by the accessor impl.
        marker: &'static str,
    },
}

/// Static metadata for generated camera-facing inquiry accessors.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuiltinInquiryAccessorMetadata {
    /// Camera control trait exposing the accessor.
    pub(crate) trait_name: &'static str,
    /// Accessor method name.
    pub(crate) method: &'static str,
    /// Generated query command used by the accessor.
    pub(crate) command: BuiltinInquiryCommand,
    /// Typed response returned by the accessor.
    pub(crate) response_type: &'static str,
    /// Profile support required to expose the accessor.
    pub(crate) profile_gate: BuiltinInquiryProfileGate,
}

/// Static metadata for generated built-in inquiry invariants.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuiltinInquiryMetadata {
    /// Human-readable registry entry name.
    pub(crate) name: &'static str,
    /// Generated command struct name, if this response is queryable.
    pub(crate) command: Option<BuiltinInquiryCommand>,
    /// Response discriminator used by routing and parsing.
    pub(crate) kind: InquiryKind,
    /// Canonical request bytes, including the VISCA terminator.
    pub(crate) bytes: Option<&'static [u8]>,
    /// Command-generation classification.
    pub(crate) query: BuiltinInquiryQuery,
    /// Whether the query has a typed response conversion.
    ///
    /// The generated table intentionally keeps the queryable
    /// `typed: none` entry (`DefogModeInquiry`) explicit. Decode-only entries
    /// are always untyped for this metadata purpose.
    pub(crate) typed: bool,
    /// Whether this inquiry is vendor-specific rather than baseline VISCA.
    pub(crate) vendor_specific: bool,
    /// Profile-specific decode behavior, if this inquiry needs it.
    pub(crate) profile_decoder: BuiltinInquiryProfileDecoder,
    /// Rationale for aliases, alternate interpretations, and intentional gaps.
    pub(crate) rationale: Option<&'static str>,
}

#[cfg(test)]
macro_rules! builtin_inquiry_is_typed {
    (none) => {
        false
    };
    (($($typed:tt)*)) => {
        true
    };
}

#[cfg(test)]
macro_rules! builtin_inquiry_profile_decoder {
    () => {
        BuiltinInquiryProfileDecoder::Default
    };
    (default) => {
        BuiltinInquiryProfileDecoder::Default
    };
    (pan_tilt_position) => {
        BuiltinInquiryProfileDecoder::PanTiltPosition
    };
    (noise_reduction_3d) => {
        BuiltinInquiryProfileDecoder::NoiseReduction3D
    };
}

#[cfg(test)]
macro_rules! builtin_inquiry_profile_gate {
    (none) => {
        BuiltinInquiryProfileGate::Always
    };
    ($profile_gate:path) => {
        BuiltinInquiryProfileGate::Capability {
            marker: stringify!($profile_gate),
        }
    };
}

macro_rules! impl_builtin_response_parser {
    (none, $struct:ident, $kind:ident) => {};
    (($response_ty:ty, $data_pattern:tt => $conversion:expr), $struct:ident, $kind:ident) => {
        impl ResponseParser for $struct {
            type Response = $response_ty;

            fn from_response(resp: Response) -> Result<Self::Response, Error> {
                match resp {
                    Response::Inquiry(InquiryData::$kind $data_pattern) => $conversion,
                    Response::Error(e) => Err(e),
                    _ => Err(Error::UnexpectedResponseType),
                }
            }
        }
    };
}

macro_rules! builtin_profile_request_validation {
    ([], $struct:ident) => {
        fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> crate::Result<()> {
            validate_builtin_inquiry_profile(stringify!($struct), profile)
        }
    };
    ([pan_tilt_position], $struct:ident) => {
        fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<(), crate::Error> {
            if profile.capabilities().has_pan_tilt && profile.pan_tilt_coordinates().is_some() {
                validate_builtin_inquiry_profile(stringify!($struct), profile)
            } else {
                Err(crate::Error::FeatureNotSupported {
                    feature: "pan/tilt position inquiry",
                })
            }
        }
    };
    ([noise_reduction_3d], $struct:ident) => {
        fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> crate::Result<()> {
            validate_builtin_inquiry_profile(stringify!($struct), profile)
        }
    };
}

macro_rules! builtin_profile_decoder_method {
    ([], $response_ty:ty, $struct:ident) => {};
    ([pan_tilt_position], $response_ty:ty, $struct:ident) => {
        fn decoder_for_profile(
            &self,
            profile: &crate::ProfileSpec,
        ) -> crate::ResponseDecoder<Self::Response> {
            fn decode(
                conversion: &Option<crate::PanTiltCoordinateConversion>,
                payload: &[u8],
            ) -> Result<$response_ty, crate::Error> {
                let conversion = conversion.ok_or(crate::Error::FeatureNotSupported {
                    feature: "pan/tilt coordinate conversion",
                })?;
                let response = decode_pan_tilt_position_with_codec(
                    Payload::new(payload),
                    conversion.wire_codec(),
                    conversion.coordinate_system(),
                )?;
                <$struct as ResponseParser>::from_response(response)
            }

            crate::ResponseDecoder::with_context(profile.pan_tilt_coordinates(), decode)
        }
    };
    ([noise_reduction_3d], $response_ty:ty, $struct:ident) => {
        fn decoder_for_profile(
            &self,
            profile: &crate::ProfileSpec,
        ) -> crate::ResponseDecoder<Self::Response> {
            fn decode(legacy_30x: &bool, payload: &[u8]) -> Result<$response_ty, crate::Error> {
                let response =
                    crate::command::parse_inquiry_payload(payload, &InquiryKind::NoiseReduction3D)?;
                let level = <$struct as ResponseParser>::from_response(response)?;
                if *legacy_30x || level.value() <= 5 {
                    Ok(level)
                } else {
                    Err(Error::InvalidResponse {
                        expected: Cow::Borrowed(
                            "3D noise-reduction inquiry level in 0..=5 for this profile",
                        ),
                        actual: vec![level.value()],
                    })
                }
            }

            // `profile_id` is a public inventory claim, not an authority token.
            // The registry seam compares every profile fact, so only the exact
            // source-backed legacy profile receives the wider inquiry domain.
            let legacy_30x = crate::profiles::ProfileId::PtzOptics30X.matches_profile_spec(profile);
            crate::ResponseDecoder::with_context(legacy_30x, decode)
        }
    };
}

fn validate_builtin_inquiry_surface(
    profile: &crate::ProfileSpec,
    surface: crate::capabilities::TypedSupportSurface,
    inquiry: &'static str,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    // These vendor/status surfaces require both the typed admission bit and
    // the underlying source-backed protocol fact. Keeping this narrow match
    // here lets the generated accessor groups remain the single inquiry list.
    let source_backed = match surface {
        crate::capabilities::TypedSupportSurface::FocusZoneInquiry => {
            capabilities.has_focus_zone_inquiry
        }
        crate::capabilities::TypedSupportSurface::UsbAudio => capabilities.has_usb_audio,
        crate::capabilities::TypedSupportSurface::ExposureMode => {
            !capabilities.exposure_modes.is_empty()
        }
        _ => true,
    };

    if source_backed && capabilities.supports_typed(surface) {
        Ok(())
    } else {
        Err(Error::FeatureNotSupported { feature: inquiry })
    }
}

/// Runtime gate for a base-domain inquiry accessor.
///
/// A base-domain inquiry (`power().state()`, `zoom().position()`, ...) has no
/// `where` clause of its own, so on the static facades it inherits its noun
/// accessor's base-domain marker (`HasPower`, `HasZoom`, ...). Those markers are
/// blanket-implemented from the domain data traits
/// (`capabilities::profile_metadata`: `impl<T: Power> HasPower`, ...), which the
/// `Capabilities::has_*` flags mirror at runtime. The erased dynamic surface
/// carries no compile-time bound, so it reproduces that same gate here — keeping
/// `dyn power().state()` refused on exactly the profiles where static `power()`
/// cannot be named (#684).
fn require_builtin_inquiry_domain(supported: bool, inquiry: &'static str) -> Result<(), Error> {
    if supported {
        Ok(())
    } else {
        Err(Error::FeatureNotSupported { feature: inquiry })
    }
}

/// Maps a base-domain accessor marker to the runtime capability flag behind it.
///
/// This is the runtime half of the `noun_marker!` mapping in
/// `crate::command::surface`: each base-domain noun's static accessor is gated on
/// the marker named here, and the erased surface gates the same inquiries on the
/// matching `Capabilities` flag so the two cannot drift.
macro_rules! base_capability {
    (HasPower, $profile:expr) => {
        $profile.capabilities().has_power
    };
    (HasZoom, $profile:expr) => {
        $profile.capabilities().has_zoom
    };
    (HasFocus, $profile:expr) => {
        $profile.capabilities().has_focus
    };
    (HasExposure, $profile:expr) => {
        $profile.capabilities().has_exposure
    };
    (HasWhiteBalance, $profile:expr) => {
        $profile.capabilities().has_white_balance
    };
    (HasImageProcessing, $profile:expr) => {
        $profile.capabilities().has_image_processing
    };
}

/// Generate runtime validation for the profile-gated inquiry accessors.
///
/// The accessor groups are the same closed metadata consumed by the static
/// and dynamic surface audits.  Projecting their marker paths here keeps
/// runtime admission in lockstep with those compile-time gates without a
/// second per-inquiry capability list.
macro_rules! define_builtin_inquiry_profile_validation {
    (accessors { $($groups:tt)* }) => {
        define_builtin_inquiry_profile_validation!(@arms [] profile; $($groups)*);
    };
    (@arms [$($arms:tt)*] $profile:ident;) => {
        fn validate_builtin_inquiry_profile(
            inquiry: &'static str,
            $profile: &crate::ProfileSpec,
        ) -> crate::Result<()> {
            match inquiry {
                // An exposure domain can retain model-specific shutter, gain,
                // or vendor controls without documenting the shared `04 39`
                // AE-mode command and inquiry family. Keep its inquiry on the
                // same source-backed inventory that admits mode changes.
                "ExposureModeInquiry" if $profile.capabilities().exposure_modes.is_empty() => {
                    Err(crate::Error::FeatureNotSupported {
                        feature: "inquiry ExposureModeInquiry",
                    })
                }
                $($arms)*
                _ => Ok(()),
            }
        }
    };
    (@arms [$($arms:tt)*]
        $profile:ident;
        $trait_name:ident {
            gate: none;
            $($command:ident => $method:ident : $response_ty:ty;)*
        }
        $($rest:tt)*
    ) => {
        define_builtin_inquiry_profile_validation!(@arms [$($arms)*] $profile; $($rest)*);
    };
    (@arms [$($arms:tt)*]
        $profile:ident;
        $trait_name:ident {
            gate: crate :: capabilities :: $profile_gate:ident;
            $($command:ident => $method:ident : $response_ty:ty;)*
        }
        $($rest:tt)*
    ) => {
        define_builtin_inquiry_profile_validation!(@arms [
            $($arms)*
            $(
                stringify!($command) => validate_builtin_inquiry_surface(
                    $profile,
                    crate::command::surface::typed_surface_for_marker!($profile_gate),
                    concat!("typed inquiry ", stringify!($command)),
                ),
            )*
        ] $profile; $($rest)*);
    };
    (@arms [$($arms:tt)*]
        $profile:ident;
        $trait_name:ident {
            base_gate: crate :: capabilities :: $base_gate:ident;
            $($command:ident => $method:ident : $response_ty:ty;)*
        }
        $($rest:tt)*
    ) => {
        define_builtin_inquiry_profile_validation!(@arms [
            $($arms)*
            $(
                stringify!($command) => require_builtin_inquiry_domain(
                    base_capability!($base_gate, $profile),
                    concat!("inquiry ", stringify!($command)),
                ),
            )*
        ] $profile; $($rest)*);
    };
}

macro_rules! impl_builtin_typed_request {
    ($profile_decode:tt, none, $struct:ident, $kind:ident, $bytes_const:ident) => {
        impl crate::Request for $struct {
            type Class = crate::request::Inquiry;

            const MAX_SIZE: usize = bytes::$bytes_const.len();
            const TIMEOUT_CLASS: crate::TimeoutClass = crate::TimeoutClass::Inquiry;
            const RETRY_CLASS: crate::RetryClass = crate::RetryClass::Inquiry;
            const CONTROL_CLASS: crate::ControlClass = crate::ControlClass::Normal;

            fn write_into(
                &self,
                camera_id: crate::CameraId,
                buffer: &mut [u8],
            ) -> Result<usize, crate::Error> {
                #[cfg(test)]
                increment_generated_inquiry_write_count();
                WireEncode::write_into(self, camera_id, buffer)
            }

            builtin_profile_request_validation!($profile_decode, $struct);
        }

        impl crate::Inquiry for $struct {
            type Response = crate::command::Response;

            fn route(&self) -> crate::InquiryRoute {
                crate::InquiryRoute::custom(InquiryKind::$kind as u16 + 1)
            }

            fn decoder(&self) -> crate::ResponseDecoder<Self::Response> {
                fn decode(payload: &[u8]) -> Result<crate::command::Response, crate::Error> {
                    crate::command::parse_inquiry_payload(payload, &InquiryKind::$kind)
                }
                crate::ResponseDecoder::from_fn(decode)
            }

            #[doc(hidden)]
            #[allow(private_interfaces)]
            fn builtin_inquiry_syntax_retry(
                &self,
                _authority: crate::requests::BuiltinInquiryAuthority,
            ) -> bool {
                true
            }
        }

        impl crate::prepared::BuiltinInquiryRequest for $struct {}
    };
    ($profile_decode:tt, ($response_ty:ty, $data_pattern:tt => $conversion:expr), $struct:ident, $kind:ident, $bytes_const:ident) => {
        impl crate::Request for $struct {
            type Class = crate::request::Inquiry;

            const MAX_SIZE: usize = bytes::$bytes_const.len();
            const TIMEOUT_CLASS: crate::TimeoutClass = crate::TimeoutClass::Inquiry;
            const RETRY_CLASS: crate::RetryClass = crate::RetryClass::Inquiry;
            const CONTROL_CLASS: crate::ControlClass = crate::ControlClass::Normal;

            fn write_into(
                &self,
                camera_id: crate::CameraId,
                buffer: &mut [u8],
            ) -> Result<usize, crate::Error> {
                #[cfg(test)]
                increment_generated_inquiry_write_count();
                WireEncode::write_into(self, camera_id, buffer)
            }

            builtin_profile_request_validation!($profile_decode, $struct);
        }

        impl crate::Inquiry for $struct {
            type Response = $response_ty;

            fn route(&self) -> crate::InquiryRoute {
                crate::InquiryRoute::custom(InquiryKind::$kind as u16 + 1)
            }

            fn decoder(&self) -> crate::ResponseDecoder<Self::Response> {
                fn decode(payload: &[u8]) -> Result<$response_ty, crate::Error> {
                    let response =
                        crate::command::parse_inquiry_payload(payload, &InquiryKind::$kind)?;
                    <$struct as ResponseParser>::from_response(response)
                }
                crate::ResponseDecoder::from_fn(decode)
            }

            builtin_profile_decoder_method!($profile_decode, $response_ty, $struct);

            #[doc(hidden)]
            #[allow(private_interfaces)]
            fn builtin_inquiry_syntax_retry(
                &self,
                _authority: crate::requests::BuiltinInquiryAuthority,
            ) -> bool {
                true
            }
        }

        impl crate::prepared::BuiltinInquiryRequest for $struct {}
    };
}

macro_rules! define_inquiry_kind_enum {
    (
        queryable { $($query_entries:tt)* }
        decode_only { $($decode_entries:tt)* }
    ) => {
        define_inquiry_kind_enum!(@query [] $($query_entries)* @decode $($decode_entries)*);
    };
    (@query [$($variants:tt)*] @decode $($decode_entries:tt)*) => {
        define_inquiry_kind_enum!(@decode [$($variants)*] $($decode_entries)*);
    };
    (@query [$($variants:tt)*]
        $(#[$meta:meta])*
        $struct:ident => {
            const $bytes_const:ident = [$($byte:expr),+ $(,)?];
            kind: $kind:ident $body:tt;
            decode: |$payload:ident| $decode_body:block;
            $(profile_decode: $profile_decode:ident;)?
            response: false;
            query: $query:expr;
            vendor_specific: $vendor_specific:expr;
            rationale: $rationale:expr;
            typed: $typed:tt;
        }
        $($rest:tt)*
    ) => {
        define_inquiry_kind_enum!(@query [$($variants)*] $($rest)*);
    };
    (@query [$($variants:tt)*]
        $(#[$meta:meta])*
        $struct:ident => {
            const $bytes_const:ident = [$($byte:expr),+ $(,)?];
            kind: $kind:ident $body:tt;
            decode: |$payload:ident| $decode_body:block;
            $(profile_decode: $profile_decode:ident;)?
            response: $response:ident;
            query: $query:expr;
            vendor_specific: $vendor_specific:expr;
            rationale: $rationale:expr;
            typed: $typed:tt;
        }
        $($rest:tt)*
    ) => {
        define_inquiry_kind_enum!(
            @query [
                $($variants)*
                $(#[$meta])*
                $kind,
            ]
            $($rest)*
        );
    };
    (@decode [$($variants:tt)*]) => {
        /// Type of expected response for inquiry commands.
        ///
        /// Used to indicate what kind of data parser should expect in the response
        /// payload.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[allow(missing_docs)]
        pub enum InquiryKind {
            $($variants)*
        }
    };
    (@decode [$($variants:tt)*]
        $decode_kind:ident => {
            data: $decode_body_shape:tt;
            decode: |$decode_payload:ident| $decode_body_block:block;
            vendor_specific: $decode_vendor_specific:expr;
            rationale: $decode_rationale:expr;
        }
        $($rest:tt)*
    ) => {
        define_inquiry_kind_enum!(
            @decode [
                $($variants)*
                $decode_kind,
            ]
            $($rest)*
        );
    };
}

macro_rules! define_inquiry_data_enum {
    (
        queryable { $($query_entries:tt)* }
        decode_only { $($decode_entries:tt)* }
    ) => {
        define_inquiry_data_enum!(@query [] $($query_entries)* @decode $($decode_entries)*);
    };
    (@query [$($variants:tt)*] @decode $($decode_entries:tt)*) => {
        define_inquiry_data_enum!(@decode [$($variants)*] $($decode_entries)*);
    };
    (@query [$($variants:tt)*]
        $(#[$meta:meta])*
        $struct:ident => {
            const $bytes_const:ident = [$($byte:expr),+ $(,)?];
            kind: $kind:ident $body:tt;
            decode: |$payload:ident| $decode_body:block;
            $(profile_decode: $profile_decode:ident;)?
            response: false;
            query: $query:expr;
            vendor_specific: $vendor_specific:expr;
            rationale: $rationale:expr;
            typed: $typed:tt;
        }
        $($rest:tt)*
    ) => {
        define_inquiry_data_enum!(@query [$($variants)*] $($rest)*);
    };
    (@query [$($variants:tt)*]
        $(#[$meta:meta])*
        $struct:ident => {
            const $bytes_const:ident = [$($byte:expr),+ $(,)?];
            kind: $kind:ident $body:tt;
            decode: |$payload:ident| $decode_body:block;
            $(profile_decode: $profile_decode:ident;)?
            response: $response:ident;
            query: $query:expr;
            vendor_specific: $vendor_specific:expr;
            rationale: $rationale:expr;
            typed: $typed:tt;
        }
        $($rest:tt)*
    ) => {
        define_inquiry_data_enum!(
            @query [
                $($variants)*
                $(#[$meta])*
                $kind $body,
            ]
            $($rest)*
        );
    };
    (@decode [$($variants:tt)*]) => {
        /// Response data from VISCA inquiry commands.
        ///
        /// Each variant represents a different type of inquiry response with its
        /// associated data.  These are returned wrapped in
        /// [`Response::Inquiry(...)`](Response::Inquiry).
        #[derive(Debug, Copy, Clone)]
        #[allow(missing_docs)]
        pub enum InquiryData {
            $($variants)*
        }
    };
    (@decode [$($variants:tt)*]
        $decode_kind:ident => {
            data: $decode_body_shape:tt;
            decode: |$decode_payload:ident| $decode_body_block:block;
            vendor_specific: $decode_vendor_specific:expr;
            rationale: $decode_rationale:expr;
        }
        $($rest:tt)*
    ) => {
        define_inquiry_data_enum!(
            @decode [
                $($variants)*
                $decode_kind $decode_body_shape,
            ]
            $($rest)*
        );
    };
}

macro_rules! define_inquiry_dispatch {
    (
        queryable { $($query_entries:tt)* }
        decode_only { $($decode_entries:tt)* }
    ) => {
        define_inquiry_dispatch!(@query payload [] $($query_entries)* @decode $($decode_entries)*);
    };
    (@query $dispatch_payload:ident [$($arms:tt)*] @decode $($decode_entries:tt)*) => {
        define_inquiry_dispatch!(@decode $dispatch_payload [$($arms)*] $($decode_entries)*);
    };
    (@query $dispatch_payload:ident [$($arms:tt)*]
        $(#[$meta:meta])*
        $struct:ident => {
            const $bytes_const:ident = [$($byte:expr),+ $(,)?];
            kind: $kind:ident $body:tt;
            decode: |$payload:ident| $decode_body:block;
            $(profile_decode: $profile_decode:ident;)?
            response: false;
            query: $query:expr;
            vendor_specific: $vendor_specific:expr;
            rationale: $rationale:expr;
            typed: $typed:tt;
        }
        $($rest:tt)*
    ) => {
        define_inquiry_dispatch!(@query $dispatch_payload [$($arms)*] $($rest)*);
    };
    (@query $dispatch_payload:ident [$($arms:tt)*]
        $(#[$meta:meta])*
        $struct:ident => {
            const $bytes_const:ident = [$($byte:expr),+ $(,)?];
            kind: $kind:ident $body:tt;
            decode: |$payload:ident| $decode_body:block;
            $(profile_decode: $profile_decode:ident;)?
            response: $response:ident;
            query: $query:expr;
            vendor_specific: $vendor_specific:expr;
            rationale: $rationale:expr;
            typed: $typed:tt;
        }
        $($rest:tt)*
    ) => {
        define_inquiry_dispatch!(
            @query $dispatch_payload [
                $($arms)*
                InquiryKind::$kind => {
                    let $payload = $dispatch_payload;
                    $decode_body
                },
            ]
            $($rest)*
        );
    };
    (@decode $dispatch_payload:ident [$($arms:tt)*]) => {
        /// Decode an inquiry response by dispatching to the appropriate decoder.
        ///
        /// Generated from the built-in inquiry table to ensure every
        /// [`InquiryKind`] variant has a corresponding decoder arm.
        pub(crate) fn dispatch(
            kind: InquiryKind,
            $dispatch_payload: Payload<'_>,
        ) -> Result<Response, Error> {
            match kind {
                $($arms)*
            }
        }
    };
    (@decode $dispatch_payload:ident [$($arms:tt)*]
        $decode_kind:ident => {
            data: $decode_body_shape:tt;
            decode: |$decode_payload:ident| $decode_body_block:block;
            vendor_specific: $decode_vendor_specific:expr;
            rationale: $decode_rationale:expr;
        }
        $($rest:tt)*
    ) => {
        define_inquiry_dispatch!(
            @decode $dispatch_payload [
                $($arms)*
                InquiryKind::$decode_kind => {
                    let $decode_payload = $dispatch_payload;
                    $decode_body_block
                },
            ]
            $($rest)*
        );
    };
}

macro_rules! define_inquiry_profile_dispatch {
    (
        queryable { $($query_entries:tt)* }
        decode_only { $($decode_entries:tt)* }
    ) => {
        define_inquiry_profile_dispatch!(@query payload [] $($query_entries)*);
    };
    (@query $dispatch_payload:ident [$($arms:tt)*]) => {
        /// Decode an inquiry response with profile-specific hooks described by
        /// the built-in inquiry table.
        pub(crate) fn dispatch_for<P: Profile + PanTilt>(
            kind: InquiryKind,
            $dispatch_payload: Payload<'_>,
        ) -> Result<Response, Error> {
            match kind {
                $($arms)*
                _ => dispatch(kind, $dispatch_payload),
            }
        }
    };
    (@query $dispatch_payload:ident [$($arms:tt)*]
        $(#[$meta:meta])*
        $struct:ident => {
            const $bytes_const:ident = [$($byte:expr),+ $(,)?];
            kind: $kind:ident $body:tt;
            decode: |$payload:ident| $decode_body:block;
            profile_decode: pan_tilt_position;
            response: $response:ident;
            query: $query:expr;
            vendor_specific: $vendor_specific:expr;
            rationale: $rationale:expr;
            typed: $typed:tt;
        }
        $($rest:tt)*
    ) => {
        define_inquiry_profile_dispatch!(
            @query $dispatch_payload [
                $($arms)*
                InquiryKind::$kind => {
                    let $payload = $dispatch_payload;
                    decode_pan_tilt_position_for::<P>($payload)
                },
            ]
            $($rest)*
        );
    };
    (@query $dispatch_payload:ident [$($arms:tt)*]
        $(#[$meta:meta])*
        $struct:ident => {
            const $bytes_const:ident = [$($byte:expr),+ $(,)?];
            kind: $kind:ident $body:tt;
            decode: |$payload:ident| $decode_body:block;
            profile_decode: default;
            response: $response:ident;
            query: $query:expr;
            vendor_specific: $vendor_specific:expr;
            rationale: $rationale:expr;
            typed: $typed:tt;
        }
        $($rest:tt)*
    ) => {
        define_inquiry_profile_dispatch!(@query $dispatch_payload [$($arms)*] $($rest)*);
    };
    (@query $dispatch_payload:ident [$($arms:tt)*]
        $(#[$meta:meta])*
        $struct:ident => {
            const $bytes_const:ident = [$($byte:expr),+ $(,)?];
            kind: $kind:ident $body:tt;
            decode: |$payload:ident| $decode_body:block;
            profile_decode: noise_reduction_3d;
            response: $response:ident;
            query: $query:expr;
            vendor_specific: $vendor_specific:expr;
            rationale: $rationale:expr;
            typed: $typed:tt;
        }
        $($rest:tt)*
    ) => {
        // `dispatch_for` returns structural `Response` data. The session-owned
        // typed decoder applies the selected profile's numeric reply domain.
        define_inquiry_profile_dispatch!(@query $dispatch_payload [$($arms)*] $($rest)*);
    };
    (@query $dispatch_payload:ident [$($arms:tt)*]
        $(#[$meta:meta])*
        $struct:ident => {
            const $bytes_const:ident = [$($byte:expr),+ $(,)?];
            kind: $kind:ident $body:tt;
            decode: |$payload:ident| $decode_body:block;
            response: $response:ident;
            query: $query:expr;
            vendor_specific: $vendor_specific:expr;
            rationale: $rationale:expr;
            typed: $typed:tt;
        }
        $($rest:tt)*
    ) => {
        define_inquiry_profile_dispatch!(@query $dispatch_payload [$($arms)*] $($rest)*);
    };
}

macro_rules! define_builtin_inquiries {
    (@accessor_array $($accessor_groups:tt)*) => {
        define_builtin_inquiries!(@accessor_array_acc [] $($accessor_groups)*)
    };
    (@accessor_array_acc [$($items:tt)*]) => {
        &[
            $($items)*
        ]
    };
    (@accessor_array_acc [$($items:tt)*]
        $trait_name:ident {
            gate: none;
            $(
                $command:ident => $method:ident : $response_ty:ty;
            )*
        }
        $($rest:tt)*
    ) => {
        define_builtin_inquiries!(@accessor_array_acc [
            $($items)*
            $(
                BuiltinInquiryAccessorMetadata {
                    trait_name: stringify!($trait_name),
                    method: stringify!($method),
                    command: <$command as BuiltinInquiryCommandMarker>::METADATA,
                    response_type: stringify!($response_ty),
                    profile_gate: builtin_inquiry_profile_gate!(none),
                },
            )*
        ] $($rest)*)
    };
    (@accessor_array_acc [$($items:tt)*]
        $trait_name:ident {
            gate: $profile_gate:path;
            $(
                $command:ident => $method:ident : $response_ty:ty;
            )*
        }
        $($rest:tt)*
    ) => {
        define_builtin_inquiries!(@accessor_array_acc [
            $($items)*
            $(
                BuiltinInquiryAccessorMetadata {
                    trait_name: stringify!($trait_name),
                    method: stringify!($method),
                    command: <$command as BuiltinInquiryCommandMarker>::METADATA,
                    response_type: stringify!($response_ty),
                    profile_gate: builtin_inquiry_profile_gate!($profile_gate),
                },
            )*
        ] $($rest)*)
    };
    (@accessor_array_acc [$($items:tt)*]
        $trait_name:ident {
            base_gate: $base_gate:path;
            $(
                $command:ident => $method:ident : $response_ty:ty;
            )*
        }
        $($rest:tt)*
    ) => {
        define_builtin_inquiries!(@accessor_array_acc [
            $($items)*
            $(
                BuiltinInquiryAccessorMetadata {
                    trait_name: stringify!($trait_name),
                    method: stringify!($method),
                    command: <$command as BuiltinInquiryCommandMarker>::METADATA,
                    response_type: stringify!($response_ty),
                    profile_gate: builtin_inquiry_profile_gate!($base_gate),
                },
            )*
        ] $($rest)*)
    };
    (
        queryable {
            $(
                $(#[$meta:meta])*
                $struct:ident => {
                    const $bytes_const:ident = [$($byte:expr),+ $(,)?];
                    kind: $kind:ident $body:tt;
                    decode: |$payload:ident| $decode_body:block;
                    $(profile_decode: $profile_decode:ident;)?
                    response: $response:ident;
                    query: $query:expr;
                    vendor_specific: $vendor_specific:expr;
                    rationale: $rationale:expr;
                    typed: $typed:tt;
                }
            )*
        }
        decode_only {
            $(
                $decode_kind:ident => {
                    data: $decode_body_shape:tt;
                    decode: |$decode_payload:ident| $decode_body_block:block;
                    vendor_specific: $decode_vendor_specific:expr;
                    rationale: $decode_rationale:expr;
                }
            )*
        }
        accessors { $($accessor_groups:tt)* }
    ) => {
        define_inquiry_kind_enum! {
            queryable {
                $(
                    $(#[$meta])*
                    $struct => {
                        const $bytes_const = [$($byte),+];
                        kind: $kind $body;
                        decode: |$payload| $decode_body;
                        $(profile_decode: $profile_decode;)?
                        response: $response;
                        query: $query;
                        vendor_specific: $vendor_specific;
                        rationale: $rationale;
                        typed: $typed;
                    }
                )*
            }
            decode_only {
                $(
                    $decode_kind => {
                        data: $decode_body_shape;
                        decode: |$decode_payload| $decode_body_block;
                        vendor_specific: $decode_vendor_specific;
                        rationale: $decode_rationale;
                    }
                )*
            }
        }

        define_inquiry_data_enum! {
            queryable {
                $(
                    $(#[$meta])*
                    $struct => {
                        const $bytes_const = [$($byte),+];
                        kind: $kind $body;
                        decode: |$payload| $decode_body;
                        $(profile_decode: $profile_decode;)?
                        response: $response;
                        query: $query;
                        vendor_specific: $vendor_specific;
                        rationale: $rationale;
                        typed: $typed;
                    }
                )*
            }
            decode_only {
                $(
                    $decode_kind => {
                        data: $decode_body_shape;
                        decode: |$decode_payload| $decode_body_block;
                        vendor_specific: $decode_vendor_specific;
                        rationale: $decode_rationale;
                    }
                )*
            }
        }

        define_inquiry_dispatch! {
            queryable {
                $(
                    $(#[$meta])*
                    $struct => {
                        const $bytes_const = [$($byte),+];
                        kind: $kind $body;
                        decode: |$payload| $decode_body;
                        $(profile_decode: $profile_decode;)?
                        response: $response;
                        query: $query;
                        vendor_specific: $vendor_specific;
                        rationale: $rationale;
                        typed: $typed;
                    }
                )*
            }
            decode_only {
                $(
                    $decode_kind => {
                        data: $decode_body_shape;
                        decode: |$decode_payload| $decode_body_block;
                        vendor_specific: $decode_vendor_specific;
                        rationale: $decode_rationale;
                    }
                )*
            }
        }

        define_inquiry_profile_dispatch! {
            queryable {
                $(
                    $(#[$meta])*
                    $struct => {
                        const $bytes_const = [$($byte),+];
                        kind: $kind $body;
                        decode: |$payload| $decode_body;
                        $(profile_decode: $profile_decode;)?
                        response: $response;
                        query: $query;
                        vendor_specific: $vendor_specific;
                        rationale: $rationale;
                        typed: $typed;
                    }
                )*
            }
            decode_only {
                $(
                    $decode_kind => {
                        data: $decode_body_shape;
                        decode: |$decode_payload| $decode_body_block;
                        vendor_specific: $decode_vendor_specific;
                        rationale: $decode_rationale;
                    }
                )*
            }
        }

        define_builtin_inquiry_profile_validation! {
            accessors { $($accessor_groups)* }
        }

        /// Canonical request bytes for generated built-in inquiry commands.
        pub mod bytes {
            $(
                $(#[$meta])*
                pub const $bytes_const: &[u8] = &[
                    $($byte,)+
                    crate::command::bytes::VISCA_TERMINATOR,
                ];
            )*
        }

        $(
            $(#[$meta])*
            #[derive(Debug, Copy, Clone, Default)]
            pub struct $struct;

            impl WireEncode for $struct {
                fn write_into(
                    &self,
                    camera_id: CameraId,
                    buffer: &mut [u8],
                ) -> Result<usize, Error> {
                    let bytes = bytes::$bytes_const;
                    let len = bytes.len();
                    if buffer.len() < len {
                        return Err(Error::BufferTooSmall {
                            required: len,
                            actual: buffer.len(),
                        });
                    }
                    buffer[..len].copy_from_slice(bytes);
                    buffer[0] = camera_id.to_address_byte();
                    Ok(len)
                }

            }

            impl_builtin_response_parser!($typed, $struct, $kind);
            impl_builtin_typed_request!([$($profile_decode)?], $typed, $struct, $kind, $bytes_const);

            #[cfg(test)]
            impl BuiltinInquiryCommandMarker for $struct {
                const METADATA: BuiltinInquiryCommand = BuiltinInquiryCommand {
                    name: stringify!($struct),
                    bytes: bytes::$bytes_const,
                };
            }
        )*

        /// Iterable built-in inquiry metadata for invariant tests and internal
        /// consistency checks.
        #[cfg(test)]
        pub(crate) const BUILTIN_INQUIRIES: &[BuiltinInquiryMetadata] = &[
            $(
                BuiltinInquiryMetadata {
                    name: stringify!($struct),
                    command: Some(<$struct as BuiltinInquiryCommandMarker>::METADATA),
                    kind: InquiryKind::$kind,
                    bytes: Some(bytes::$bytes_const),
                    query: $query,
                    typed: builtin_inquiry_is_typed!($typed),
                    vendor_specific: $vendor_specific,
                    profile_decoder: builtin_inquiry_profile_decoder!($($profile_decode)?),
                    rationale: $rationale,
                },
            )*
            $(
                BuiltinInquiryMetadata {
                    name: stringify!($decode_kind),
                    command: None,
                    kind: InquiryKind::$decode_kind,
                    bytes: None,
                    query: BuiltinInquiryQuery::DecodeOnly,
                    typed: false,
                    vendor_specific: $decode_vendor_specific,
                    profile_decoder: BuiltinInquiryProfileDecoder::Default,
                    rationale: $decode_rationale,
                },
            )*
        ];

        /// Iterable camera-facing inquiry accessor metadata generated from the
        /// same table as the built-in commands.
        #[cfg(test)]
        pub(crate) const BUILTIN_INQUIRY_ACCESSORS: &[BuiltinInquiryAccessorMetadata] =
            define_builtin_inquiries!(@accessor_array $($accessor_groups)*);

        #[cfg(test)]
        mod generated_invariant_tests {
            use super::*;

            fn assert_inquiry_matches_metadata<C>(
                cmd: C,
                expected: &[u8],
                camera_id: CameraId,
                name: &str,
            ) where
                C: WireEncode,
            {
                let mut buffer = [0u8; 32];
                let len = cmd
                    .write_into(camera_id, &mut buffer)
                    .expect("inquiry should encode");

                let mut expected_for_camera = [0u8; 32];
                expected_for_camera[..expected.len()].copy_from_slice(expected);
                expected_for_camera[0] = camera_id.to_address_byte();

                assert_eq!(len, expected.len(), "{name} must report exact length");
                assert_eq!(
                    &buffer[..len],
                    &expected_for_camera[..expected.len()],
                    "{name} must match canonical inquiry bytes for camera {camera_id}",
                );
                assert_eq!(
                    buffer[..len].iter().filter(|&&b| b == crate::command::VISCA_TERMINATOR).count(),
                    1,
                    "{name} must contain exactly one terminator",
                );
            }

            #[test]
            fn generated_queryable_inquiries_match_metadata_for_all_camera_ids() {
                for camera_num in 1..=8 {
                    let camera_id = CameraId::new(camera_num).expect("valid camera id");
                    $(
                        assert_inquiry_matches_metadata(
                            $struct,
                            bytes::$bytes_const,
                            camera_id,
                            stringify!($struct),
                        );
                    )*
                }
            }

            #[test]
            fn decode_only_entries_do_not_have_command_bytes() {
                for meta in BUILTIN_INQUIRIES {
                    if matches!(meta.query, BuiltinInquiryQuery::DecodeOnly) {
                        assert!(meta.command.is_none(), "{} must not expose a command", meta.name);
                        assert!(meta.bytes.is_none(), "{} must not expose request bytes", meta.name);
                    }
                }
            }

            #[test]
            fn duplicate_request_bytes_are_explicit() {
                for (i, left) in BUILTIN_INQUIRIES.iter().enumerate() {
                    let Some(left_bytes) = left.bytes else {
                        continue;
                    };
                    for right in BUILTIN_INQUIRIES.iter().skip(i + 1) {
                        let Some(right_bytes) = right.bytes else {
                            continue;
                        };
                        if left_bytes == right_bytes {
                            let left_explicit = matches!(
                                left.query,
                                BuiltinInquiryQuery::Alias { .. }
                                    | BuiltinInquiryQuery::AlternateTypedInterpretation { .. }
                            );
                            let right_explicit = matches!(
                                right.query,
                                BuiltinInquiryQuery::Alias { .. }
                                    | BuiltinInquiryQuery::AlternateTypedInterpretation { .. }
                            );
                            assert!(
                                left_explicit || right_explicit,
                                "duplicate inquiry bytes for {} and {} must be classified",
                                left.name,
                                right.name,
                            );
                            assert!(
                                left.rationale.is_some() || right.rationale.is_some(),
                                "duplicate inquiry bytes for {} and {} need a rationale",
                                left.name,
                                right.name,
                            );
                        }
                    }
                }
            }

            #[test]
            fn aliases_and_alternate_interpretations_have_rationale() {
                for meta in BUILTIN_INQUIRIES {
                    match meta.query {
                        BuiltinInquiryQuery::Alias { canonical }
                        | BuiltinInquiryQuery::AlternateTypedInterpretation { canonical } => {
                            assert!(meta.rationale.is_some(), "{} needs a rationale", meta.name);
                            assert_eq!(
                                meta.bytes,
                                Some(canonical.bytes()),
                                "{} must share request bytes with canonical {}",
                                meta.name,
                                canonical.name(),
                            );
                        }
                        BuiltinInquiryQuery::Queryable | BuiltinInquiryQuery::DecodeOnly => {}
                    }
                }
            }

            #[test]
            fn generated_metadata_is_internally_complete() {
                let mut saw_vendor_specific = false;
                let mut saw_profile_decoder = false;

                for meta in BUILTIN_INQUIRIES {
                    assert!(!meta.name.is_empty(), "metadata entry must have a name");

                    if let Some(command) = meta.command {
                        assert_eq!(
                            meta.name,
                            command.name(),
                            "{} command discriminator must match metadata name",
                            meta.name,
                        );
                        assert_eq!(
                            meta.bytes,
                            Some(command.bytes()),
                            "{} command discriminator must expose canonical bytes",
                            meta.name,
                        );
                    }

                    if meta.vendor_specific {
                        saw_vendor_specific = true;
                    }

                    match meta.profile_decoder {
                        BuiltinInquiryProfileDecoder::Default => {}
                        BuiltinInquiryProfileDecoder::PanTiltPosition => {
                            saw_profile_decoder = true;
                            assert_eq!(meta.kind, InquiryKind::PanTiltPosition);
                        }
                        BuiltinInquiryProfileDecoder::NoiseReduction3D => {
                            saw_profile_decoder = true;
                            assert_eq!(meta.kind, InquiryKind::NoiseReduction3D);
                        }
                    }
                }

                assert!(saw_vendor_specific, "vendor-specific inquiries must be modeled");
                assert!(
                    saw_profile_decoder,
                    "profile-aware inquiry decoding must be modeled"
                );
            }

            #[test]
            fn queryable_typed_exclusions_are_exact() {
                let mut untyped = BUILTIN_INQUIRIES
                    .iter()
                    .filter(|meta| {
                        !matches!(meta.query, BuiltinInquiryQuery::DecodeOnly) && !meta.typed
                    })
                    .map(|meta| meta.name)
                    .collect::<Vec<_>>();
                untyped.sort_unstable();

                assert_eq!(untyped, ["DefogModeInquiry"]);
            }

            #[test]
            fn required_queryable_typed_inquiries_have_generated_accessors() {
                let required = [
                    ("FocusRangeInquiry", "focus_range"),
                    (
                        "AutoWhiteBalanceSensitivityInquiry",
                        "auto_white_balance_sensitivity",
                    ),
                    ("TallyRedInquiry", "red_tally_status"),
                    ("MotionSyncPresetInquiry", "motion_sync_speed"),
                    ("DynamicRangeInquiry", "dynamic_range"),
                    ("FlickerModeInquiry", "flicker_mode"),
                    ("AutoFocusSensitivityInquiry", "auto_focus_sensitivity"),
                ];

                for (command, method) in required {
                    assert!(
                        BUILTIN_INQUIRY_ACCESSORS.iter().any(|accessor| {
                            accessor.command.name() == command && accessor.method == method
                        }),
                        "{command} must have generated typed accessor {method}",
                    );
                }
            }

            #[test]
            fn nd_filter_inquiry_has_one_generated_accessor_path() {
                let rows = BUILTIN_INQUIRY_ACCESSORS
                    .iter()
                    .filter(|accessor| accessor.command.name() == "NdFilterInquiry")
                    .collect::<Vec<_>>();

                assert_eq!(rows.len(), 1, "NdFilterInquiry must have one accessor row");
                assert_eq!(rows[0].trait_name, "NdFilterInquiryControl");
                assert_eq!(rows[0].method, "nd_filter_position");
                assert!(
                    !BUILTIN_INQUIRY_ACCESSORS.iter().any(|accessor| {
                        accessor.trait_name == "NdFilterControl"
                            && accessor.command.name() == "NdFilterInquiry"
                    }),
                    "the legacy NdFilterControl inquiry path must not be generated",
                );
            }

            #[test]
            fn camera_accessor_metadata_is_internally_complete() {
                assert!(
                    !BUILTIN_INQUIRY_ACCESSORS.is_empty(),
                    "camera-facing inquiry accessors must be table-backed",
                );

                for accessor in BUILTIN_INQUIRY_ACCESSORS {
                    assert!(
                        !accessor.trait_name.is_empty(),
                        "{} must name an accessor trait",
                        accessor.method,
                    );
                    assert!(
                        !accessor.method.is_empty(),
                        "{} must name an accessor method",
                        accessor.trait_name,
                    );
                    assert!(
                        !accessor.response_type.is_empty(),
                        "{}::{} must name a response type",
                        accessor.trait_name,
                        accessor.method,
                    );
                    assert_eq!(
                        accessor.command.bytes().last(),
                        Some(&crate::command::VISCA_TERMINATOR),
                        "{}::{} must reference a generated command with canonical bytes",
                        accessor.trait_name,
                        accessor.method,
                    );

                    match accessor.profile_gate {
                        BuiltinInquiryProfileGate::Always => {}
                        BuiltinInquiryProfileGate::Capability { marker } => {
                            let normalized_marker = marker.replace(' ', "");
                            assert!(
                                normalized_marker.starts_with("crate::capabilities::Has"),
                                "{}::{} has an unexpected profile gate marker: {marker}",
                                accessor.trait_name,
                                accessor.method,
                            );
                        }
                    }
                }
            }

            #[test]
            fn representative_short_buffer_errors_are_exact() {
                let mut buffer = [0u8; 32];
                let actual = bytes::POWER.len() - 1;
                let result = PowerInquiry.write_into(CameraId::CAMERA_1, &mut buffer[..actual]);
                assert!(
                    matches!(
                        result,
                        Err(Error::BufferTooSmall {
                            required,
                            actual: reported_actual,
                        }) if required == bytes::POWER.len() && reported_actual == actual
                    ),
                    "PowerInquiry buffer-too-small details changed: {result:?}",
                );

                let actual = bytes::TALLY_RED.len() - 1;
                let result = TallyRedInquiry.write_into(CameraId::CAMERA_1, &mut buffer[..actual]);
                assert!(
                    matches!(
                        result,
                        Err(Error::BufferTooSmall {
                            required,
                            actual: reported_actual,
                        }) if required == bytes::TALLY_RED.len() && reported_actual == actual
                    ),
                    "TallyRedInquiry buffer-too-small details changed: {result:?}",
                );
            }
        }
    };
}

macro_rules! builtin_inquiry_table {
    ($callback:ident) => {
        $callback! {
    queryable {
        /// Inquiry command to get the current power state of the camera.
        PowerInquiry => {
            const POWER = [0x81, 0x09, 0x04, 0x00];
            kind: Power {
                /// Whether the camera is powered on.
                on: bool,
            };
            decode: |payload| {
                let on = payload.parse_bool("power_status", BoolConvention::OnIs02)?;
                Ok(Response::Inquiry(InquiryData::Power { on }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (bool, { on } => Ok(on));
        }

        /// Inquiry command to get the camera version information.
        VersionInquiry => {
            const VERSION = [0x81, 0x09, 0x00, 0x02];
            kind: Version {
                /// Vendor ID.
                vendor: u16,
                /// Model ID.
                model: u16,
                /// ROM version.
                rom_version: u32,
                /// Maximum socket number.
                max_socket: u8,
            };
            decode: |payload| {
                if payload.len() != 7 {
                    return Err(Error::invalid_response_length(7, payload.as_slice()));
                }
                let vendor = ((payload.as_slice()[0] as u16) << 8) | (payload.as_slice()[1] as u16);
                let model = ((payload.as_slice()[2] as u16) << 8) | (payload.as_slice()[3] as u16);
                let rom_version = ((payload.as_slice()[4] as u32) << 8) | (payload.as_slice()[5] as u32);
                let max_socket = payload.as_slice()[6];
                Ok(Response::Inquiry(InquiryData::Version { vendor, model, rom_version, max_socket }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (
                crate::command::VersionInfo,
                { vendor, model, rom_version, max_socket } => Ok(crate::command::VersionInfo {
                    vendor,
                    model,
                    rom_version,
                    max_socket,
                })
            );
        }

        /// Inquiry command to get the current pan/tilt position.
        PanTiltPositionInquiry => {
            const PAN_TILT_POSITION = [0x81, 0x09, 0x06, 0x12];
            kind: PanTiltPosition {
                /// Current pan position.
                pan: i32,
                /// Current tilt position.
                tilt: i32,
            };
            decode: |payload| {
                decode_pan_tilt_position(payload)
            };
            profile_decode: pan_tilt_position;
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (
                crate::camera::PanTiltPosition,
                { pan, tilt } => Ok(crate::camera::PanTiltPosition { pan, tilt })
            );
        }

        /// Inquiry command to get the current zoom position.
        ZoomPositionInquiry => {
            const ZOOM_POSITION = [0x81, 0x09, 0x04, 0x47];
            kind: ZoomPosition {
                /// Zoom position value.
                position: u16,
            };
            decode: |payload| {
                let nibbles = Nibbles4Or8::try_from(payload)?;
                if matches!(nibbles, Nibbles4Or8::N8(_)) {
                    tracing::warn!(
                        "ZoomPosition: Received extended format (8 nibbles). Using first 4 nibbles (16-bit) per VISCA spec."
                    );
                }
                let position = nibbles.first_u16();
                Ok(Response::Inquiry(InquiryData::ZoomPosition { position }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (
                crate::types::ZoomPosition,
                { position } => crate::types::ZoomPosition::new(position)
            );
        }

        /// Inquiry command to get the current focus position.
        FocusPositionInquiry => {
            const FOCUS_POSITION = [0x81, 0x09, 0x04, 0x48];
            kind: FocusPosition {
                /// Focus position value.
                position: u16,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                let position = nibbles.u16_quad(0);
                Ok(Response::Inquiry(InquiryData::FocusPosition { position }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (
                crate::types::FocusPosition,
                { position } => Ok(crate::types::FocusPosition::new(position))
            );
        }

        /// Inquiry command to get the current exposure mode setting.
        ExposureModeInquiry => {
            const EXPOSURE_MODE = [0x81, 0x09, 0x04, 0x39];
            kind: ExposureMode {
                /// Current exposure mode (Auto, Manual, Shutter, Iris, or Bright).
                mode: ExposureMode,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                let mode = match payload.as_slice()[0] {
                    0x00 => ExposureMode::Auto,
                    0x03 => ExposureMode::Manual,
                    0x0A => ExposureMode::Shutter,
                    0x0B => ExposureMode::Iris,
                    0x0D => ExposureMode::Bright,
                    v => {
                        return Err(Error::InvalidParameter {
                            parameter: "exposure_mode",
                            value: Cow::Owned(format!("{v:02X}")),
                            reason: Cow::Borrowed("Unknown exposure mode value"),
                        })
                    }
                };
                Ok(Response::Inquiry(InquiryData::ExposureMode { mode }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (ExposureMode, { mode } => Ok(mode));
        }

        /// Inquiry command to get the current exposure compensation value.
        ExposureCompensationInquiry => {
            const EXPOSURE_COMPENSATION = [0x81, 0x09, 0x04, 0x4E];
            kind: ExposureCompensation {
                /// Exposure compensation value (-7 to +7).
                value: i8,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                let raw_value = nibbles.u8_pair(2);
                #[allow(clippy::cast_possible_wrap)]
                let value = raw_value as i8 - 7;
                Ok(Response::Inquiry(InquiryData::ExposureCompensation { value }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: Some("Shares CAM_ExpCompPosInq bytes with ExposureCompensationPosition; this entry exposes the legacy signed EV level interpretation.");
            typed: (
                crate::types::ExposureCompensationLevel,
                { value } => crate::types::ExposureCompensationLevel::new(value)
            );
        }

        /// Inquiry command to get the exposure compensation mode on/off status.
        ExposureCompensationModeInquiry => {
            const EXPOSURE_COMPENSATION_MODE = [0x81, 0x09, 0x04, 0x3E];
            kind: ExposureCompensationMode {
                /// Whether exposure compensation is enabled.
                on: bool,
            };
            decode: |payload| {
                let on = payload.parse_bool("exposure_compensation_mode", BoolConvention::OnIs02)?;
                Ok(Response::Inquiry(InquiryData::ExposureCompensationMode { on }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (bool, { on } => Ok(on));
        }

        /// Inquiry command to get the current iris position value.
        IrisInquiry => {
            const IRIS = [0x81, 0x09, 0x04, 0x4B];
            kind: Iris {
                /// Iris position (0x0=Close to 0xC=F1.8).
                position: u8,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                Ok(Response::Inquiry(InquiryData::Iris { position: nibbles.u8_pair(2) }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (
                crate::types::IrisLevel,
                { position } => crate::types::IrisLevel::new(position)
            );
        }

        /// Inquiry command to get the current shutter speed setting.
        ShutterInquiry => {
            const SHUTTER = [0x81, 0x09, 0x04, 0x4A];
            kind: Shutter {
                /// Shutter position (0x01=1/30 to 0x11=1/10000).
                position: u16,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                let position = nibbles.u8_pair(2) as u16;
                Ok(Response::Inquiry(InquiryData::Shutter { position }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (
                crate::types::ShutterSpeed,
                { position } => crate::types::ShutterSpeed::new(position)
            );
        }

        /// Inquiry command to get the current brightness adjustment value.
        BrightnessInquiry => {
            const BRIGHT = [0x81, 0x09, 0x04, 0x4D];
            kind: Brightness {
                /// Brightness position (0x00=0 to 0x11=17).
                position: u16,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                let position = nibbles.u16_quad(0);
                Ok(Response::Inquiry(InquiryData::Brightness { position }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (
                crate::types::BrightnessLevel,
                { position } => crate::types::BrightnessLevel::new(position)
            );
        }

        /// Inquiry command to get the current white balance mode.
        WhiteBalanceModeInquiry => {
            const WHITE_BALANCE_MODE = [0x81, 0x09, 0x04, 0x35];
            kind: WhiteBalanceMode {
                /// Current white balance mode.
                mode: WhiteBalanceMode,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                let mode = match payload.as_slice()[0] {
                    0x00 => WhiteBalanceMode::Auto,
                    0x01 => WhiteBalanceMode::Indoor,
                    0x02 => WhiteBalanceMode::Outdoor,
                    0x03 => WhiteBalanceMode::OnePush,
                    0x04 => WhiteBalanceMode::ATW,
                    0x05 => WhiteBalanceMode::Manual,
                    0x20 => WhiteBalanceMode::ColorTemperature,
                    v => {
                        return Err(Error::InvalidParameter {
                            parameter: "white_balance_mode",
                            value: Cow::Owned(format!("{v:02X}")),
                            reason: Cow::Borrowed("Unknown white balance mode value"),
                        })
                    }
                };
                Ok(Response::Inquiry(InquiryData::WhiteBalanceMode { mode }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (WhiteBalanceMode, { mode } => Ok(mode));
        }

        /// Inquiry command to get the current color temperature value.
        ColorTemperatureInquiry => {
            const COLOR_TEMPERATURE = [0x81, 0x09, 0x04, 0x20];
            kind: ColorTemperature {
                /// Color temperature in Kelvin.
                temperature: u16,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                let temperature = payload.as_slice()[0] as u16;
                Ok(Response::Inquiry(InquiryData::ColorTemperature { temperature }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (
                crate::types::ColorTemp,
                { temperature } => crate::types::ColorTemp::new(temperature)
            );
        }

        /// Inquiry command to get the current red gain value.
        RedGainInquiry => {
            const RED_GAIN = [0x81, 0x09, 0x04, 0x43];
            kind: RedChannel {
                /// Red channel absolute gain value.
                gain: u8,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                let gain = nibbles.u8_pair(2);
                Ok(Response::Inquiry(InquiryData::RedChannel { gain }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: Some("CAM_RGainInq shares bytes with RedTuningInquiry; this entry exposes the absolute red-channel gain interpretation.");
            typed: (
                crate::types::RedChannel,
                { gain } => crate::types::RedChannel::new(gain)
            );
        }

        /// Inquiry command to get the current blue gain value.
        BlueGainInquiry => {
            const BLUE_GAIN = [0x81, 0x09, 0x04, 0x44];
            kind: BlueChannel {
                /// Blue channel absolute gain value.
                gain: u8,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                let gain = nibbles.u8_pair(2);
                Ok(Response::Inquiry(InquiryData::BlueChannel { gain }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: Some("CAM_BGainInq shares bytes with BlueTuningInquiry; this entry exposes the absolute blue-channel gain interpretation.");
            typed: (
                crate::types::BlueChannel,
                { gain } => crate::types::BlueChannel::new(gain)
            );
        }

        /// Inquiry command to get the current sharpness mode setting.
        SharpnessModeInquiry => {
            const SHARPNESS_MODE = [0x81, 0x09, 0x04, 0x05];
            kind: SharpnessMode {
                /// Current sharpness mode.
                mode: SharpnessMode,
            };
            decode: |payload| {
                require_nonempty(&payload)?;
                let mode = match payload.as_slice()[0] {
                    0x02 => SharpnessMode::Auto,
                    0x03 => SharpnessMode::Manual,
                    v => {
                        return Err(Error::InvalidParameter {
                            parameter: "sharpness_mode",
                            value: Cow::Owned(format!("0x{v:02X}")),
                            reason: Cow::Borrowed("Expected 0x02 (Auto) or 0x03 (Manual)"),
                        })
                    }
                };
                Ok(Response::Inquiry(InquiryData::SharpnessMode { mode }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (SharpnessMode, { mode } => Ok(mode));
        }

        /// Inquiry command to get the current color saturation level.
        SaturationInquiry => {
            const SATURATION = [0x81, 0x09, 0x04, 0x49];
            kind: Saturation {
                /// Saturation level (0x0=60% to 0xE=200%).
                level: u8,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                Ok(Response::Inquiry(InquiryData::Saturation { level: nibbles.last_nibble() }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (
                crate::types::SaturationLevel,
                { level } => crate::types::SaturationLevel::new(level)
            );
        }

        /// Inquiry command to get the current hue adjustment value.
        HueInquiry => {
            const HUE = [0x81, 0x09, 0x04, 0x4F];
            kind: Hue {
                /// Hue value (0x0=0 to 0xE=14).
                hue: u8,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                Ok(Response::Inquiry(InquiryData::Hue { hue: nibbles.last_nibble() }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (
                crate::types::HueLevel,
                { hue } => crate::types::HueLevel::new(hue)
            );
        }

        /// Inquiry command to get the current gain value.
        GainInquiry => {
            const GAIN = [0x81, 0x09, 0x04, 0x4C];
            kind: Gain {
                /// Gain level value (0x00=0 to 0x07=7).
                gain: u8,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                Ok(Response::Inquiry(InquiryData::Gain { gain: nibbles.last_nibble() }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (
                crate::types::GainLevel,
                { gain } => crate::types::GainLevel::new(gain)
            );
        }

        /// Inquiry command to get the current gain limit setting.
        GainLimitInquiry => {
            const GAIN_LIMIT = [0x81, 0x09, 0x04, 0x2C];
            kind: GainLimit {
                /// Maximum gain limit (0x0=0 to 0xF=15).
                limit: u8,
            };
            decode: |payload| {
                let nibbles = Nibbles::<1>::try_from(payload)?;
                Ok(Response::Inquiry(InquiryData::GainLimit {
                    limit: nibbles.byte(0),
                }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (
                crate::types::GainLimit,
                { limit } => crate::types::GainLimit::new(limit)
            );
        }

        /// Inquiry command to get the backlight compensation mode.
        BacklightInquiry => {
            const BACKLIGHT = [0x81, 0x09, 0x04, 0x33];
            kind: Backlight {
                /// Whether backlight compensation is enabled.
                status: bool,
            };
            decode: |payload| {
                let status = payload.parse_bool("backlight_status", BoolConvention::OnIs02)?;
                Ok(Response::Inquiry(InquiryData::Backlight { status }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (bool, { status } => Ok(status));
        }

        /// Inquiry command to get the combined flip state.
        ImageFlipInquiry => {
            const IMAGE_FLIP = [0x81, 0x09, 0x04, 0xA4];
            kind: FlipState {
                /// Whether horizontal flip is enabled.
                horizontal: bool,
                /// Whether vertical flip is enabled.
                vertical: bool,
            };
            decode: |payload| {
                decode_flip_state(payload)
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: Some("PTZOptics CAM_FlipInq returns combined horizontal/vertical flip state and intentionally shares bytes with FlipStateInquiry.");
            typed: (
                crate::command::FlipState,
                { horizontal, vertical } => Ok(crate::command::FlipState {
                    horizontal,
                    vertical,
                })
            );
        }

        /// Inquiry command to get the 2D noise reduction mode.
        NoiseReduction2DModeInquiry => {
            const NOISE_REDUCTION_2D_MODE = [0x81, 0x09, 0x04, 0x50];
            kind: NoiseReduction2DMode {
                /// Current 2D noise reduction mode.
                mode: NoiseReduction2DMode,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                let mode = NoiseReduction2DMode::try_from(payload.as_slice()[0])?;
                Ok(Response::Inquiry(InquiryData::NoiseReduction2DMode { mode }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: Some("The PTZOptics inquiry table identifies 04 50 as the 2D noise-reduction Auto/Manual mode register.");
            typed: (NoiseReduction2DMode, { mode } => Ok(mode));
        }

        /// Inquiry command to get the 2D noise reduction level.
        NoiseReduction2DInquiry => {
            const NOISE_REDUCTION_2D = [0x81, 0x09, 0x04, 0x53];
            kind: NoiseReduction2D {
                /// 2D noise reduction level.
                level: u8,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                let level = payload.as_slice()[0];
                Ok(Response::Inquiry(InquiryData::NoiseReduction2D { level }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: Some("The PTZOptics inquiry table identifies 04 53 as the 2D noise-reduction level register.");
            typed: (
                crate::types::NoiseReduction2DLevel,
                { level } => crate::types::NoiseReduction2DLevel::new(level)
            );
        }

        /// Inquiry command to get the 3D noise reduction level.
        ///
        /// [`ResponseParser::from_response`] is intentionally profile-neutral
        /// and accepts the public value type's `0..=8` domain. Camera/session
        /// execution applies the selected profile's source-backed reply range:
        /// `0..=5` for current PTZOptics G2/G3 and `0..=8` only for the exact
        /// legacy [`crate::profiles::PtzOptics30X`] profile.
        NoiseReduction3DInquiry => {
            const NOISE_REDUCTION_3D = [0x81, 0x09, 0x04, 0x54];
            kind: NoiseReduction3D {
                /// 3D noise reduction level.
                level: u8,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                let level = payload.as_slice()[0];
                Ok(Response::Inquiry(InquiryData::NoiseReduction3D { level }))
            };
            profile_decode: noise_reduction_3d;
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: Some("The PTZOptics inquiry table identifies 04 54 as the 3D noise-reduction level register.");
            typed: (
                crate::types::NoiseReduction3DLevel,
                { level } => crate::types::NoiseReduction3DLevel::new(level)
            );
        }

        /// Inquiry command to get the dynamic range mode/level.
        DynamicRangeInquiry => {
            const DYNAMIC_RANGE = [0x81, 0x09, 0x04, 0x25];
            kind: DynamicRange {
                /// Dynamic range level (0x0=0 to 0x8=8).
                level: u8,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                let level = payload.as_slice()[0];
                Ok(Response::Inquiry(InquiryData::DynamicRange { level }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: None;
            typed: (
                crate::types::DynamicRangeLevel,
                { level } => crate::types::DynamicRangeLevel::new(level)
            );
        }

        /// Inquiry command to get the current focus zone selection.
        FocusZoneInquiry => {
            const FOCUS_ZONE = [0x81, 0x09, 0x04, 0xAA];
            kind: FocusZone {
                /// Current focus zone setting.
                zone: FocusZone,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                let zone = match payload.as_slice()[0] {
                    0x00 => FocusZone::Top,
                    0x01 => FocusZone::Center,
                    0x02 => FocusZone::Bottom,
                    v => {
                        return Err(Error::InvalidParameter {
                            parameter: "focus_zone",
                            value: Cow::Owned(format!("{v:02X}")),
                            reason: Cow::Borrowed("Unknown focus zone value"),
                        })
                    }
                };
                Ok(Response::Inquiry(InquiryData::FocusZone { zone }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (FocusZone, { zone } => Ok(zone));
        }

        /// Inquiry command to get the auto-focus sensitivity setting.
        AutoFocusSensitivityInquiry => {
            const AUTO_FOCUS_SENSITIVITY = [0x81, 0x09, 0x04, 0x58];
            kind: AutoFocusSensitivity {
                /// Current auto-focus sensitivity setting.
                sensitivity: AutoFocusSensitivity,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                let sensitivity = match payload.as_slice()[0] {
                    0x01 => AutoFocusSensitivity::High,
                    0x02 => AutoFocusSensitivity::Normal,
                    0x03 => AutoFocusSensitivity::Low,
                    v => {
                        return Err(Error::InvalidParameter {
                            parameter: "auto_focus_sensitivity",
                            value: Cow::Owned(format!("{v:02X}")),
                            reason: Cow::Borrowed("Unknown auto focus sensitivity value"),
                        })
                    }
                };
                Ok(Response::Inquiry(InquiryData::AutoFocusSensitivity { sensitivity }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (
                AutoFocusSensitivity,
                { sensitivity } => Ok(sensitivity)
            );
        }

        /// Inquiry command to get the focus near limit position.
        FocusNearLimitInquiry => {
            const FOCUS_NEAR_LIMIT = [0x81, 0x09, 0x04, 0x28];
            kind: FocusNearLimit {
                /// Near limit position value.
                position: u16,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                let position = nibbles.u16_quad(0);
                Ok(Response::Inquiry(InquiryData::FocusNearLimit { position }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (
                crate::types::FocusPosition,
                { position } => Ok(crate::types::FocusPosition::new(position))
            );
        }

        /// Inquiry command to get the current focus mode.
        FocusModeInquiry => {
            const FOCUS_MODE = [0x81, 0x09, 0x04, 0x38];
            kind: FocusMode {
                /// Current focus mode (Auto or Manual).
                mode: FocusMode,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                let mode = match payload.as_slice()[0] {
                    0x02 => FocusMode::Auto,
                    0x03 => FocusMode::Manual,
                    v => {
                        return Err(Error::InvalidParameter {
                            parameter: "focus_mode",
                            value: Cow::Owned(format!("{v:02X}")),
                            reason: Cow::Borrowed("Unknown focus mode value"),
                        })
                    }
                };
                Ok(Response::Inquiry(InquiryData::FocusMode { mode }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (FocusMode, { mode } => Ok(mode));
        }

        /// Inquiry command to get the menu open/close status.
        MenuOpenCloseInquiry => {
            const MENU_OPEN_CLOSE = [0x81, 0x09, 0x06, 0x06];
            kind: MenuOpenClose {
                /// Whether the camera menu is open.
                is_open: bool,
            };
            decode: |payload| {
                let is_open = payload.parse_bool("menu_status", BoolConvention::OnIs02)?;
                Ok(Response::Inquiry(InquiryData::MenuOpenClose { is_open }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: Some("PTZOptics menu inquiries use category 0x06 to match menu command bytes.");
            typed: (bool, { is_open } => Ok(is_open));
        }

        /// Inquiry command to get combined tally light status.
        TallyStatusInquiry => {
            const TALLY_STATUS = [0x81, 0x09, 0x04, 0xA8];
            kind: TallyStatus {
                /// Whether the red tally light is on.
                red_on: bool,
                /// Whether the green tally light is on.
                green_on: bool,
            };
            decode: |payload| {
                require_len(&payload, 2)?;
                let red_payload = Payload::new(&payload.as_slice()[0..1]);
                let green_payload = Payload::new(&payload.as_slice()[1..2]);
                let red_on = red_payload.parse_bool("tally_red_status", BoolConvention::OnIs03)?;
                let green_on = green_payload.parse_bool("tally_green_status", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::TallyStatus { red_on, green_on }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: Some("PTZOptics extension returning packed red and green tally states.");
            typed: (
                crate::command::TallyStatusState,
                { red_on, green_on } => Ok(crate::command::TallyStatusState {
                    red_on,
                    green_on,
                })
            );
        }

        /// Inquiry command to get the night/day mode status.
        NightDayModeInquiry => {
            const NIGHT_DAY_MODE = [0x81, 0x09, 0x04, 0x60];
            kind: NightDayMode {
                /// Whether the camera is in night mode.
                is_night: bool,
            };
            decode: |payload| {
                let is_night = payload.parse_bool("night_day_mode", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::NightDayMode { is_night }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (bool, { is_night } => Ok(is_night));
        }

        /// Inquiry command to get the ND filter position.
        NdFilterInquiry => {
            const ND_FILTER = [0x81, 0x09, 0x04, 0x64];
            kind: NdFilter {
                /// Current ND filter position (Clear, 1/4, 1/8, 1/16, etc.).
                position: NdFilterPosition,
            };
            decode: |payload| {
                require_nonempty(&payload)?;
                Ok(Response::Inquiry(InquiryData::NdFilter {
                    position: NdFilterPosition::from_byte(payload.as_slice()[0]),
                }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (NdFilterPosition, { position } => Ok(position));
        }

        /// Inquiry command to get the current picture effect mode.
        PictureEffectInquiry => {
            const PICTURE_EFFECT = [0x81, 0x09, 0x04, 0x63];
            kind: PictureEffect {
                /// Current picture effect (Off or Black & White for built-in profiles).
                effect: PictureEffectMode,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                Ok(Response::Inquiry(InquiryData::PictureEffect {
                    effect: PictureEffectMode::from_byte(payload.as_slice()[0]),
                }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (PictureEffectMode, { effect } => Ok(effect));
        }

        /// Inquiry command to get the current flip mode.
        FlipStateInquiry => {
            const FLIP_MODE = [0x81, 0x09, 0x04, 0xA4];
            kind: FlipState {
                /// Whether horizontal flip is enabled.
                horizontal: bool,
                /// Whether vertical flip is enabled.
                vertical: bool,
            };
            decode: |payload| {
                decode_flip_state(payload)
            };
            response: false;
            query: BuiltinInquiryQuery::Alias {
                canonical: <ImageFlipInquiry as BuiltinInquiryCommandMarker>::METADATA,
            };
            vendor_specific: true;
            rationale: Some("Public flip-mode accessor intentionally aliases ImageFlipInquiry because both expose CAM_FlipInq.");
            typed: (
                crate::command::FlipState,
                { horizontal, vertical } => Ok(crate::command::FlipState {
                    horizontal,
                    vertical,
                })
            );
        }

        /// Inquiry command to get the standby mode status.
        StandbyInquiry => {
            const STANDBY = [0x81, 0x09, 0x04, 0x70];
            kind: Standby {
                /// Whether the camera is in standby mode.
                in_standby: bool,
            };
            decode: |payload| {
                let in_standby = payload.parse_bool("standby_mode", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::Standby { in_standby }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (bool, { in_standby } => Ok(in_standby));
        }

        /// Inquiry command to get the focus range setting.
        FocusRangeInquiry => {
            const FOCUS_RANGE = [0x81, 0x09, 0x04, 0x2A];
            kind: FocusRange {
                /// Current focus range setting.
                range: FocusRange,
            };
            decode: |payload| {
                require_nonempty(&payload)?;
                let range = FocusRange::try_from(payload.as_slice()[0])?;
                Ok(Response::Inquiry(InquiryData::FocusRange { range }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (FocusRange, { range } => Ok(range));
        }

        /// Inquiry command to get the iris control mode.
        IrisControlInquiry => {
            const IRIS_CONTROL = [0x81, 0x09, 0x04, 0x2B];
            kind: IrisControl {
                /// Whether iris is in auto mode.
                auto: bool,
            };
            decode: |payload| {
                let auto = payload.parse_bool("iris_control", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::IrisControl { auto }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (bool, { auto } => Ok(auto));
        }

        /// Inquiry command to get the defog mode status.
        DefogModeInquiry => {
            const DEFOG_MODE = [0x81, 0x09, 0x04, 0x37];
            kind: DefogMode {
                /// Whether defog is enabled.
                enabled: bool,
            };
            decode: |payload| {
                let enabled = payload.parse_bool("defog_mode", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::DefogMode { enabled }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: None;
            typed: none;
        }

        /// Inquiry command to get the defog level.
        DefogLevelInquiry => {
            const DEFOG_LEVEL = [0x81, 0x09, 0x04, 0xA0];
            kind: DefogLevel {
                /// Current defog strength level (0-5).
                level: DefogLevel,
            };
            decode: |payload| {
                require_nonempty(&payload)?;
                let level = DefogLevel::new(payload.as_slice()[0]).map_err(|_| Error::InvalidParameter {
                    parameter: "defog_level",
                    value: Cow::Owned(payload.as_slice()[0].to_string()),
                    reason: Cow::Borrowed("value out of range (0-5)"),
                })?;
                Ok(Response::Inquiry(InquiryData::DefogLevel { level }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: None;
            typed: (DefogLevel, { level } => Ok(level));
        }

        /// Inquiry command to get the digital PTZ mode status.
        DigitalPtzInquiry => {
            const DIGITAL_PTZ = [0x81, 0x09, 0x04, 0x6B];
            kind: DigitalPtz {
                /// Whether digital Ptz is enabled.
                enabled: bool,
            };
            decode: |payload| {
                let enabled = payload.parse_bool("digital_ptz", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::DigitalPtz { enabled }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: None;
            typed: (bool, { enabled } => Ok(enabled));
        }

        /// Inquiry command to get the auto white balance sensitivity setting.
        AutoWhiteBalanceSensitivityInquiry => {
            const AUTO_WB_SENSITIVITY = [0x81, 0x09, 0x04, 0xA9];
            kind: AutoWhiteBalanceSensitivity {
                /// Sensitivity level (Low, Normal, High).
                sensitivity: AutoWhiteBalanceSensitivity,
            };
            decode: |payload| {
                require_nonempty(&payload)?;
                let sensitivity = match payload.as_slice()[0] {
                    0x00 => AutoWhiteBalanceSensitivity::High,
                    0x01 => AutoWhiteBalanceSensitivity::Normal,
                    0x02 => AutoWhiteBalanceSensitivity::Low,
                    v => {
                        return Err(Error::InvalidParameter {
                            parameter: "auto_wb_sensitivity",
                            value: Cow::Owned(format!("0x{v:02X}")),
                            reason: Cow::Borrowed(
                                "Invalid auto white balance sensitivity. Expected 0x00 (High), 0x01 (Normal), or 0x02 (Low)",
                            ),
                        })
                    }
                };
                Ok(Response::Inquiry(InquiryData::AutoWhiteBalanceSensitivity { sensitivity }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: Some("Shares bytes with TallyAutoAdjustInquiry on PTZOptics profiles; this entry interprets the register as AWB sensitivity.");
            typed: (
                AutoWhiteBalanceSensitivity,
                { sensitivity } => Ok(sensitivity)
            );
        }

        /// Inquiry command to get the exposure compensation position.
        ExposureCompensationPositionInquiry => {
            const EXPOSURE_COMPENSATION_POSITION = [0x81, 0x09, 0x04, 0x4E];
            kind: ExposureCompensationPosition {
                /// Exposure compensation position value (high-resolution EV adjustment).
                position: ExposureCompensationPosition,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                let position = ExposureCompensationPosition::new(nibbles.u16_quad(0));
                Ok(Response::Inquiry(InquiryData::ExposureCompensationPosition { position }))
            };
            response: true;
            query: BuiltinInquiryQuery::AlternateTypedInterpretation {
                canonical: <ExposureCompensationInquiry as BuiltinInquiryCommandMarker>::METADATA,
            };
            vendor_specific: false;
            rationale: Some("Same wire query as ExposureCompensationInquiry, exposed as the high-resolution position newtype.");
            typed: (
                ExposureCompensationPosition,
                { position } => Ok(position)
            );
        }

        /// Inquiry command to get the red channel tuning level.
        RedTuningInquiry => {
            const RED_TUNING = [0x81, 0x09, 0x04, 0x43];
            kind: RedTuning {
                /// Red channel tuning level (-10 to +10).
                level: i8,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                let raw = nibbles.u8_pair(2);
                #[allow(clippy::cast_possible_wrap)]
                let level = raw as i8 - 10;
                Ok(Response::Inquiry(InquiryData::RedTuning { level }))
            };
            response: true;
            query: BuiltinInquiryQuery::AlternateTypedInterpretation {
                canonical: <RedGainInquiry as BuiltinInquiryCommandMarker>::METADATA,
            };
            vendor_specific: false;
            rationale: Some("Same register as RedGainInquiry, interpreted as signed white-balance tuning.");
            typed: (
                crate::types::RedTuning,
                { level } => crate::types::RedTuning::new(level)
            );
        }

        /// Inquiry command to get the blue channel tuning level.
        BlueTuningInquiry => {
            const BLUE_TUNING = [0x81, 0x09, 0x04, 0x44];
            kind: BlueTuning {
                /// Blue channel tuning level (-10 to +10).
                level: i8,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                let raw = nibbles.u8_pair(2);
                #[allow(clippy::cast_possible_wrap)]
                let level = raw as i8 - 10;
                Ok(Response::Inquiry(InquiryData::BlueTuning { level }))
            };
            response: true;
            query: BuiltinInquiryQuery::AlternateTypedInterpretation {
                canonical: <BlueGainInquiry as BuiltinInquiryCommandMarker>::METADATA,
            };
            vendor_specific: false;
            rationale: Some("Same register as BlueGainInquiry, interpreted as signed white-balance tuning.");
            typed: (
                crate::types::BlueTuning,
                { level } => crate::types::BlueTuning::new(level)
            );
        }

        /// Inquiry command to get the current gamma curve setting.
        GammaInquiry => {
            const GAMMA = [0x81, 0x09, 0x04, 0x5B];
            kind: Gamma {
                /// Gamma curve setting (0=Standard, 1-4=different gamma curves).
                value: u8,
            };
            decode: |payload| {
                require_nonempty(&payload)?;
                Ok(Response::Inquiry(InquiryData::Gamma { value: payload.as_slice()[0] }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: None;
            typed: (
                crate::types::GammaLevel,
                { value } => crate::types::GammaLevel::new(value)
            );
        }

        /// Inquiry command to get the auto trace mode status.
        AutoTraceInquiry => {
            const AUTO_TRACE = [0x81, 0x09, 0x50, 0x09];
            kind: AutoTrace {
                /// Whether auto trace is enabled.
                enabled: bool,
            };
            decode: |payload| {
                let enabled = payload.parse_bool("auto_trace", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::AutoTrace { enabled }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: Some("Uses the non-default auto-trace inquiry category 0x50.");
            typed: (bool, { enabled } => Ok(enabled));
        }

        /// Inquiry command to get the focus unlock state.
        FocusUnlockInquiry => {
            const FOCUS_UNLOCK = [0x81, 0x09, 0x54, 0x08];
            kind: FocusUnlock {
                /// Whether focus is unlocked.
                unlocked: bool,
            };
            decode: |payload| {
                let unlocked = payload.parse_bool("focus_unlock", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::FocusUnlock { unlocked }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: Some("Uses the non-default focus-unlock inquiry category 0x54.");
            typed: (bool, { unlocked } => Ok(unlocked));
        }

        /// Inquiry command to get the current sharpness position.
        SharpnessPositionInquiry => {
            const SHARPNESS_POSITION = [0x81, 0x09, 0x04, 0x42];
            kind: SharpnessPosition {
                /// Current sharpness position value.
                position: u16,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                let position = nibbles.u16_quad(0);
                Ok(Response::Inquiry(InquiryData::SharpnessPosition { position }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (
                crate::types::SharpnessLevel,
                { position } => crate::types::SharpnessLevel::new(position as u8)
            );
        }

        /// Inquiry command to get the broadcast domain setting.
        BroadcastDomainInquiry => {
            const BROADCAST_DOMAIN = [0x81, 0x09, 0x04, 0x75];
            kind: BroadcastDomain (BroadcastDomain);
            decode: |payload| {
                require_len(&payload, 1)?;
                let domain = BroadcastDomain::new(payload.as_slice()[0]).map_err(|_| {
                    Error::InvalidParameter {
                        parameter: "broadcast_domain",
                        value: Cow::Owned(payload.as_slice()[0].to_string()),
                        reason: Cow::Borrowed("value out of range (0-3)"),
                    }
                })?;
                Ok(Response::Inquiry(InquiryData::BroadcastDomain(domain)))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: None;
            typed: (BroadcastDomain, (val) => Ok(val));
        }

        /// Inquiry command to get the motion sync mode setting.
        MotionSyncModeInquiry => {
            const MOTION_SYNC_MODE = [0x81, 0x09, 0x04, 0x56];
            kind: MotionSyncMode {
                /// Current motion sync mode setting.
                mode: MotionSyncMode,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                let mode = MotionSyncMode::try_from(payload.as_slice()[0])?;
                Ok(Response::Inquiry(InquiryData::MotionSyncMode { mode }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: None;
            typed: (MotionSyncMode, { mode } => Ok(mode));
        }

        /// Inquiry command to get the motion sync speed setting.
        MotionSyncPresetInquiry => {
            const MOTION_SYNC_SPEED = [0x81, 0x09, 0x04, 0x57];
            kind: MotionSyncPreset {
                /// Current motion sync speed setting.
                speed: MotionSyncPreset,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                let speed = MotionSyncPreset::try_from(payload.as_slice()[0])?;
                Ok(Response::Inquiry(InquiryData::MotionSyncPreset { speed }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: None;
            typed: (MotionSyncPreset, { speed } => Ok(speed));
        }


        /// Inquiry command to get the USB audio state.
        UsbAudioInquiry => {
            const USB_AUDIO = [0x81, 0x2A, 0x02, 0xA0, 0x04];
            kind: UsbAudio {
                /// Whether USB audio is enabled.
                on: bool,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                let on = payload.parse_bool("usb_audio_status", BoolConvention::OnIs02)?;
                Ok(Response::Inquiry(InquiryData::UsbAudio { on }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: None;
            typed: (bool, { on } => Ok(on));
        }

        /// Inquiry command to get the two tone mode state.
        TwoToneModeInquiry => {
            const TWO_TONE_MODE = [0x81, 0x09, 0x04, 0x74];
            kind: TwoToneMode {
                /// Whether two tone mode is enabled.
                on: bool,
            };
            decode: |payload| {
                let on = payload.parse_bool("two_tone_mode_status", BoolConvention::OnIs02)?;
                Ok(Response::Inquiry(InquiryData::TwoToneMode { on }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: None;
            typed: (bool, { on } => Ok(on));
        }

        /// Inquiry command to get the ND filter preset setting.
        NdFilterPresetInquiry => {
            const ND_FILTER_PRESET = [0x81, 0x09, 0x04, 0x66];
            kind: NdFilterPreset {
                /// Current ND filter preset number.
                preset: NdFilterPreset,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                let preset = NdFilterPreset::new(payload.as_slice()[0]).map_err(|_| {
                    Error::InvalidParameter {
                        parameter: "nd_filter_preset",
                        value: Cow::Owned(payload.as_slice()[0].to_string()),
                        reason: Cow::Borrowed("value out of range (0-3)"),
                    }
                })?;
                Ok(Response::Inquiry(InquiryData::NdFilterPreset { preset }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: None;
            typed: (NdFilterPreset, { preset } => Ok(preset));
        }

        /// Inquiry command to get the digital mode state.
        DigitalInquiry => {
            const DIGITAL = [0x81, 0x09, 0x04, 0x7B];
            kind: Digital {
                /// Whether digital mode is enabled.
                on: bool,
            };
            decode: |payload| {
                let on = payload.parse_bool("digital_mode_status", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::Digital { on }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: None;
            typed: (bool, { on } => Ok(on));
        }

        /// Inquiry command to get the tally auto adjust state.
        TallyAutoAdjustInquiry => {
            const TALLY_AUTO_ADJUST = [0x81, 0x09, 0x04, 0xA9];
            kind: TallyAutoAdjust {
                /// Whether tally auto adjust is enabled.
                on: bool,
            };
            decode: |payload| {
                let on = payload.parse_bool("tally_auto_adjust_status", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::TallyAutoAdjust { on }))
            };
            response: true;
            query: BuiltinInquiryQuery::AlternateTypedInterpretation {
                canonical: <AutoWhiteBalanceSensitivityInquiry as BuiltinInquiryCommandMarker>::METADATA,
            };
            vendor_specific: true;
            rationale: Some("PTZOptics interprets the same register used by AWB sensitivity as tally auto-adjust state.");
            typed: (bool, { on } => Ok(on));
        }

        /// Inquiry command to get the red tally light status.
        TallyRedInquiry => {
            const TALLY_RED = [0x81, 0x09, 0x7E, 0x01, 0x0A, 0x00];
            kind: TallyRed {
                /// Whether the red tally light is on.
                on: bool,
            };
            decode: |payload| {
                let on = payload.parse_bool("tally_red_status", BoolConvention::OnIs02)?;
                Ok(Response::Inquiry(InquiryData::TallyRed { on }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: false;
            rationale: Some("Extended baseline tally inquiry with non-standard byte length.");
            typed: (bool, { on } => Ok(on));
        }

        /// Inquiry command to get the green tally light status.
        TallyGreenInquiry => {
            const TALLY_GREEN = [0x81, 0x09, 0x7E, 0x04, 0x1A, 0x00];
            kind: TallyGreen {
                /// Whether the green tally light is on.
                on: bool,
            };
            decode: |payload| {
                let on = payload.parse_bool("tally_green_status", BoolConvention::OnIs02)?;
                Ok(Response::Inquiry(InquiryData::TallyGreen { on }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: Some("Sony FR7 extended tally inquiry with non-standard byte length.");
            typed: (bool, { on } => Ok(on));
        }

        /// Inquiry command to get the current flicker mode setting.
        FlickerModeInquiry => {
            const FLICKER_MODE = [0x81, 0x09, 0x04, 0x55];
            kind: FlickerMode {
                /// Current anti-flicker mode setting.
                mode: AntiFlickerMode,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                let mode = match payload.as_slice()[0] {
                    0x00 => AntiFlickerMode::Off,
                    0x01 => AntiFlickerMode::Hz50,
                    0x02 => AntiFlickerMode::Hz60,
                    v => {
                        return Err(Error::InvalidParameter {
                            parameter: "flicker_mode",
                            value: Cow::Owned(format!("{v:02X}")),
                            reason: Cow::Borrowed("Unknown flicker mode value"),
                        });
                    }
                };
                Ok(Response::Inquiry(InquiryData::FlickerMode { mode }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: None;
            typed: (AntiFlickerMode, { mode } => Ok(mode));
        }

        /// Inquiry command to get the current contrast level.
        ContrastInquiry => {
            const CONTRAST = [0x81, 0x09, 0x04, 0xA2];
            kind: Contrast {
                /// Current contrast level.
                level: u8,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                Ok(Response::Inquiry(InquiryData::Contrast { level: nibbles.last_nibble() }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: None;
            typed: (
                crate::types::ContrastLevel,
                { level } => crate::types::ContrastLevel::new(level)
            );
        }

        /// Inquiry command to get the current luminance level.
        LuminanceInquiry => {
            const LUMINANCE_LEVEL = [0x81, 0x09, 0x04, 0xA1];
            kind: Luminance {
                /// Current luminance (image processing brightness) level.
                level: u8,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                Ok(Response::Inquiry(InquiryData::Luminance { level: nibbles.last_nibble() }))
            };
            response: true;
            query: BuiltinInquiryQuery::Queryable;
            vendor_specific: true;
            rationale: None;
            typed: (
                crate::types::LuminanceLevel,
                { level } => crate::types::LuminanceLevel::new(level)
            );
        }
    }

    decode_only {
        ZoomOut => {
            data: {
                /// Whether zoom out is active.
                active: bool,
            };
            decode: |payload| {
                let active = payload.parse_bool("zoom_out_status", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::ZoomOut { active }))
            };
            vendor_specific: false;
            rationale: Some("Movement-state response decoded for routing, but no public built-in wire query is exposed.");
        }
        ZoomIn => {
            data: {
                /// Whether zoom in is active.
                active: bool,
            };
            decode: |payload| {
                let active = payload.parse_bool("zoom_in_status", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::ZoomIn { active }))
            };
            vendor_specific: false;
            rationale: Some("Movement-state response decoded for routing, but no public built-in wire query is exposed.");
        }
        ZoomTeleWide => {
            data: {
                /// Whether zoom tele is active (false = wide active).
                tele: bool,
            };
            decode: |payload| {
                let tele = payload.parse_bool("zoom_tele_wide_status", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::ZoomTeleWide { tele }))
            };
            vendor_specific: false;
            rationale: Some("Movement-state response decoded for routing, but no public built-in wire query is exposed.");
        }
        AutoFocus => {
            data: {
                /// Whether auto focus is enabled.
                enabled: bool,
            };
            decode: |payload| {
                let enabled = payload.parse_bool("autofocus_status", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::AutoFocus { enabled }))
            };
            vendor_specific: false;
            rationale: Some("One-push autofocus status is decoded but not exposed as a built-in query command.");
        }
        FocusNearFar => {
            data: {
                /// Whether focus near is active (false = far active).
                near: bool,
            };
            decode: |payload| {
                let near = payload.parse_bool("focus_near_far_status", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::FocusNearFar { near }))
            };
            vendor_specific: false;
            rationale: Some("Near/far movement-state response decoded for routing, but no public built-in wire query is exposed.");
        }
        IrisUp => {
            data: {
                /// Whether iris up is active.
                active: bool,
            };
            decode: |payload| {
                let active = payload.parse_bool("iris_up_status", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::IrisUp { active }))
            };
            vendor_specific: false;
            rationale: Some("Iris movement-state response decoded for routing, but no public built-in wire query is exposed.");
        }
        IrisDown => {
            data: {
                /// Whether iris down is active.
                active: bool,
            };
            decode: |payload| {
                let active = payload.parse_bool("iris_down_status", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::IrisDown { active }))
            };
            vendor_specific: false;
            rationale: Some("Iris movement-state response decoded for routing, but no public built-in wire query is exposed.");
        }
        Sharpness => {
            data: {
                /// Current sharpness value.
                value: u8,
            };
            decode: |payload| {
                let nibbles = Nibbles::<4>::try_from(payload)?;
                let value = nibbles.u8_pair(2);
                Ok(Response::Inquiry(InquiryData::Sharpness { value }))
            };
            vendor_specific: false;
            rationale: Some("Legacy sharpness value response is decoded; public queries use SharpnessPositionInquiry.");
        }
        Rtmp => {
            data: {
                /// Whether RTMP streaming is enabled.
                on: bool,
            };
            decode: |payload| {
                let on = payload.parse_bool("rtmp_status", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::Rtmp { on }))
            };
            vendor_specific: true;
            rationale: Some("Streaming-state response decoded for vendor integrations, but no public built-in query command is exposed.");
        }
        NightDayPosition => {
            data: {
                /// Current night/day position value.
                position: u8,
            };
            decode: |payload| {
                require_len(&payload, 1)?;
                Ok(Response::Inquiry(InquiryData::NightDayPosition { position: payload.as_slice()[0] }))
            };
            vendor_specific: false;
            rationale: Some("Night/day position response is decode-only; public API exposes NightDayModeInquiry.");
        }
        NightDaySwitch => {
            data: {
                /// Whether night/day switch is enabled.
                enabled: bool,
            };
            decode: |payload| {
                let enabled = payload.parse_bool("night_day_switch", BoolConvention::OnIs03)?;
                Ok(Response::Inquiry(InquiryData::NightDaySwitch { enabled }))
            };
            vendor_specific: false;
            rationale: Some("Night/day switch response is decode-only; public API exposes NightDayModeInquiry.");
        }
    }

    accessors {
        // Base-domain inquiries carry no `where` clause of their own, so on the
        // static facades they inherit their noun accessor's base-domain marker
        // (`noun_marker!` in `crate::command::surface`). `base_gate` reproduces
        // that same gate for the erased surface at runtime (#684), so a runtime
        // `ProfileSpec` missing the domain cannot reach `dyn <noun>().<inquiry>()`
        // any more than the static `<noun>()` accessor can be named without the
        // marker. Truly ungated inquiries — the `System` and `Advanced` nouns
        // (`noun_marker!` = `None`) and the universally-reachable `Menu` noun —
        // stay in `InquiryControl` with `gate: none`.
        PowerInquiryControl {
            base_gate: crate::capabilities::HasPower;
            PowerInquiry => power_state: bool;
        }
        ZoomInquiryControl {
            base_gate: crate::capabilities::HasZoom;
            ZoomPositionInquiry => zoom_position: crate::types::ZoomPosition;
        }
        FocusInquiryControl {
            base_gate: crate::capabilities::HasFocus;
            FocusPositionInquiry => focus_position: crate::types::FocusPosition;
            FocusModeInquiry => focus_mode: FocusMode;
            FocusRangeInquiry => focus_range: FocusRange;
        }
        ExposureModeInquiryControl {
            gate: crate::capabilities::HasExposureMode;
            ExposureModeInquiry => exposure_mode: ExposureMode;
        }
        ExposureInquiryControl {
            base_gate: crate::capabilities::HasExposure;
            ShutterInquiry => shutter: crate::types::ShutterSpeed;
            GainInquiry => gain: crate::types::GainLevel;
            GainLimitInquiry => gain_limit: crate::types::GainLimit;
        }
        PtzOpticsAntiFlickerInquiryControl {
            gate: crate::capabilities::HasPtzOpticsAntiFlicker;
            FlickerModeInquiry => flicker_mode: crate::command::exposure::AntiFlickerMode;
        }
        WhiteBalanceInquiryControl {
            base_gate: crate::capabilities::HasWhiteBalance;
            WhiteBalanceModeInquiry => white_balance_mode: WhiteBalanceMode;
        }
        ImageInquiryControl {
            base_gate: crate::capabilities::HasImageProcessing;
            DefogLevelInquiry => defog_level: crate::types::DefogLevel;
        }
        InquiryControl {
            gate: none;
            VersionInquiry => version: crate::command::VersionInfo;
            MenuOpenCloseInquiry => menu_status: bool;
            NightDayModeInquiry => night_day_mode: bool;
            StandbyInquiry => standby_enabled: bool;
            DigitalPtzInquiry => digital_ptz_enabled: bool;
            AutoTraceInquiry => auto_trace_enabled: bool;
            FocusUnlockInquiry => focus_unlock: bool;
            BroadcastDomainInquiry => broadcast_domain: crate::types::BroadcastDomain;
            TwoToneModeInquiry => two_tone_mode_enabled: bool;
            DigitalInquiry => digital_mode_enabled: bool;
        }
        UsbAudioInquiryControl {
            gate: crate::capabilities::HasUsbAudio;
            UsbAudioInquiry => usb_audio_enabled: bool;
        }
        BrightnessInquiryControl {
            gate: crate::capabilities::HasBrightnessControl;
            BrightnessInquiry => brightness: crate::types::BrightnessLevel;
        }
        ContrastInquiryControl {
            gate: crate::capabilities::HasContrastControl;
            ContrastInquiry => contrast: crate::types::ContrastLevel;
        }
        SharpnessInquiryControl {
            gate: crate::capabilities::HasSharpnessControl;
            SharpnessModeInquiry => sharpness_mode: SharpnessMode;
            SharpnessPositionInquiry => sharpness_level: crate::types::SharpnessLevel;
        }
        TallyControl {
            gate: crate::capabilities::HasTally;
            TallyStatusInquiry => tally_status: crate::command::TallyStatusState;
            TallyRedInquiry => red_tally_status: bool;
            TallyGreenInquiry => green_tally_status: bool;
            TallyAutoAdjustInquiry => tally_auto_adjust_enabled: bool;
        }
        ExposureCompensationInquiryControl {
            gate: crate::capabilities::HasExposureCompensation;
            ExposureCompensationInquiry => exposure_compensation: crate::types::ExposureCompensationLevel;
            ExposureCompensationModeInquiry => exposure_compensation_enabled: bool;
            ExposureCompensationPositionInquiry => exposure_compensation_position: crate::types::ExposureCompensationPosition;
        }
        BacklightCompensationInquiryControl {
            gate: crate::capabilities::HasBacklightCompensation;
            BacklightInquiry => backlight_enabled: bool;
        }
        WideDynamicRangeInquiryControl {
            gate: crate::capabilities::HasWideDynamicRange;
            DynamicRangeInquiry => dynamic_range: crate::types::DynamicRangeLevel;
        }
        ColorTemperatureInquiryControl {
            gate: crate::capabilities::HasColorTemperature;
            ColorTemperatureInquiry => color_temperature: crate::types::ColorTemp;
        }
        RgbGainInquiryControl {
            gate: crate::capabilities::HasRgbGain;
            RedGainInquiry => red_gain: crate::types::RedChannel;
            BlueGainInquiry => blue_gain: crate::types::BlueChannel;
        }
        RgbTuningInquiryControl {
            gate: crate::capabilities::HasRgbTuning;
            RedTuningInquiry => red_tuning: crate::types::RedTuning;
            BlueTuningInquiry => blue_tuning: crate::types::BlueTuning;
        }
        SaturationInquiryControl {
            gate: crate::capabilities::HasSaturationControl;
            SaturationInquiry => saturation: crate::types::SaturationLevel;
        }
        HueInquiryControl {
            gate: crate::capabilities::HasHueControl;
            HueInquiry => hue: crate::types::HueLevel;
        }
        LuminanceInquiryControl {
            gate: crate::capabilities::HasLuminanceControl;
            LuminanceInquiry => luminance: crate::types::LuminanceLevel;
        }
        GammaInquiryControl {
            gate: crate::capabilities::HasGammaControl;
            GammaInquiry => gamma: crate::types::GammaLevel;
        }
        ImageFlipInquiryControl {
            gate: crate::capabilities::HasImageFlip;
            ImageFlipInquiry => image_flip: crate::command::FlipState;
            FlipStateInquiry => flip_mode: crate::command::FlipState;
        }
        NoiseReduction2DInquiryControl {
            gate: crate::capabilities::HasNoiseReduction2D;
            NoiseReduction2DModeInquiry => noise_reduction_2d_mode: crate::command::NoiseReduction2DMode;
            NoiseReduction2DInquiry => noise_reduction_2d: crate::types::NoiseReduction2DLevel;
        }
        NoiseReduction3DInquiryControl {
            gate: crate::capabilities::HasNoiseReduction3D;
            NoiseReduction3DInquiry => noise_reduction_3d: crate::types::NoiseReduction3DLevel;
        }
        PictureEffectInquiryControl {
            gate: crate::capabilities::HasPictureEffect;
            PictureEffectInquiry => picture_effect: crate::command::PictureEffectMode;
        }
        NdFilterInquiryControl {
            gate: crate::capabilities::HasNdFilter;
            NdFilterInquiry => nd_filter_position: crate::command::NdFilterPosition;
            NdFilterPresetInquiry => nd_filter_preset: crate::types::NdFilterPreset;
        }
        MotionSyncControl {
            gate: crate::capabilities::HasMotionSync;
            MotionSyncModeInquiry => motion_sync_mode: crate::command::MotionSyncMode;
            MotionSyncPresetInquiry => motion_sync_speed: crate::command::MotionSyncPreset;
        }
        FocusNearLimitInquiryControl {
            gate: crate::capabilities::HasFocusNearLimitInquiry;
            FocusNearLimitInquiry => focus_near_limit: crate::types::FocusPosition;
        }
        FocusZoneInquiryControl {
            gate: crate::capabilities::HasFocusZoneInquiry;
            FocusZoneInquiry => focus_zone: FocusZone;
        }
        AutoFocusSensitivityInquiryControl {
            gate: crate::capabilities::HasAutoFocusSensitivity;
            AutoFocusSensitivityInquiry => auto_focus_sensitivity: AutoFocusSensitivity;
        }
        AutoWhiteBalanceSensitivityInquiryControl {
            gate: crate::capabilities::HasAutoWhiteBalanceSensitivity;
            AutoWhiteBalanceSensitivityInquiry => auto_white_balance_sensitivity: AutoWhiteBalanceSensitivity;
        }
        IrisControlInquiryControl {
            gate: crate::capabilities::HasIrisControlInquiry;
            IrisControlInquiry => iris_control: bool;
        }
        IrisInquiryControl {
            gate: crate::capabilities::HasIrisControl;
            IrisInquiry => iris: crate::types::IrisLevel;
        }
        PanTiltInquiryControl {
            gate: none;
            PanTiltPositionInquiry => pan_tilt_position: crate::camera::PanTiltPosition;
        }
    }
        }
    };
}

pub(crate) use builtin_inquiry_table;

builtin_inquiry_table!(define_builtin_inquiries);

// ============================================================
// Helper functions
// ============================================================

/// Require payload to have exactly the specified length.
#[inline]
fn require_len(payload: &Payload<'_>, expected: usize) -> Result<(), Error> {
    if payload.len() != expected {
        return Err(Error::invalid_response_length(expected, payload.as_slice()));
    }
    Ok(())
}

/// Require payload to be non-empty.
#[inline]
fn require_nonempty(payload: &Payload<'_>) -> Result<(), Error> {
    if payload.is_empty() {
        return Err(Error::invalid_response_length(1, payload.as_slice()));
    }
    Ok(())
}

/// Decode the combined horizontal/vertical flip-state response shared by the
/// two public inquiry names for `CAM_FlipInq`.
fn decode_flip_state(payload: Payload<'_>) -> Result<Response, Error> {
    require_len(&payload, 1)?;

    let (horizontal, vertical) = match payload.as_slice()[0] {
        0x00 => (false, false),
        0x01 => (true, false),
        0x02 => (false, true),
        0x03 => (true, true),
        value => {
            return Err(Error::InvalidParameter {
                parameter: "image_flip_mode",
                value: Cow::Owned(format!("{value:02X}")),
                reason: Cow::Borrowed("Expected a combined flip mode from 0x00 through 0x03"),
            })
        }
    };

    Ok(Response::Inquiry(InquiryData::FlipState {
        horizontal,
        vertical,
    }))
}

/// Decode PanTiltPosition without profile awareness.
///
/// This decoder interprets pan/tilt positions as signed 16-bit values.
fn decode_pan_tilt_position(payload: Payload<'_>) -> Result<Response, Error> {
    if payload.len() == 8 {
        let nibbles = Nibbles::<8>::try_from(payload)?;
        let pan = i32::from(nibbles.i16_quad(0));
        let tilt = i32::from(nibbles.i16_quad(4));
        Ok(Response::Inquiry(InquiryData::PanTiltPosition {
            pan,
            tilt,
        }))
    } else if payload.len() == 4 {
        tracing::warn!(
            "PanTiltPosition: Received compact format (4 bytes). Payload: {:02X?}. Treating as home position.",
            payload.as_slice()
        );
        let pan = if payload.len() >= 2 {
            #[allow(clippy::cast_possible_wrap)]
            let p = ((payload.as_slice()[0] as i16) << 8) | (payload.as_slice()[1] as i16);
            i32::from(p)
        } else {
            0
        };
        let tilt = if payload.len() >= 4 {
            #[allow(clippy::cast_possible_wrap)]
            let t = ((payload.as_slice()[2] as i16) << 8) | (payload.as_slice()[3] as i16);
            i32::from(t)
        } else {
            0
        };
        Ok(Response::Inquiry(InquiryData::PanTiltPosition {
            pan,
            tilt,
        }))
    } else {
        tracing::debug!(
            "PanTiltPosition: Payload length {} doesn't match pan/tilt format (expected 8 or 4 bytes)",
            payload.len()
        );
        Err(Error::DecoderNotFound {
            inquiry_kind: InquiryKind::PanTiltPosition,
            payload_hex: format_payload_hex(payload.as_slice()),
        })
    }
}

/// Decode PanTiltPosition using the profile's coordinate-system conversion.
fn decode_pan_tilt_position_for<P: Profile + PanTilt>(
    payload: Payload<'_>,
) -> Result<Response, Error> {
    decode_pan_tilt_position_with_codec(payload, P::PAN_TILT_WIRE_CODEC, P::COORDINATE_SYSTEM)
}

fn decode_pan_tilt_position_with_codec(
    payload: Payload<'_>,
    codec: PanTiltWireCodec,
    coordinate_system: crate::capabilities::CoordinateSystem,
) -> Result<Response, Error> {
    match codec {
        PanTiltWireCodec::StandardVisca => {
            if payload.len() == 8 {
                let nibbles = Nibbles::<8>::try_from(payload)?;
                let pan_u16 = nibbles.u16_quad(0);
                let tilt_u16 = nibbles.u16_quad(4);
                let (pan, tilt) = coordinate_system.convert_from_camera_coords(pan_u16, tilt_u16);

                Ok(Response::Inquiry(InquiryData::PanTiltPosition {
                    pan: i32::from(pan),
                    tilt: i32::from(tilt),
                }))
            } else if payload.len() == 4 {
                tracing::warn!(
                    "PanTiltPosition: Received compact standard VISCA format (4 bytes). Payload: {:02X?}. Treating as home position.",
                    payload.as_slice()
                );
                let pan_u16 =
                    ((payload.as_slice()[0] as u16) << 8) | (payload.as_slice()[1] as u16);
                let tilt_u16 =
                    ((payload.as_slice()[2] as u16) << 8) | (payload.as_slice()[3] as u16);
                let (pan, tilt) = coordinate_system.convert_from_camera_coords(pan_u16, tilt_u16);

                Ok(Response::Inquiry(InquiryData::PanTiltPosition {
                    pan: i32::from(pan),
                    tilt: i32::from(tilt),
                }))
            } else {
                tracing::debug!(
                    "PanTiltPosition: Payload length {} doesn't match standard VISCA pan/tilt format (expected 8 or 4 bytes)",
                    payload.len()
                );
                Err(Error::DecoderNotFound {
                    inquiry_kind: InquiryKind::PanTiltPosition,
                    payload_hex: format_payload_hex(payload.as_slice()),
                })
            }
        }
        PanTiltWireCodec::SonyBrc300 => {
            if coordinate_system != crate::capabilities::CoordinateSystem::SignedCentered {
                return Err(Error::InvalidRequest(
                    "Sony BRC-300 pan/tilt framing requires signed-centered coordinates".into(),
                ));
            }
            if payload.len() != 9 {
                tracing::debug!(
                    "PanTiltPosition: Payload length {} doesn't match Sony BRC-300 pan/tilt format (expected 9 nibbles)",
                    payload.len()
                );
                return Err(Error::DecoderNotFound {
                    inquiry_kind: InquiryKind::PanTiltPosition,
                    payload_hex: format_payload_hex(payload.as_slice()),
                });
            }
            let nibbles = Nibbles::<9>::try_from(payload)?;
            Ok(Response::Inquiry(InquiryData::PanTiltPosition {
                pan: nibbles.i20_penta(0),
                tilt: i32::from(nibbles.i16_quad(5)),
            }))
        }
    }
}

#[cfg(test)]
mod wire_decoder_regression_tests {
    use super::*;
    use crate::command::parse_inquiry_payload;

    #[test]
    fn autofocus_sensitivity_uses_documented_wire_values() {
        for (wire, expected) in [
            (0x01, AutoFocusSensitivity::High),
            (0x02, AutoFocusSensitivity::Normal),
            (0x03, AutoFocusSensitivity::Low),
        ] {
            let response = parse_inquiry_payload(&[wire], &InquiryKind::AutoFocusSensitivity);
            assert!(
                matches!(
                    &response,
                    Ok(Response::Inquiry(InquiryData::AutoFocusSensitivity { sensitivity }))
                        if *sensitivity == expected
                ),
                "documented AF sensitivity value {wire:#04X} must decode as {expected:?}, got {response:?}"
            );
        }
    }

    #[test]
    fn autofocus_sensitivity_rejects_unknown_or_trailing_values() {
        for wire in [0x00, 0x04, 0xff] {
            assert!(matches!(
                parse_inquiry_payload(&[wire], &InquiryKind::AutoFocusSensitivity),
                Err(Error::InvalidParameter {
                    parameter: "auto_focus_sensitivity",
                    ..
                })
            ));
        }

        assert!(matches!(
            parse_inquiry_payload(&[0x01, 0x00], &InquiryKind::AutoFocusSensitivity),
            Err(Error::InvalidResponseLength {
                expected: 1,
                actual: 2,
                ..
            })
        ));
    }

    #[test]
    fn gain_limit_requires_a_single_nibble() {
        assert!(matches!(
            parse_inquiry_payload(&[0x0f], &InquiryKind::GainLimit),
            Ok(Response::Inquiry(InquiryData::GainLimit { limit: 0x0f }))
        ));

        for wire in [0x10, 0x80, 0xff] {
            assert!(matches!(
                parse_inquiry_payload(&[wire], &InquiryKind::GainLimit),
                Err(Error::InvalidResponseFormat)
            ));
        }
    }

    #[test]
    fn image_flip_requires_one_known_combined_mode() {
        for (wire, horizontal, vertical) in [
            (0x00, false, false),
            (0x01, true, false),
            (0x02, false, true),
            (0x03, true, true),
        ] {
            let response = parse_inquiry_payload(&[wire], &InquiryKind::FlipState);
            assert!(
                matches!(
                    &response,
                    Ok(Response::Inquiry(InquiryData::FlipState {
                        horizontal: decoded_horizontal,
                        vertical: decoded_vertical,
                    })) if *decoded_horizontal == horizontal && *decoded_vertical == vertical
                ),
                "documented flip mode {wire:#04X} must decode as ({horizontal}, {vertical}), got {response:?}"
            );
        }

        assert!(matches!(
            parse_inquiry_payload(&[0x04], &InquiryKind::FlipState),
            Err(Error::InvalidParameter {
                parameter: "image_flip_mode",
                ..
            })
        ));
        assert!(matches!(
            parse_inquiry_payload(&[0x02, 0x00], &InquiryKind::FlipState),
            Err(Error::InvalidResponseLength {
                expected: 1,
                actual: 2,
                ..
            })
        ));
    }

    #[test]
    fn packed_tally_status_remains_an_explicit_two_byte_response() {
        assert!(matches!(
            parse_inquiry_payload(&[0x03, 0x02], &InquiryKind::TallyStatus),
            Ok(Response::Inquiry(InquiryData::TallyStatus {
                red_on: true,
                green_on: false,
            }))
        ));
        assert!(matches!(
            parse_inquiry_payload(&[0x03, 0x02, 0x00], &InquiryKind::TallyStatus),
            Err(Error::InvalidResponseLength {
                expected: 2,
                actual: 3,
                ..
            })
        ));
    }
}
