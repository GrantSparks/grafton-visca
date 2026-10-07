//! Built-in inquiry support generated from crate-local metadata.
//!
//! Built-in inquiries are intentionally not implemented through the public
//! `ViscaInquiry` derive. The single [`define_builtin_inquiries!`] table below
//! is the source of truth for the response discriminants, the decoded response
//! data, the decoder dispatch, the zero-sized inquiry commands and their
//! request bodies, the typed query conversions, and the invariant metadata.
//!
//! The table has one row grammar, matched once:
//!
//! - `queryable` rows declare a response kind, its data and decoder, and the
//!   one or more commands that query it;
//! - `decode_only` rows declare a response kind that no built-in command
//!   queries, with the provenance that explains why.
//!
//! The profile gate of each typed inquiry is not part of this table: it is the
//! gate of the inquiry's noun-table row, which `crate::command::surface`
//! projects into [`BuiltinInquiryGate`].

use super::exposure::{AntiFlickerMode, ExposureMode};
use super::focus::{AutoFocusSensitivity, FocusMode, FocusRange, FocusZone};
use super::image::{ImageFlipMode, NoiseReduction2DMode, SharpnessMode};
use super::pan_tilt::PanTiltFraming;
use super::resolution::{NdFilterPosition, PictureEffectMode};
use super::response::{BoolConvention, Nibbles, Nibbles4Or8, Payload, Response};
use super::system::{MotionSyncMode, MotionSyncPreset};
use super::white_balance::{AutoWhiteBalanceSensitivity, WhiteBalanceMode};
use crate::command::bytes::constants::{
    color, exposure, focus, gain, image, menu, power, streaming, tally, white_balance, zoom,
    INQUIRY,
};
use crate::command::bytes::FrameWriter;
use crate::command::surface::{BuiltinInquiryGate, RowGate};
use crate::command::{encode::WireEncode, FlipState, ResponseParser};
use crate::types::{
    BlueTuning, BroadcastDomain, DefogLevel, ExposureCompensationLevel,
    ExposureCompensationPosition, NdFilterPreset, RedTuning,
};
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
    /// Address-free request body, without the terminator.
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

/// The typed response of a generated inquiry: the conversion's type, or the
/// untyped [`Response`] for `typed: none`.
macro_rules! builtin_inquiry_response {
    (none) => {
        crate::command::Response
    };
    (($response_ty:ty, $data_pattern:tt => $conversion:expr)) => {
        $response_ty
    };
}

/// Implements [`ResponseParser`] for a typed generated inquiry.
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

/// Lifts a decoded [`Response`] into a generated inquiry's typed response.
macro_rules! builtin_inquiry_lift {
    (none, $struct:ident, $response:expr) => {
        Ok($response)
    };
    (($($typed:tt)*), $struct:ident, $response:expr) => {
        <$struct as ResponseParser>::from_response($response)
    };
}

/// The `typed: none` inquiry has no noun row and no gate; a typed inquiry's
/// [`BuiltinInquiryGate`] comes from its noun-table row.
macro_rules! builtin_inquiry_untyped_gate {
    (none, $struct:ident) => {
        impl BuiltinInquiryGate for $struct {
            const GATE: RowGate = RowGate::Always;
        }
    };
    (($($typed:tt)*), $struct:ident) => {};
}

/// Runtime admission for a generated inquiry under its noun row's gate.
///
/// The erased dynamic surface carries no compile-time bound, so it reproduces
/// at runtime the gate the static `<noun>().<inquiry>()` call resolves (#684):
/// a base-domain inquiry (`power().state()`, `zoom().position()`, ...) has no
/// `where` clause of its own and inherits its noun's base-domain marker, and a
/// typed one requires its marker's typed-support surface.
fn validate_builtin_inquiry_gate(
    profile: &crate::ProfileSpec,
    gate: RowGate,
    inquiry: &'static str,
    typed_inquiry: &'static str,
) -> Result<(), Error> {
    match gate {
        RowGate::Always => Ok(()),
        RowGate::Domain(domain) if domain.permits(profile) => Ok(()),
        RowGate::Domain(_) => Err(Error::FeatureNotSupported { feature: inquiry }),
        RowGate::Typed(surface) => {
            validate_builtin_inquiry_surface(profile, surface, typed_inquiry)
        }
    }
}

fn validate_builtin_inquiry_surface(
    profile: &crate::ProfileSpec,
    surface: crate::capabilities::TypedSupportSurface,
    inquiry: &'static str,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    // These vendor/status surfaces require both the typed admission bit and
    // the underlying source-backed protocol fact. Keeping this narrow match
    // here lets the noun-table rows remain the single gate list.
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
        Err(Error::FeatureNotSupported {
            feature: if surface == crate::capabilities::TypedSupportSurface::ExposureMode {
                crate::command::exposure::SHARED_EXPOSURE_MODE_FEATURE
            } else {
                inquiry
            },
        })
    }
}

/// The facts every generated inquiry command shares with the generic decode
/// path below.
trait GeneratedInquiry: crate::Inquiry {
    /// The response kind the command queries.
    const KIND: InquiryKind;
    /// The address-free request body, without the terminator.
    const BODY: &'static [u8];

    /// Lifts the decoded response into the typed response.
    fn lift(response: Response) -> Result<Self::Response, Error>;
}

/// Decodes a payload as `C`'s response with the profile-less pan/tilt
/// framing.
fn decode_payload<C: GeneratedInquiry>(payload: &[u8]) -> Result<C::Response, Error> {
    C::lift(dispatch(C::KIND, Payload::new(payload))?)
}

/// Decodes a payload as `C`'s response with a profile's pan/tilt framing.
///
/// `framing` is `None` only for a profile without a pan/tilt coordinate
/// conversion, which cannot decode a position reply.
fn decode_payload_with_framing<C: GeneratedInquiry>(
    framing: &Option<PanTiltFraming>,
    payload: &[u8],
) -> Result<C::Response, Error> {
    let framing = framing.ok_or(Error::FeatureNotSupported {
        feature: "pan/tilt coordinate conversion",
    })?;
    C::lift(dispatch_with_framing(
        C::KIND,
        Payload::new(payload),
        framing,
    )?)
}

/// The decoder a generated inquiry uses for a validated profile.
///
/// Only the pan/tilt position reply depends on the profile, so every other
/// inquiry keeps the allocation-free profile-less decoder.
fn profile_decoder<C>(profile: &crate::ProfileSpec) -> crate::ResponseDecoder<C::Response>
where
    C: GeneratedInquiry,
    C::Response: 'static,
{
    if C::KIND == InquiryKind::PanTiltPosition {
        let framing = profile
            .pan_tilt_coordinates()
            .map(PanTiltFraming::for_conversion);
        crate::ResponseDecoder::with_context(framing, decode_payload_with_framing::<C>)
    } else {
        crate::ResponseDecoder::from_fn(decode_payload::<C>)
    }
}

/// Decodes an inquiry response, using `framing` for the pan/tilt position
/// reply and the profile-independent decoder for every other kind.
pub(crate) fn dispatch_with_framing(
    kind: InquiryKind,
    payload: Payload<'_>,
    framing: PanTiltFraming,
) -> Result<Response, Error> {
    match kind {
        InquiryKind::PanTiltPosition => {
            decode_pan_tilt_position(payload, framing).map(Response::Inquiry)
        }
        _ => dispatch(kind, payload),
    }
}

/// Generates every built-in inquiry item from the one table below.
macro_rules! define_builtin_inquiries {
    (
        queryable {
            $(
                $(#[$kind_meta:meta])*
                $kind:ident $data:tt => |$payload:ident| $decode:expr;
                commands [
                    $(
                        $(#[$meta:meta])*
                        $struct:ident {
                            bytes: $bytes:expr,
                            typed: $typed:tt,
                            query: $query:expr,
                            vendor_specific: $vendor_specific:expr,
                            rationale: $rationale:expr $(,)?
                        }
                    )+
                ]
            )*
        }
        decode_only {
            $(
                $(#[$decode_meta:meta])*
                $decode_kind:ident $decode_data:tt => |$decode_payload:ident| $decode_body:expr;
                vendor_specific: $decode_vendor_specific:expr,
                rationale: $decode_rationale:expr;
            )*
        }
    ) => {
        /// Type of expected response for inquiry commands.
        ///
        /// Used to indicate what kind of data parser should expect in the response
        /// payload.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[non_exhaustive]
        pub enum InquiryKind {
            $(
                $(#[$kind_meta])*
                $kind,
            )*
            $(
                $(#[$decode_meta])*
                $decode_kind,
            )*
        }

        /// Response data from VISCA inquiry commands.
        ///
        /// Each variant represents a different type of inquiry response with its
        /// associated data.  These are returned wrapped in
        /// [`Response::Inquiry(...)`](Response::Inquiry).
        #[derive(Debug, Copy, Clone)]
        #[non_exhaustive]
        pub enum InquiryData {
            $(
                $(#[$kind_meta])*
                $kind $data,
            )*
            $(
                $(#[$decode_meta])*
                $decode_kind $decode_data,
            )*
        }

        /// Decode an inquiry response with the profile-independent decoder
        /// of its kind.
        ///
        /// Generated from the built-in inquiry table, so every
        /// [`InquiryKind`] variant has exactly one decoder arm.
        pub(crate) fn dispatch(kind: InquiryKind, payload: Payload<'_>) -> Result<Response, Error> {
            let data = match kind {
                $(
                    InquiryKind::$kind => {
                        let $payload = payload;
                        $decode
                    }
                )*
                $(
                    InquiryKind::$decode_kind => {
                        let $decode_payload = payload;
                        $decode_body
                    }
                )*
            };
            data.map(Response::Inquiry)
        }

        /// Generated inquiry-kind inventory used by the downstream fuzz
        /// harness. Keeping it beside the defining table makes newly added
        /// decoders enter the fuzz matrix automatically.
        #[cfg(all(feature = "test-utils", any(test, feature = "blocking")))]
        pub(crate) const FUZZ_INQUIRY_KINDS: &[InquiryKind] = &[
            $(InquiryKind::$kind,)*
            $(InquiryKind::$decode_kind,)*
        ];

        /// Re-exports of the generated inquiry commands for `crate::command`.
        pub(crate) mod commands {
            pub use super::{$($($struct,)+)*};
        }

        $($(
            $(#[$meta])*
            #[derive(Debug, Copy, Clone, Default)]
            pub struct $struct;

            impl GeneratedInquiry for $struct {
                const KIND: InquiryKind = InquiryKind::$kind;
                const BODY: &'static [u8] = &$bytes;

                fn lift(response: Response) -> Result<Self::Response, Error> {
                    builtin_inquiry_lift!($typed, $struct, response)
                }
            }

            impl WireEncode for $struct {
                fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                    FrameWriter::new(camera_id, buffer).bytes(Self::BODY).finish()
                }
            }

            impl_builtin_response_parser!($typed, $struct, $kind);
            builtin_inquiry_untyped_gate!($typed, $struct);

            impl crate::Request for $struct {
                type Class = crate::request::Inquiry;

                const MAX_SIZE: usize = crate::command::bytes::frame_len(Self::BODY);
                const TIMEOUT_CLASS: crate::TimeoutClass = crate::TimeoutClass::Inquiry;
                const RETRY_CLASS: crate::RetryClass = crate::RetryClass::Inquiry;
                const CONTROL_CLASS: crate::ControlClass = crate::ControlClass::Normal;

                fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                    #[cfg(test)]
                    increment_generated_inquiry_write_count();
                    WireEncode::write_into(self, camera_id, buffer)
                }

                fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> crate::Result<()> {
                    validate_builtin_inquiry_gate(
                        profile,
                        <Self as BuiltinInquiryGate>::GATE,
                        concat!("inquiry ", stringify!($struct)),
                        concat!("typed inquiry ", stringify!($struct)),
                    )
                }
            }

            impl crate::Inquiry for $struct {
                type Response = builtin_inquiry_response!($typed);

                fn route(&self) -> crate::InquiryRoute {
                    crate::InquiryRoute::custom(InquiryKind::$kind as u16 + 1)
                }

                fn decoder(&self) -> crate::ResponseDecoder<Self::Response> {
                    crate::ResponseDecoder::from_fn(decode_payload::<Self>)
                }

                fn decoder_for_profile(
                    &self,
                    profile: &crate::ProfileSpec,
                ) -> crate::ResponseDecoder<Self::Response> {
                    profile_decoder::<Self>(profile)
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

            #[cfg(test)]
            impl BuiltinInquiryCommandMarker for $struct {
                const METADATA: BuiltinInquiryCommand = BuiltinInquiryCommand {
                    name: stringify!($struct),
                    bytes: <$struct as GeneratedInquiry>::BODY,
                };
            }
        )+)*

        /// Every generated command's name, request body and encoded frame.
        #[cfg(test)]
        fn encode_each_generated_inquiry(
            camera_id: CameraId,
        ) -> Vec<(&'static str, &'static [u8], Result<Vec<u8>, Error>)> {
            let encode = |command: &dyn WireEncode| {
                let mut buffer = [0u8; 32];
                command
                    .write_into(camera_id, &mut buffer)
                    .map(|len| buffer[..len].to_vec())
            };
            vec![
                $($(
                    (
                        stringify!($struct),
                        <$struct as GeneratedInquiry>::BODY,
                        encode(&$struct),
                    ),
                )+)*
            ]
        }

        /// Iterable built-in inquiry metadata for invariant tests and internal
        /// consistency checks.
        #[cfg(test)]
        pub(crate) const BUILTIN_INQUIRIES: &[BuiltinInquiryMetadata] = &[
            $($(
                BuiltinInquiryMetadata {
                    name: stringify!($struct),
                    command: Some(<$struct as BuiltinInquiryCommandMarker>::METADATA),
                    kind: InquiryKind::$kind,
                    bytes: Some(<$struct as GeneratedInquiry>::BODY),
                    query: $query,
                    typed: builtin_inquiry_is_typed!($typed),
                    vendor_specific: $vendor_specific,
                    rationale: $rationale,
                },
            )+)*
            $(
                BuiltinInquiryMetadata {
                    name: stringify!($decode_kind),
                    command: None,
                    kind: InquiryKind::$decode_kind,
                    bytes: None,
                    query: BuiltinInquiryQuery::DecodeOnly,
                    typed: false,
                    vendor_specific: $decode_vendor_specific,
                    rationale: $decode_rationale,
                },
            )*
        ];
    };
}

define_builtin_inquiries! {
    queryable {
        /// The current power state of the camera.
        Power {
            /// Whether the camera is powered on.
            on: bool,
        } => |payload| Ok(InquiryData::Power {
            on: payload.parse_bool("power_status", BoolConvention::OnIs02)?,
        });
        commands [
            /// Inquiry command to get the current power state of the camera.
            PowerInquiry {
                bytes: power::STATE_INQUIRY,
                typed: (bool, { on } => Ok(on)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The camera version information.
        Version {
            /// Camera version information.
            info: crate::command::VersionInfo,
        } => |payload| {
            require_len(&payload, 7)?;
            let reply = payload.as_slice();
            Ok(InquiryData::Version {
                info: crate::command::VersionInfo {
                    vendor: u16::from_be_bytes([reply[0], reply[1]]),
                    model: u16::from_be_bytes([reply[2], reply[3]]),
                    rom_version: u32::from(u16::from_be_bytes([reply[4], reply[5]])),
                    max_socket: reply[6],
                },
            })
        };
        commands [
            /// Inquiry command to get the camera version information.
            VersionInquiry {
                bytes: [INQUIRY, 0x00, 0x02],
                typed: (crate::command::VersionInfo, { info } => Ok(info)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current pan/tilt position.
        PanTiltPosition {
            /// Current pan position.
            pan: i32,
            /// Current tilt position.
            tilt: i32,
        } => |payload| decode_pan_tilt_position(payload, PanTiltFraming::STANDARD);
        commands [
            /// Inquiry command to get the current pan/tilt position.
            PanTiltPositionInquiry {
                bytes: [INQUIRY, 0x06, 0x12],
                typed: (
                    crate::camera::PanTiltPosition,
                    { pan, tilt } => Ok(crate::camera::PanTiltPosition { pan, tilt })
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current zoom position.
        ZoomPosition {
            /// Zoom position value.
            position: u16,
        } => |payload| {
            let nibbles = Nibbles4Or8::try_from(payload)?;
            if matches!(nibbles, Nibbles4Or8::N8(_)) {
                tracing::warn!(
                    "ZoomPosition: Received extended format (8 nibbles). Using first 4 nibbles (16-bit) per VISCA spec."
                );
            }
            Ok(InquiryData::ZoomPosition {
                position: nibbles.first_u16(),
            })
        };
        commands [
            /// Inquiry command to get the current zoom position.
            ZoomPositionInquiry {
                bytes: zoom::POSITION_INQUIRY,
                typed: (
                    crate::types::ZoomPosition,
                    { position } => crate::types::ZoomPosition::new(position)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current focus position.
        FocusPosition {
            /// Focus position value.
            position: u16,
        } => |payload| Ok(InquiryData::FocusPosition {
            position: Nibbles::<4>::try_from(payload)?.u16_quad(0),
        });
        commands [
            /// Inquiry command to get the current focus position.
            FocusPositionInquiry {
                bytes: focus::POSITION_INQUIRY,
                typed: (
                    crate::types::FocusPosition,
                    { position } => Ok(crate::types::FocusPosition::new(position))
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current exposure mode setting.
        ExposureMode {
            /// Current exposure mode (Auto, Manual, Shutter, Iris, or Bright).
            mode: ExposureMode,
        } => |payload| Ok(InquiryData::ExposureMode {
            mode: decode_enum(&payload)?,
        });
        commands [
            /// Inquiry command to get the current exposure mode setting.
            ExposureModeInquiry {
                bytes: exposure::MODE_INQUIRY,
                typed: (ExposureMode, { mode } => Ok(mode)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current exposure compensation value.
        ExposureCompensation {
            /// Exposure compensation value (-7 to +7).
            value: i8,
        } => |payload| Ok(InquiryData::ExposureCompensation {
            value: ExposureCompensationLevel::from_protocol_value(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)?.value(),
        });
        commands [
            /// Inquiry command to get the current exposure compensation value.
            ExposureCompensationInquiry {
                bytes: exposure::COMPENSATION_INQUIRY,
                typed: (
                    ExposureCompensationLevel,
                    { value } => ExposureCompensationLevel::new(value)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: Some("Shares CAM_ExpCompPosInq bytes with ExposureCompensationPosition; this entry exposes the legacy signed EV level interpretation."),
            }
        ]

        /// The exposure compensation mode on/off status.
        ExposureCompensationMode {
            /// Whether exposure compensation is enabled.
            on: bool,
        } => |payload| Ok(InquiryData::ExposureCompensationMode {
            on: payload.parse_bool("exposure_compensation_mode", BoolConvention::OnIs02)?,
        });
        commands [
            /// Inquiry command to get the exposure compensation mode on/off status.
            ExposureCompensationModeInquiry {
                bytes: exposure::COMPENSATION_SWITCH_INQUIRY,
                typed: (bool, { on } => Ok(on)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current iris position value.
        Iris {
            /// Iris position in the built-in profiles' `0x00..=0x1E`
            /// wire union; each selected profile still validates its own
            /// documented range.
            position: u8,
        } => |payload| Ok(InquiryData::Iris {
            position: Nibbles::<4>::try_from(payload)?.zero_extended_u8()?,
        });
        commands [
            /// Inquiry command to get the current iris position value.
            IrisInquiry {
                bytes: exposure::IRIS_INQUIRY,
                typed: (
                    crate::types::IrisLevel,
                    { position } => crate::types::IrisLevel::new(position)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current shutter speed setting.
        Shutter {
            /// Profile-specific shutter code; the profile's shutter table maps it to an exposure time.
            position: u8,
        } => |payload| Ok(InquiryData::Shutter {
            position: Nibbles::<4>::try_from(payload)?.zero_extended_u8()?,
        });
        commands [
            /// Inquiry command to get the current shutter speed setting.
            ShutterInquiry {
                bytes: exposure::SHUTTER_INQUIRY,
                typed: (
                    crate::types::ShutterSpeed,
                    { position } => Ok(crate::types::ShutterSpeed::new(position))
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current brightness adjustment value.
        Brightness {
            /// Bright position `pq` (`00 00 0p 0q`); every byte is a position.
            position: u8,
        } => |payload| Ok(InquiryData::Brightness {
            position: Nibbles::<4>::try_from(payload)?.zero_extended_u8()?,
        });
        commands [
            /// Inquiry command to get the current brightness adjustment value.
            BrightnessInquiry {
                bytes: exposure::BRIGHT_INQUIRY,
                typed: (
                    crate::types::BrightnessLevel,
                    { position } => Ok(crate::types::BrightnessLevel::new(position))
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current white balance mode.
        WhiteBalanceMode {
            /// Current white balance mode.
            mode: WhiteBalanceMode,
        } => |payload| Ok(InquiryData::WhiteBalanceMode {
            mode: decode_enum(&payload)?,
        });
        commands [
            /// Inquiry command to get the current white balance mode.
            WhiteBalanceModeInquiry {
                bytes: white_balance::MODE_INQUIRY,
                typed: (WhiteBalanceMode, { mode } => Ok(mode)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The color temperature wire step.
        ///
        /// Decoded with the one-byte `y0 50 pq FF` reply layout the PTZOptics G2
        /// bench returns (#498); see `ColorTemperatureInquiry`.
        ColorTemperature {
            /// Color temperature wire step; `ColorTemp::to_kelvin` converts it to Kelvin.
            temperature: u16,
        } => |payload| Ok(InquiryData::ColorTemperature {
            temperature: u16::from(decode_byte(&payload)?),
        });
        commands [
            /// Inquiry command to get the current color temperature value.
            ColorTemperatureInquiry {
                bytes: color::TEMPERATURE_INQUIRY,
                typed: (
                    crate::types::ColorTemp,
                    { temperature } => crate::types::ColorTemp::new(temperature)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current red gain value.
        RedChannel {
            /// Red channel absolute gain value.
            gain: u8,
        } => |payload| Ok(InquiryData::RedChannel {
            gain: Nibbles::<4>::try_from(payload)?.zero_extended_u8()?,
        });
        commands [
            /// Inquiry command to get the current red gain value.
            RedGainInquiry {
                bytes: color::RED_GAIN_INQUIRY,
                typed: (
                    crate::types::RedChannel,
                    { gain } => crate::types::RedChannel::new(gain)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: Some("CAM_RGainInq shares bytes with RedTuningInquiry; this entry exposes the absolute red-channel gain interpretation."),
            }
        ]

        /// The current blue gain value.
        BlueChannel {
            /// Blue channel absolute gain value.
            gain: u8,
        } => |payload| Ok(InquiryData::BlueChannel {
            gain: Nibbles::<4>::try_from(payload)?.zero_extended_u8()?,
        });
        commands [
            /// Inquiry command to get the current blue gain value.
            BlueGainInquiry {
                bytes: color::BLUE_GAIN_INQUIRY,
                typed: (
                    crate::types::BlueChannel,
                    { gain } => crate::types::BlueChannel::new(gain)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: Some("CAM_BGainInq shares bytes with BlueTuningInquiry; this entry exposes the absolute blue-channel gain interpretation."),
            }
        ]

        /// The current sharpness mode setting.
        SharpnessMode {
            /// Current sharpness mode.
            mode: SharpnessMode,
        } => |payload| Ok(InquiryData::SharpnessMode {
            mode: decode_enum(&payload)?,
        });
        commands [
            /// Inquiry command to get the current sharpness mode setting.
            SharpnessModeInquiry {
                bytes: image::SHARPNESS_MODE_INQUIRY,
                typed: (SharpnessMode, { mode } => Ok(mode)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current color saturation level.
        Saturation {
            /// Saturation level (0x0=60% to 0xE=200%).
            level: u8,
        } => |payload| Ok(InquiryData::Saturation {
            level: Nibbles::<4>::try_from(payload)?.zero_extended_nibble()?,
        });
        commands [
            /// Inquiry command to get the current color saturation level.
            SaturationInquiry {
                bytes: color::SATURATION_INQUIRY,
                typed: (
                    crate::types::SaturationLevel,
                    { level } => crate::types::SaturationLevel::new(level)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current hue adjustment value.
        Hue {
            /// Hue value (0x0=0 to 0xE=14).
            hue: u8,
        } => |payload| Ok(InquiryData::Hue {
            hue: Nibbles::<4>::try_from(payload)?.zero_extended_nibble()?,
        });
        commands [
            /// Inquiry command to get the current hue adjustment value.
            HueInquiry {
                bytes: color::HUE_INQUIRY,
                typed: (
                    crate::types::HueLevel,
                    { hue } => crate::types::HueLevel::new(hue)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current gain value.
        Gain {
            /// Gain position `pq` (`00 00 0p 0q`, both digits kept); `GainLevel`
            /// admits `0x00..=0x0F` and each profile its own gain range.
            gain: u8,
        } => |payload| Ok(InquiryData::Gain {
            gain: Nibbles::<4>::try_from(payload)?.zero_extended_u8()?,
        });
        commands [
            /// Inquiry command to get the current gain value.
            GainInquiry {
                bytes: gain::INQUIRY,
                typed: (
                    crate::types::GainLevel,
                    { gain } => crate::types::GainLevel::new(gain)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current gain limit setting.
        GainLimit {
            /// Maximum gain limit (0x0=0 to 0xF=15).
            limit: u8,
        } => |payload| Ok(InquiryData::GainLimit {
            limit: Nibbles::<1>::try_from(payload)?.byte(0),
        });
        commands [
            /// Inquiry command to get the current gain limit setting.
            GainLimitInquiry {
                bytes: gain::LIMIT_INQUIRY,
                typed: (
                    crate::types::GainLimit,
                    { limit } => crate::types::GainLimit::new(limit)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The backlight compensation mode.
        Backlight {
            /// Whether backlight compensation is enabled.
            status: bool,
        } => |payload| Ok(InquiryData::Backlight {
            status: payload.parse_bool("backlight_status", BoolConvention::OnIs02)?,
        });
        commands [
            /// Inquiry command to get the backlight compensation mode.
            BacklightInquiry {
                bytes: image::BACKLIGHT_INQUIRY,
                typed: (bool, { status } => Ok(status)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The combined horizontal and vertical flip state.
        FlipState {
            /// Combined horizontal and vertical flip state.
            state: FlipState,
        } => |payload| Ok(InquiryData::FlipState {
            state: FlipState::from(decode_enum::<ImageFlipMode>(&payload)?),
        });
        commands [
            /// Inquiry command to get the combined flip state.
            ImageFlipInquiry {
                bytes: image::FLIP_COMBINED_INQUIRY,
                typed: (FlipState, { state } => Ok(state)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("PTZOptics CAM_FlipInq returns combined horizontal/vertical flip state and intentionally shares bytes with FlipStateInquiry."),
            }
            /// Inquiry command to get the current flip mode.
            FlipStateInquiry {
                bytes: image::FLIP_COMBINED_INQUIRY,
                typed: (FlipState, { state } => Ok(state)),
                query: BuiltinInquiryQuery::Alias {
                    canonical: <ImageFlipInquiry as BuiltinInquiryCommandMarker>::METADATA,
                },
                vendor_specific: true,
                rationale: Some("Public flip-mode accessor intentionally aliases ImageFlipInquiry because both expose CAM_FlipInq."),
            }
        ]

        /// The 2D noise reduction mode.
        NoiseReduction2DMode {
            /// Current 2D noise reduction mode.
            mode: NoiseReduction2DMode,
        } => |payload| Ok(InquiryData::NoiseReduction2DMode {
            mode: decode_enum(&payload)?,
        });
        commands [
            /// Inquiry command to get the 2D noise reduction mode.
            NoiseReduction2DModeInquiry {
                bytes: image::NOISE_REDUCTION_2D_MODE_INQUIRY,
                typed: (NoiseReduction2DMode, { mode } => Ok(mode)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: Some("The PTZOptics inquiry table identifies 04 50 as the 2D noise-reduction Auto/Manual mode register."),
            }
        ]

        /// The 2D noise reduction level.
        NoiseReduction2D {
            /// 2D noise reduction level.
            level: u8,
        } => |payload| Ok(InquiryData::NoiseReduction2D {
            level: decode_byte(&payload)?,
        });
        commands [
            /// Inquiry command to get the 2D noise reduction level.
            NoiseReduction2DInquiry {
                bytes: image::NOISE_REDUCTION_2D_INQUIRY,
                typed: (
                    crate::types::NoiseReduction2DLevel,
                    { level } => crate::types::NoiseReduction2DLevel::new(level)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: Some("The PTZOptics inquiry table identifies 04 53 as the 2D noise-reduction level register."),
            }
        ]

        /// The 3D noise reduction level.
        NoiseReduction3D {
            /// 3D noise reduction level.
            level: u8,
        } => |payload| Ok(InquiryData::NoiseReduction3D {
            level: decode_byte(&payload)?,
        });
        commands [
            /// Inquiry command to get the 3D noise reduction level.
            ///
            /// Every decoder accepts the public value type's `0..=8` domain. This
            /// keeps well-formed readback values symmetric with the separately
            /// documented `04 54` setter domain across PTZOptics profiles (#717).
            NoiseReduction3DInquiry {
                bytes: image::NOISE_REDUCTION_3D_INQUIRY,
                typed: (
                    crate::types::NoiseReduction3DLevel,
                    { level } => crate::types::NoiseReduction3DLevel::new(level)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: Some("The PTZOptics inquiry table identifies 04 54 as the 3D noise-reduction level register."),
            }
        ]

        /// The dynamic range mode/level.
        DynamicRange {
            /// Dynamic range level (0x0=0 to 0x8=8).
            level: u8,
        } => |payload| Ok(InquiryData::DynamicRange {
            level: decode_byte(&payload)?,
        });
        commands [
            /// Inquiry command to get the dynamic range mode/level.
            DynamicRangeInquiry {
                bytes: exposure::DYNAMIC_RANGE_INQUIRY,
                typed: (
                    crate::types::DynamicRangeLevel,
                    { level } => crate::types::DynamicRangeLevel::new(level)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]

        /// The current focus zone selection.
        FocusZone {
            /// Current focus zone setting.
            zone: FocusZone,
        } => |payload| Ok(InquiryData::FocusZone {
            zone: decode_enum(&payload)?,
        });
        commands [
            /// Inquiry command to get the current focus zone selection.
            FocusZoneInquiry {
                bytes: focus::ZONE_INQUIRY,
                typed: (FocusZone, { zone } => Ok(zone)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The auto-focus sensitivity setting.
        AutoFocusSensitivity {
            /// Current auto-focus sensitivity setting.
            sensitivity: AutoFocusSensitivity,
        } => |payload| Ok(InquiryData::AutoFocusSensitivity {
            sensitivity: decode_enum(&payload)?,
        });
        commands [
            /// Inquiry command to get the auto-focus sensitivity setting.
            AutoFocusSensitivityInquiry {
                bytes: focus::AF_SENSITIVITY_INQUIRY,
                typed: (
                    AutoFocusSensitivity,
                    { sensitivity } => Ok(sensitivity)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The focus near limit position.
        FocusNearLimit {
            /// Near limit position value.
            position: u16,
        } => |payload| Ok(InquiryData::FocusNearLimit {
            position: Nibbles::<4>::try_from(payload)?.u16_quad(0),
        });
        commands [
            /// Inquiry command to get the focus near limit position.
            FocusNearLimitInquiry {
                bytes: focus::NEAR_LIMIT_INQUIRY,
                typed: (
                    crate::types::FocusPosition,
                    { position } => Ok(crate::types::FocusPosition::new(position))
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current focus mode.
        FocusMode {
            /// Current focus mode (Auto or Manual).
            mode: FocusMode,
        } => |payload| Ok(InquiryData::FocusMode {
            mode: decode_enum(&payload)?,
        });
        commands [
            /// Inquiry command to get the current focus mode.
            FocusModeInquiry {
                bytes: focus::MODE_INQUIRY,
                typed: (FocusMode, { mode } => Ok(mode)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The menu open/close status.
        MenuOpenClose {
            /// Whether the camera menu is open.
            is_open: bool,
        } => |payload| Ok(InquiryData::MenuOpenClose {
            is_open: payload.parse_bool("menu_status", BoolConvention::OnIs02)?,
        });
        commands [
            /// Inquiry command to get the menu open/close status.
            MenuOpenCloseInquiry {
                bytes: menu::MENU_INQUIRY,
                typed: (bool, { is_open } => Ok(is_open)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("PTZOptics menu inquiries use category 0x06 to match menu command bytes."),
            }
        ]

        /// The combined tally light status.
        TallyStatus {
            /// Red and green tally light state.
            state: crate::command::TallyStatusState,
        } => |payload| {
            require_len(&payload, 2)?;
            let reply = payload.as_slice();
            Ok(InquiryData::TallyStatus {
                state: crate::command::TallyStatusState {
                    red_on: Payload::new(&reply[..1])
                        .parse_bool("tally_red_status", BoolConvention::OnIs03)?,
                    green_on: Payload::new(&reply[1..])
                        .parse_bool("tally_green_status", BoolConvention::OnIs03)?,
                },
            })
        };
        commands [
            /// Inquiry command to get combined tally light status.
            TallyStatusInquiry {
                bytes: [INQUIRY, 0x04, 0xA8],
                typed: (crate::command::TallyStatusState, { state } => Ok(state)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("PTZOptics extension returning packed red and green tally states."),
            }
        ]

        /// The night/day mode status.
        NightDayMode {
            /// Whether the camera is in night mode.
            is_night: bool,
        } => |payload| Ok(InquiryData::NightDayMode {
            is_night: payload.parse_bool("night_day_mode", BoolConvention::OnIs03)?,
        });
        commands [
            /// Inquiry command to get the night/day mode status.
            NightDayModeInquiry {
                bytes: [INQUIRY, 0x04, 0x60],
                typed: (bool, { is_night } => Ok(is_night)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The nD filter position.
        NdFilter {
            /// Current ND filter position (Clear, 1/4, 1/8, 1/16, etc.).
            position: NdFilterPosition,
        } => |payload| Ok(InquiryData::NdFilter {
            position: NdFilterPosition::from_byte(decode_byte(&payload)?),
        });
        commands [
            /// Inquiry command to get the ND filter position.
            NdFilterInquiry {
                bytes: [INQUIRY, 0x04, 0x64],
                typed: (NdFilterPosition, { position } => Ok(position)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current picture effect mode.
        PictureEffect {
            /// Current picture effect (Off or Black & White for built-in profiles).
            effect: PictureEffectMode,
        } => |payload| Ok(InquiryData::PictureEffect {
            effect: PictureEffectMode::from_byte(decode_byte(&payload)?),
        });
        commands [
            /// Inquiry command to get the current picture effect mode.
            PictureEffectInquiry {
                bytes: image::PICTURE_EFFECT_INQUIRY,
                typed: (PictureEffectMode, { effect } => Ok(effect)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The standby mode status.
        Standby {
            /// Whether the camera is in standby mode.
            in_standby: bool,
        } => |payload| Ok(InquiryData::Standby {
            in_standby: payload.parse_bool("standby_mode", BoolConvention::OnIs03)?,
        });
        commands [
            /// Inquiry command to get the standby mode status.
            StandbyInquiry {
                bytes: [INQUIRY, 0x04, 0x70],
                typed: (bool, { in_standby } => Ok(in_standby)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The focus range setting.
        FocusRange {
            /// Current focus range setting.
            range: FocusRange,
        } => |payload| Ok(InquiryData::FocusRange {
            range: decode_enum(&payload)?,
        });
        commands [
            /// Inquiry command to get the focus range setting.
            FocusRangeInquiry {
                bytes: [INQUIRY, 0x04, 0x2A],
                typed: (FocusRange, { range } => Ok(range)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The iris control mode.
        IrisControl {
            /// Whether iris is in auto mode.
            auto: bool,
        } => |payload| Ok(InquiryData::IrisControl {
            auto: payload.parse_bool("iris_control", BoolConvention::OnIs03)?,
        });
        commands [
            /// Inquiry command to get the iris control mode.
            IrisControlInquiry {
                bytes: [INQUIRY, 0x04, 0x2B],
                typed: (bool, { auto } => Ok(auto)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The defog mode status.
        DefogMode {
            /// Whether defog is enabled.
            enabled: bool,
        } => |payload| Ok(InquiryData::DefogMode {
            enabled: payload.parse_bool("defog_mode", BoolConvention::OnIs03)?,
        });
        commands [
            /// Inquiry command to get the defog mode status.
            DefogModeInquiry {
                bytes: [INQUIRY, 0x04, 0x37],
                typed: none,
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]

        /// The defog level.
        DefogLevel {
            /// Current defog strength level (0-5).
            level: DefogLevel,
        } => |payload| Ok(InquiryData::DefogLevel {
            level: DefogLevel::new(decode_byte(&payload)?)?,
        });
        commands [
            /// Inquiry command to get the defog level.
            DefogLevelInquiry {
                bytes: [INQUIRY, 0x04, 0xA0],
                typed: (DefogLevel, { level } => Ok(level)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]

        /// The digital PTZ mode status.
        DigitalPtz {
            /// Whether digital Ptz is enabled.
            enabled: bool,
        } => |payload| Ok(InquiryData::DigitalPtz {
            enabled: payload.parse_bool("digital_ptz", BoolConvention::OnIs03)?,
        });
        commands [
            /// Inquiry command to get the digital PTZ mode status.
            DigitalPtzInquiry {
                bytes: [INQUIRY, 0x04, 0x6B],
                typed: (bool, { enabled } => Ok(enabled)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]

        /// The auto white balance sensitivity setting.
        AutoWhiteBalanceSensitivity {
            /// Sensitivity level (Low, Normal, High).
            sensitivity: AutoWhiteBalanceSensitivity,
        } => |payload| Ok(InquiryData::AutoWhiteBalanceSensitivity {
            sensitivity: decode_enum(&payload)?,
        });
        commands [
            /// Inquiry command to get the auto white balance sensitivity setting.
            AutoWhiteBalanceSensitivityInquiry {
                bytes: white_balance::AWB_SENSITIVITY_INQUIRY,
                typed: (
                    AutoWhiteBalanceSensitivity,
                    { sensitivity } => Ok(sensitivity)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: Some("Shares bytes with TallyAutoAdjustInquiry on PTZOptics profiles; this entry interprets the register as AWB sensitivity."),
            }
        ]

        /// The exposure compensation position.
        ExposureCompensationPosition {
            /// Exposure compensation position value (high-resolution EV adjustment).
            position: ExposureCompensationPosition,
        } => |payload| Ok(InquiryData::ExposureCompensationPosition {
            position: ExposureCompensationPosition::new(Nibbles::<4>::try_from(payload)?.u16_quad(0)),
        });
        commands [
            /// Inquiry command to get the exposure compensation position.
            ExposureCompensationPositionInquiry {
                bytes: exposure::COMPENSATION_INQUIRY,
                typed: (
                    ExposureCompensationPosition,
                    { position } => Ok(position)
                ),
                query: BuiltinInquiryQuery::AlternateTypedInterpretation {
                    canonical: <ExposureCompensationInquiry as BuiltinInquiryCommandMarker>::METADATA,
                },
                vendor_specific: false,
                rationale: Some("Same wire query as ExposureCompensationInquiry, exposed as the high-resolution position newtype."),
            }
        ]

        /// The red channel tuning level.
        RedTuning {
            /// Red channel tuning level (-10 to +10).
            level: i8,
        } => |payload| Ok(InquiryData::RedTuning {
            level: RedTuning::from_protocol_value(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)?.value(),
        });
        commands [
            /// Inquiry command to get the red channel tuning level.
            RedTuningInquiry {
                bytes: color::RED_GAIN_INQUIRY,
                typed: (
                    RedTuning,
                    { level } => RedTuning::new(level)
                ),
                query: BuiltinInquiryQuery::AlternateTypedInterpretation {
                    canonical: <RedGainInquiry as BuiltinInquiryCommandMarker>::METADATA,
                },
                vendor_specific: false,
                rationale: Some("Same register as RedGainInquiry, interpreted as signed white-balance tuning."),
            }
        ]

        /// The blue channel tuning level.
        BlueTuning {
            /// Blue channel tuning level (-10 to +10).
            level: i8,
        } => |payload| Ok(InquiryData::BlueTuning {
            level: BlueTuning::from_protocol_value(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)?.value(),
        });
        commands [
            /// Inquiry command to get the blue channel tuning level.
            BlueTuningInquiry {
                bytes: color::BLUE_GAIN_INQUIRY,
                typed: (
                    BlueTuning,
                    { level } => BlueTuning::new(level)
                ),
                query: BuiltinInquiryQuery::AlternateTypedInterpretation {
                    canonical: <BlueGainInquiry as BuiltinInquiryCommandMarker>::METADATA,
                },
                vendor_specific: false,
                rationale: Some("Same register as BlueGainInquiry, interpreted as signed white-balance tuning."),
            }
        ]

        /// The current gamma curve setting.
        Gamma {
            /// Gamma curve setting (0=Standard, 1-4=different gamma curves).
            value: u8,
        } => |payload| Ok(InquiryData::Gamma {
            value: decode_byte(&payload)?,
        });
        commands [
            /// Inquiry command to get the current gamma curve setting.
            GammaInquiry {
                bytes: image::GAMMA_INQUIRY,
                typed: (
                    crate::types::GammaLevel,
                    { value } => crate::types::GammaLevel::new(value)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]

        /// The auto trace mode status.
        AutoTrace {
            /// Whether auto trace is enabled.
            enabled: bool,
        } => |payload| Ok(InquiryData::AutoTrace {
            enabled: payload.parse_bool("auto_trace", BoolConvention::OnIs03)?,
        });
        commands [
            /// Inquiry command to get the auto trace mode status.
            AutoTraceInquiry {
                bytes: [INQUIRY, 0x50, 0x09],
                typed: (bool, { enabled } => Ok(enabled)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("Uses the non-default auto-trace inquiry category 0x50."),
            }
        ]

        /// The focus unlock state.
        FocusUnlock {
            /// Whether focus is unlocked.
            unlocked: bool,
        } => |payload| Ok(InquiryData::FocusUnlock {
            unlocked: payload.parse_bool("focus_unlock", BoolConvention::OnIs03)?,
        });
        commands [
            /// Inquiry command to get the focus unlock state.
            FocusUnlockInquiry {
                bytes: [INQUIRY, 0x54, 0x08],
                typed: (bool, { unlocked } => Ok(unlocked)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("Uses the non-default focus-unlock inquiry category 0x54."),
            }
        ]

        /// The current sharpness position.
        SharpnessPosition {
            /// Current sharpness position value (`pq`).
            position: u8,
        } => |payload| Ok(InquiryData::SharpnessPosition {
            position: Nibbles::<4>::try_from(payload)?.zero_extended_u8()?,
        });
        commands [
            /// Inquiry command to get the current sharpness position.
            SharpnessPositionInquiry {
                bytes: image::SHARPNESS_INQUIRY,
                typed: (
                    crate::types::SharpnessLevel,
                    { position } => crate::types::SharpnessLevel::new(position)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The broadcast domain setting.
        BroadcastDomain (BroadcastDomain) => |payload| Ok(InquiryData::BroadcastDomain(BroadcastDomain::new(
            decode_byte(&payload)?,
        )?));
        commands [
            /// Inquiry command to get the broadcast domain setting.
            BroadcastDomainInquiry {
                bytes: [INQUIRY, 0x04, 0x75],
                typed: (BroadcastDomain, (val) => Ok(val)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]

        /// The motion sync mode setting.
        MotionSyncMode {
            /// Current motion sync mode setting.
            mode: MotionSyncMode,
        } => |payload| Ok(InquiryData::MotionSyncMode {
            mode: decode_enum(&payload)?,
        });
        commands [
            /// Inquiry command to get the motion sync mode setting.
            MotionSyncModeInquiry {
                bytes: [INQUIRY, 0x04, 0x56],
                typed: (MotionSyncMode, { mode } => Ok(mode)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]

        /// The motion sync speed setting.
        MotionSyncPreset {
            /// Current motion sync speed setting.
            speed: MotionSyncPreset,
        } => |payload| Ok(InquiryData::MotionSyncPreset {
            speed: decode_enum(&payload)?,
        });
        commands [
            /// Inquiry command to get the motion sync speed setting.
            MotionSyncPresetInquiry {
                bytes: [INQUIRY, 0x04, 0x57],
                typed: (MotionSyncPreset, { speed } => Ok(speed)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]

        /// The uSB audio state.
        UsbAudio {
            /// Whether USB audio is enabled.
            on: bool,
        } => |payload| Ok(InquiryData::UsbAudio {
            on: payload.parse_bool("usb_audio_status", BoolConvention::OnIs02)?,
        });
        commands [
            /// Inquiry command to get the USB audio state.
            UsbAudioInquiry {
                bytes: streaming::USB_AUDIO,
                typed: (bool, { on } => Ok(on)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]

        /// The two tone mode state.
        TwoToneMode {
            /// Whether two tone mode is enabled.
            on: bool,
        } => |payload| Ok(InquiryData::TwoToneMode {
            on: payload.parse_bool("two_tone_mode_status", BoolConvention::OnIs02)?,
        });
        commands [
            /// Inquiry command to get the two tone mode state.
            TwoToneModeInquiry {
                bytes: [INQUIRY, 0x04, 0x74],
                typed: (bool, { on } => Ok(on)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]

        /// The nD filter preset setting.
        NdFilterPreset {
            /// Current ND filter preset number.
            preset: NdFilterPreset,
        } => |payload| Ok(InquiryData::NdFilterPreset {
            preset: NdFilterPreset::new(decode_byte(&payload)?)?,
        });
        commands [
            /// Inquiry command to get the ND filter preset setting.
            NdFilterPresetInquiry {
                bytes: [INQUIRY, 0x7E, 0x01, 0x53],
                typed: (NdFilterPreset, { preset } => Ok(preset)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("Sony FR7 NDPresetInq; the legacy 09 04 66 register is picture-flip state, not an FR7 ND-filter preset."),
            }
        ]

        /// The digital mode state.
        Digital {
            /// Whether digital mode is enabled.
            on: bool,
        } => |payload| Ok(InquiryData::Digital {
            on: payload.parse_bool("digital_mode_status", BoolConvention::OnIs03)?,
        });
        commands [
            /// Inquiry command to get the digital mode state.
            DigitalInquiry {
                bytes: [INQUIRY, 0x04, 0x7B],
                typed: (bool, { on } => Ok(on)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]

        /// The tally auto adjust state.
        TallyAutoAdjust {
            /// Whether tally auto adjust is enabled.
            on: bool,
        } => |payload| Ok(InquiryData::TallyAutoAdjust {
            on: payload.parse_bool("tally_auto_adjust_status", BoolConvention::OnIs03)?,
        });
        commands [
            /// Inquiry command to get the tally auto adjust state.
            TallyAutoAdjustInquiry {
                bytes: white_balance::AWB_SENSITIVITY_INQUIRY,
                typed: (bool, { on } => Ok(on)),
                query: BuiltinInquiryQuery::AlternateTypedInterpretation {
                    canonical: <AutoWhiteBalanceSensitivityInquiry as BuiltinInquiryCommandMarker>::METADATA,
                },
                vendor_specific: true,
                rationale: Some("PTZOptics interprets the same register used by AWB sensitivity as tally auto-adjust state."),
            }
        ]

        /// The red tally light status.
        TallyRed {
            /// Whether the red tally light is on.
            on: bool,
        } => |payload| Ok(InquiryData::TallyRed {
            on: payload.parse_bool("tally_red_status", BoolConvention::OnIs02)?,
        });
        commands [
            /// Inquiry command to get the red tally light status.
            TallyRedInquiry {
                bytes: tally::RED_INQUIRY,
                typed: (bool, { on } => Ok(on)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: Some("Sony FR7 extended red-tally inquiry (81 09 7E 01 0A FF)."),
            }
        ]

        /// The green tally light status.
        TallyGreen {
            /// Whether the green tally light is on.
            on: bool,
        } => |payload| Ok(InquiryData::TallyGreen {
            on: payload.parse_bool("tally_green_status", BoolConvention::OnIs02)?,
        });
        commands [
            /// Inquiry command to get the green tally light status.
            TallyGreenInquiry {
                bytes: tally::GREEN_INQUIRY,
                typed: (bool, { on } => Ok(on)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("Sony FR7 extended green-tally inquiry (81 09 7E 04 1A FF)."),
            }
        ]

        /// The current flicker mode setting.
        FlickerMode {
            /// Current anti-flicker mode setting.
            mode: AntiFlickerMode,
        } => |payload| Ok(InquiryData::FlickerMode {
            mode: decode_enum(&payload)?,
        });
        commands [
            /// Inquiry command to get the current flicker mode setting.
            FlickerModeInquiry {
                bytes: [INQUIRY, 0x04, 0x55],
                typed: (AntiFlickerMode, { mode } => Ok(mode)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]

        /// The current contrast level.
        Contrast {
            /// Current contrast level.
            level: u8,
        } => |payload| Ok(InquiryData::Contrast {
            level: Nibbles::<4>::try_from(payload)?.zero_extended_u8()?,
        });
        commands [
            /// Inquiry command to get the current contrast level.
            ContrastInquiry {
                bytes: image::CONTRAST_INQUIRY,
                typed: (
                    crate::types::ContrastLevel,
                    { level } => crate::types::ContrastLevel::new(level)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]

        /// The current luminance level.
        Luminance {
            /// Current luminance (image processing brightness) level.
            level: u8,
        } => |payload| Ok(InquiryData::Luminance {
            level: Nibbles::<4>::try_from(payload)?.zero_extended_u8()?,
        });
        commands [
            /// Inquiry command to get the current luminance level.
            LuminanceInquiry {
                bytes: image::LUMINANCE_INQUIRY,
                typed: (
                    crate::types::LuminanceLevel,
                    { level } => crate::types::LuminanceLevel::new(level)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]
    }
    decode_only {
        /// Decode-only response kind for `ZoomOut`.
        ZoomOut {
            /// Whether zoom out is active.
            active: bool,
        } => |payload| Ok(InquiryData::ZoomOut {
            active: payload.parse_bool("zoom_out_status", BoolConvention::OnIs03)?,
        });
        vendor_specific: false,
        rationale: Some("Movement-state response decoded for routing, but no public built-in wire query is exposed.");

        /// Decode-only response kind for `ZoomIn`.
        ZoomIn {
            /// Whether zoom in is active.
            active: bool,
        } => |payload| Ok(InquiryData::ZoomIn {
            active: payload.parse_bool("zoom_in_status", BoolConvention::OnIs03)?,
        });
        vendor_specific: false,
        rationale: Some("Movement-state response decoded for routing, but no public built-in wire query is exposed.");

        /// Decode-only response kind for `ZoomTeleWide`.
        ZoomTeleWide {
            /// Whether zoom tele is active (false = wide active).
            tele: bool,
        } => |payload| Ok(InquiryData::ZoomTeleWide {
            tele: payload.parse_bool("zoom_tele_wide_status", BoolConvention::OnIs03)?,
        });
        vendor_specific: false,
        rationale: Some("Movement-state response decoded for routing, but no public built-in wire query is exposed.");

        /// Decode-only response kind for `AutoFocus`.
        AutoFocus {
            /// Whether auto focus is enabled.
            enabled: bool,
        } => |payload| Ok(InquiryData::AutoFocus {
            enabled: payload.parse_bool("autofocus_status", BoolConvention::OnIs03)?,
        });
        vendor_specific: false,
        rationale: Some("One-push autofocus status is decoded but not exposed as a built-in query command.");

        /// Decode-only response kind for `FocusNearFar`.
        FocusNearFar {
            /// Whether focus near is active (false = far active).
            near: bool,
        } => |payload| Ok(InquiryData::FocusNearFar {
            near: payload.parse_bool("focus_near_far_status", BoolConvention::OnIs03)?,
        });
        vendor_specific: false,
        rationale: Some("Near/far movement-state response decoded for routing, but no public built-in wire query is exposed.");

        /// Decode-only response kind for `IrisUp`.
        IrisUp {
            /// Whether iris up is active.
            active: bool,
        } => |payload| Ok(InquiryData::IrisUp {
            active: payload.parse_bool("iris_up_status", BoolConvention::OnIs03)?,
        });
        vendor_specific: false,
        rationale: Some("Iris movement-state response decoded for routing, but no public built-in wire query is exposed.");

        /// Decode-only response kind for `IrisDown`.
        IrisDown {
            /// Whether iris down is active.
            active: bool,
        } => |payload| Ok(InquiryData::IrisDown {
            active: payload.parse_bool("iris_down_status", BoolConvention::OnIs03)?,
        });
        vendor_specific: false,
        rationale: Some("Iris movement-state response decoded for routing, but no public built-in wire query is exposed.");

        /// Decode-only response kind for `Sharpness`.
        Sharpness {
            /// Current sharpness value.
            value: u8,
        } => |payload| Ok(InquiryData::Sharpness {
            value: Nibbles::<4>::try_from(payload)?.zero_extended_u8()?,
        });
        vendor_specific: false,
        rationale: Some("Legacy sharpness value response is decoded; public queries use SharpnessPositionInquiry.");

        /// Decode-only response kind for `Rtmp`.
        Rtmp {
            /// Whether RTMP streaming is enabled.
            on: bool,
        } => |payload| Ok(InquiryData::Rtmp {
            on: payload.parse_bool("rtmp_status", BoolConvention::OnIs03)?,
        });
        vendor_specific: true,
        rationale: Some("Streaming-state response decoded for vendor integrations, but no public built-in query command is exposed.");

        /// Decode-only response kind for `NightDayPosition`.
        NightDayPosition {
            /// Current night/day position value.
            position: u8,
        } => |payload| Ok(InquiryData::NightDayPosition {
            position: decode_byte(&payload)?,
        });
        vendor_specific: false,
        rationale: Some("Night/day position response is decode-only; public API exposes NightDayModeInquiry.");

        /// Decode-only response kind for `NightDaySwitch`.
        NightDaySwitch {
            /// Whether night/day switch is enabled.
            enabled: bool,
        } => |payload| Ok(InquiryData::NightDaySwitch {
            enabled: payload.parse_bool("night_day_switch", BoolConvention::OnIs03)?,
        });
        vendor_specific: false,
        rationale: Some("Night/day switch response is decode-only; public API exposes NightDayModeInquiry.");
    }
}

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

/// The data byte of a one-byte reply (`y0 50 pp FF`).
fn decode_byte(payload: &Payload<'_>) -> Result<u8, Error> {
    require_len(payload, 1)?;
    Ok(payload.as_slice()[0])
}

/// Decodes a one-byte code reply through the `ViscaEnum` byte table of `T`.
///
/// Every discrete code reply decodes through the enum's own discriminants, so
/// an unknown code is [`Error::InvalidResponse`] for every inquiry (#809).
fn decode_enum<T: TryFrom<u8, Error = Error>>(payload: &Payload<'_>) -> Result<T, Error> {
    T::try_from(decode_byte(payload)?)
}

/// Decodes a pan/tilt position reply with `framing`.
fn decode_pan_tilt_position(
    payload: Payload<'_>,
    framing: PanTiltFraming,
) -> Result<InquiryData, Error> {
    let (pan, tilt) = framing.decode_position(payload)?;
    Ok(InquiryData::PanTiltPosition { pan, tilt })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod generated_invariant_tests {
    use super::*;
    use crate::command::VISCA_TERMINATOR;

    #[test]
    fn generated_queryable_inquiries_frame_their_bodies_for_all_camera_ids() {
        for camera_num in 1..=8 {
            let camera_id = CameraId::new(camera_num).expect("valid camera id");
            let frames = encode_each_generated_inquiry(camera_id);
            assert_eq!(frames.len(), 63);
            for (name, body, frame) in frames {
                let frame = frame.unwrap_or_else(|error| panic!("{name} must encode: {error:?}"));
                let mut expected = vec![camera_id.to_address_byte()];
                expected.extend_from_slice(body);
                expected.push(VISCA_TERMINATOR);
                assert_eq!(
                    frame, expected,
                    "{name} must frame its body for {camera_id}"
                );
            }
        }
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn fuzz_inventory_tracks_every_generated_decoder_kind() {
        let distinct = FUZZ_INQUIRY_KINDS
            .iter()
            .map(core::mem::discriminant)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(distinct.len(), FUZZ_INQUIRY_KINDS.len());
        assert_eq!(FUZZ_INQUIRY_KINDS.len(), 73);
        assert_eq!(BUILTIN_INQUIRIES.len(), 74);
        for meta in BUILTIN_INQUIRIES {
            assert!(
                FUZZ_INQUIRY_KINDS.contains(&meta.kind),
                "{} must enter the fuzz matrix",
                meta.name
            );
        }
    }

    #[test]
    fn decode_only_entries_do_not_have_command_bytes() {
        for meta in BUILTIN_INQUIRIES {
            if matches!(meta.query, BuiltinInquiryQuery::DecodeOnly) {
                assert!(
                    meta.command.is_none(),
                    "{} must not expose a command",
                    meta.name
                );
                assert!(
                    meta.bytes.is_none(),
                    "{} must not expose request bytes",
                    meta.name
                );
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
                    let explicit = |query| {
                        matches!(
                            query,
                            BuiltinInquiryQuery::Alias { .. }
                                | BuiltinInquiryQuery::AlternateTypedInterpretation { .. }
                        )
                    };
                    assert!(
                        explicit(left.query) || explicit(right.query),
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
                    "{} command discriminator must expose its request body",
                    meta.name,
                );
            }

            if meta.vendor_specific {
                saw_vendor_specific = true;
            }
        }

        assert!(
            saw_vendor_specific,
            "vendor-specific inquiries must be modeled"
        );
    }

    #[test]
    fn queryable_typed_exclusions_are_exact() {
        let mut untyped = BUILTIN_INQUIRIES
            .iter()
            .filter(|meta| !matches!(meta.query, BuiltinInquiryQuery::DecodeOnly) && !meta.typed)
            .map(|meta| meta.name)
            .collect::<Vec<_>>();
        untyped.sort_unstable();

        assert_eq!(untyped, ["DefogModeInquiry"]);
    }

    #[test]
    fn representative_short_buffer_errors_are_exact() {
        let mut buffer = [0u8; 32];
        for (name, required, result) in [
            (
                "PowerInquiry",
                <PowerInquiry as crate::Request>::MAX_SIZE,
                WireEncode::write_into(
                    &PowerInquiry,
                    CameraId::CAMERA_1,
                    &mut buffer[..<PowerInquiry as crate::Request>::MAX_SIZE - 1],
                ),
            ),
            (
                "TallyRedInquiry",
                <TallyRedInquiry as crate::Request>::MAX_SIZE,
                WireEncode::write_into(
                    &TallyRedInquiry,
                    CameraId::CAMERA_1,
                    &mut buffer[..<TallyRedInquiry as crate::Request>::MAX_SIZE - 1],
                ),
            ),
        ] {
            assert!(
                matches!(
                    result,
                    Err(Error::BufferTooSmall {
                        required: reported_required,
                        actual,
                    }) if reported_required == required && actual == required - 1
                ),
                "{name} buffer-too-small details changed: {result:?}",
            );
        }
    }

    /// Issue #810: the tally inquiry used to have a test-only mirror constant
    /// (`81 09 7E 01 0A 00`) that disagreed with the bytes actually sent
    /// (`81 09 7E 01 0A FF`). Both tally families now share one register, so
    /// the emitted command and inquiry frames must carry the same register.
    #[test]
    fn tally_inquiries_and_commands_emit_one_shared_register() {
        use crate::command::{TallyGreenOn, TallyRedOn};

        let frame = |write: &dyn Fn(&mut [u8]) -> Result<usize, Error>| {
            let mut buffer = [0u8; 16];
            let len = write(&mut buffer).unwrap();
            buffer[..len].to_vec()
        };
        let red_inquiry =
            frame(&|b| WireEncode::write_into(&TallyRedInquiry, CameraId::CAMERA_1, b));
        let red_on = frame(&|b| WireEncode::write_into(&TallyRedOn, CameraId::CAMERA_1, b));
        assert_eq!(red_inquiry, [0x81, 0x09, 0x7E, 0x01, 0x0A, 0xFF]);
        assert_eq!(red_on, [0x81, 0x01, 0x7E, 0x01, 0x0A, 0x00, 0x02, 0xFF]);
        assert_eq!(red_inquiry[2..5], red_on[2..5]);

        let green_inquiry =
            frame(&|b| WireEncode::write_into(&TallyGreenInquiry, CameraId::CAMERA_1, b));
        let green_on = frame(&|b| WireEncode::write_into(&TallyGreenOn, CameraId::CAMERA_1, b));
        assert_eq!(green_inquiry, [0x81, 0x09, 0x7E, 0x04, 0x1A, 0xFF]);
        assert_eq!(green_inquiry[2..5], green_on[2..5]);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod wire_decoder_regression_tests {
    use super::*;
    use crate::{command::parse_inquiry_payload, Inquiry};

    #[test]
    fn iris_and_gain_typed_decoders_accept_their_profile_advertised_upper_halves() {
        let iris = IrisInquiry
            .decoder()
            .decode(&[0x00, 0x00, 0x01, 0x04])
            .unwrap();
        assert_eq!(iris.value(), 0x14);

        let gain = GainInquiry
            .decoder()
            .decode(&[0x00, 0x00, 0x00, 0x0C])
            .unwrap();
        assert_eq!(gain.value(), 0x0C);
    }

    #[test]
    fn centered_level_decoders_reject_out_of_range_nibbles_without_panicking() {
        for (kind, parameter, maximum) in [
            (
                InquiryKind::ExposureCompensation,
                "exposure_compensation",
                14,
            ),
            (InquiryKind::RedTuning, "red_tuning", 20),
            (InquiryKind::BlueTuning, "blue_tuning", 20),
        ] {
            // One encoded value just above the range and one that does not
            // fit `i8` take the same out-of-range path.
            for (payload, raw) in [
                ([0x00, 0x00, 0x08, 0x04], 0x84),
                (
                    [0x00, 0x00, (maximum + 1) >> 4, (maximum + 1) & 0x0F],
                    maximum + 1,
                ),
            ] {
                let response = parse_inquiry_payload(&payload, &kind);
                assert!(
                    matches!(
                        response,
                        Err(Error::ParameterOutOfRange {
                            parameter: actual,
                            value,
                            min: 0,
                            max,
                        }) if actual == parameter && value == i32::from(raw) && max == i32::from(maximum)
                    ),
                    "{parameter} {payload:02X?}: {response:?}"
                );
            }
        }
    }

    #[test]
    fn centered_levels_decode_through_their_wire_centre() {
        for (kind, payload, expected) in [
            (
                InquiryKind::ExposureCompensation,
                [0x00, 0x00, 0x00, 0x00],
                -7,
            ),
            (
                InquiryKind::ExposureCompensation,
                [0x00, 0x00, 0x00, 0x07],
                0,
            ),
            (InquiryKind::RedTuning, [0x00, 0x00, 0x01, 0x04], 10),
            (InquiryKind::BlueTuning, [0x00, 0x00, 0x00, 0x0A], 0),
        ] {
            let level = match parse_inquiry_payload(&payload, &kind).unwrap() {
                Response::Inquiry(InquiryData::ExposureCompensation { value }) => value,
                Response::Inquiry(InquiryData::RedTuning { level })
                | Response::Inquiry(InquiryData::BlueTuning { level }) => level,
                other => panic!("{kind:?}: {other:?}"),
            };
            assert_eq!(level, expected, "{kind:?} {payload:02X?}");
        }
    }

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
            let response = parse_inquiry_payload(&[wire], &InquiryKind::AutoFocusSensitivity);
            assert!(
                matches!(&response, Err(Error::InvalidResponse { actual, .. }) if actual == &[wire]),
                "{wire:#04X}: {response:?}"
            );
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
                    Ok(Response::Inquiry(InquiryData::FlipState { state }))
                        if *state == FlipState { horizontal, vertical }
                ),
                "documented flip mode {wire:#04X} must decode as ({horizontal}, {vertical}), got {response:?}"
            );
        }

        assert!(matches!(
            parse_inquiry_payload(&[0x04], &InquiryKind::FlipState),
            Err(Error::InvalidResponse { actual, .. }) if actual == [0x04]
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
                state: crate::command::TallyStatusState {
                    red_on: true,
                    green_on: false,
                },
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

    #[test]
    fn standard_pan_tilt_requires_eight_nibble_bytes() {
        assert!(matches!(
            parse_inquiry_payload(
                &[0x00, 0x01, 0x02, 0x03, 0x00, 0x04, 0x05, 0x06],
                &InquiryKind::PanTiltPosition,
            ),
            Ok(Response::Inquiry(InquiryData::PanTiltPosition {
                pan: 0x0123,
                tilt: 0x0456,
            }))
        ));

        assert!(matches!(
            parse_inquiry_payload(&[0x12, 0x34, 0x56, 0x78], &InquiryKind::PanTiltPosition),
            Err(Error::InvalidResponseLength {
                expected: 8,
                actual: 4,
                ..
            })
        ));
    }

    /// Issue #809: an unknown discrete code byte is `InvalidResponse` for
    /// every inquiry. Some decoders used to hand-match their enum's bytes and
    /// return the caller-error `InvalidParameter`, while the derived tables
    /// returned `InvalidResponse`.
    ///
    /// The sweep covers every response kind: each one-byte reply outside a
    /// kind's code set must be `InvalidResponse` carrying that byte, unless the
    /// kind is a numeric field whose out-of-range values are the value type's
    /// `ParameterOutOfRange`, or a raw byte that every value decodes.
    #[cfg(feature = "test-utils")]
    #[test]
    fn every_unknown_code_byte_is_invalid_response() {
        const CODE_KINDS: &[InquiryKind] = &[
            InquiryKind::Power,
            InquiryKind::ExposureMode,
            InquiryKind::ExposureCompensationMode,
            InquiryKind::WhiteBalanceMode,
            InquiryKind::SharpnessMode,
            InquiryKind::Backlight,
            InquiryKind::FlipState,
            InquiryKind::NoiseReduction2DMode,
            InquiryKind::FocusZone,
            InquiryKind::AutoFocusSensitivity,
            InquiryKind::FocusMode,
            InquiryKind::MenuOpenClose,
            InquiryKind::NightDayMode,
            InquiryKind::Standby,
            InquiryKind::FocusRange,
            InquiryKind::IrisControl,
            InquiryKind::DefogMode,
            InquiryKind::DigitalPtz,
            InquiryKind::AutoWhiteBalanceSensitivity,
            InquiryKind::AutoTrace,
            InquiryKind::FocusUnlock,
            InquiryKind::MotionSyncMode,
            InquiryKind::MotionSyncPreset,
            InquiryKind::UsbAudio,
            InquiryKind::TwoToneMode,
            InquiryKind::Digital,
            InquiryKind::TallyAutoAdjust,
            InquiryKind::TallyRed,
            InquiryKind::TallyGreen,
            InquiryKind::FlickerMode,
            InquiryKind::ZoomOut,
            InquiryKind::ZoomIn,
            InquiryKind::ZoomTeleWide,
            InquiryKind::AutoFocus,
            InquiryKind::FocusNearFar,
            InquiryKind::IrisUp,
            InquiryKind::IrisDown,
            InquiryKind::Rtmp,
            InquiryKind::NightDaySwitch,
        ];
        const UNKNOWN: u8 = 0x7F;

        for &kind in FUZZ_INQUIRY_KINDS {
            let result = parse_inquiry_payload(&[UNKNOWN], &kind);
            if CODE_KINDS.contains(&kind) {
                assert!(
                    matches!(&result, Err(Error::InvalidResponse { actual, .. }) if actual == &[UNKNOWN]),
                    "{kind:?}: an unknown code byte must be InvalidResponse, got {result:?}",
                );
            } else {
                assert!(
                    !matches!(
                        result,
                        Err(Error::InvalidResponse { .. } | Error::InvalidParameter { .. })
                    ),
                    "{kind:?} is not a code reply but reported {result:?}; list it in CODE_KINDS",
                );
            }
        }
    }

    /// Issue #828 L1: padded replies (`00 00 0p 0q`, `00 00 00 0p`) must have
    /// zero padding. The decoders used to drop the leading nibbles, so a
    /// sharpness position of `00 01 00 05` decoded as level 5.
    #[test]
    fn padded_replies_reject_nonzero_leading_nibbles() {
        let pair_kinds = [
            InquiryKind::ExposureCompensation,
            InquiryKind::Iris,
            InquiryKind::Shutter,
            InquiryKind::Brightness,
            InquiryKind::RedChannel,
            InquiryKind::BlueChannel,
            InquiryKind::Gain,
            InquiryKind::RedTuning,
            InquiryKind::BlueTuning,
            InquiryKind::SharpnessPosition,
            InquiryKind::Sharpness,
            InquiryKind::Contrast,
            InquiryKind::Luminance,
        ];
        for kind in pair_kinds {
            assert!(
                parse_inquiry_payload(&[0x00, 0x00, 0x00, 0x05], &kind).is_ok(),
                "{kind:?} must decode a zero-padded reply"
            );
            for padded in [[0x00, 0x01, 0x00, 0x05], [0x01, 0x00, 0x00, 0x05]] {
                let result = parse_inquiry_payload(&padded, &kind);
                assert!(
                    matches!(result, Err(Error::InvalidResponseFormat)),
                    "{kind:?} {padded:02X?}: {result:?}"
                );
            }
        }
        for kind in [InquiryKind::Saturation, InquiryKind::Hue] {
            for padded in [[0x00, 0x00, 0x01, 0x05], [0x0E, 0x00, 0x00, 0x05]] {
                let result = parse_inquiry_payload(&padded, &kind);
                assert!(
                    matches!(result, Err(Error::InvalidResponseFormat)),
                    "{kind:?} {padded:02X?}: {result:?}"
                );
            }
        }

        let sharpness = SharpnessPositionInquiry
            .decoder()
            .decode(&[0x00, 0x01, 0x00, 0x05]);
        assert!(
            matches!(sharpness, Err(Error::InvalidResponseFormat)),
            "{sharpness:?}"
        );
        let contrast = parse_inquiry_payload(&[0x00, 0x00, 0x01, 0x02], &InquiryKind::Contrast);
        assert!(
            matches!(
                contrast,
                Ok(Response::Inquiry(InquiryData::Contrast { level: 0x12 }))
            ),
            "the reference's `0p 0q` contrast position keeps both digits: {contrast:?}"
        );
    }

    /// Issue #828 L2: a malformed pan/tilt reply reports the same
    /// `InvalidResponseLength` whatever the profile's framing.
    #[test]
    fn pan_tilt_decode_errors_do_not_depend_on_the_profile_framing() {
        let brc = PanTiltFraming::SonyBrc300;
        let standard = PanTiltFraming::STANDARD;
        let eight = [0x00; 8];
        let nine = [0x00; 9];

        assert!(matches!(
            dispatch_with_framing(InquiryKind::PanTiltPosition, Payload::new(&eight), brc),
            Err(Error::InvalidResponseLength {
                expected: 9,
                actual: 8,
                ..
            })
        ));
        assert!(matches!(
            dispatch_with_framing(InquiryKind::PanTiltPosition, Payload::new(&nine), standard),
            Err(Error::InvalidResponseLength {
                expected: 8,
                actual: 9,
                ..
            })
        ));
        assert!(matches!(
            dispatch_with_framing(
                InquiryKind::PanTiltPosition,
                Payload::new(&[0x0F, 0x07, 0x05, 0x0A, 0x08, 0x0E, 0x07, 0x09, 0x06]),
                brc,
            ),
            Ok(Response::Inquiry(InquiryData::PanTiltPosition {
                pan: -0x08A58,
                tilt: -0x186A
            }))
        ));

        // Every other kind decodes identically with any framing.
        let power = dispatch_with_framing(InquiryKind::Power, Payload::new(&[0x02]), brc);
        assert!(matches!(
            power,
            Ok(Response::Inquiry(InquiryData::Power { on: true }))
        ));
    }
}
