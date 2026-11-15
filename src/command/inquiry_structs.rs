//! Built-in inquiry support generated from crate-local metadata.
//!
//! Built-in inquiries are intentionally not implemented through the public
//! `ViscaInquiry` derive. The single [`define_builtin_inquiries!`] table below
//! is the source of truth for the response discriminants, the decoded response
//! data, the decoder dispatch, the zero-sized inquiry commands and their
//! request bodies, the typed query conversions, and the invariant metadata.
//!
//! Each row declares a sourced response kind, its data and decoder, and the
//! one or more commands that query it. Unsupported packets use custom requests
//! and custom decoders instead of compatibility-only built-in response kinds.
//!
//! The profile gate of each typed inquiry is not part of this table: it is the
//! gate of the inquiry's noun-table row, which `crate::command::surface`
//! projects into [`BuiltinInquiryGate`].

use super::exposure::{AntiFlickerMode, ExposureMode};
use super::focus::{AutoFocusSensitivity, FocusMode, FocusZone};
use super::image::{ImageFlipMode, NoiseReduction2DMode, SharpnessMode};
use super::nd_filter::NdFilterValue;
use super::pan_tilt::PanTiltFraming;
use super::power::PowerStatus;
use super::resolution::PictureEffectMode;
use super::response::{BoolConvention, Nibbles, Payload, Response};
use super::system::{MotionSyncMode, MotionSyncPreset};
use super::white_balance::{AutoWhiteBalanceSensitivity, WhiteBalanceMode};
use crate::command::bytes::constants::{
    color, exposure, focus, gain, image, menu, power, streaming, tally, white_balance, zoom,
    INQUIRY,
};
use crate::command::bytes::FrameWriter;
use crate::command::surface::{BuiltinInquiryGate, RowGate};
use crate::command::{encode::WireEncode, FlipState, ResponseParser};
use crate::source_evidence::{source, Source};
use crate::types::{
    BlueTuning, ExposureCompensationLevel, ExposureCompensationPosition, NdFilterPreset, RedTuning,
};
use crate::{CameraId, Error};

// This is deliberately thread-local so unit tests can prove the profile gate
// runs before generated inquiry serialization without racing parallel tests.
#[cfg(test)]
std::thread_local! {
    static GENERATED_INQUIRY_WRITE_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(all(test, any(feature = "blocking", feature = "async")))]
pub(crate) fn reset_generated_inquiry_write_count() {
    GENERATED_INQUIRY_WRITE_COUNT.with(|count| count.set(0));
}

#[cfg(all(test, any(feature = "blocking", feature = "async")))]
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
    /// Generates a query command with shared bytes but a different typed
    /// interpretation of the same wire response.
    AlternateTypedInterpretation {
        /// Canonical command for the shared request bytes.
        canonical: BuiltinInquiryCommand,
    },
}

/// Static metadata for generated built-in inquiry invariants.
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct BuiltinInquiryMetadata {
    /// Human-readable registry entry name.
    pub(crate) name: &'static str,
    pub(crate) source: Source,
    pub(crate) boolean: Option<BoolConvention>,
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
    /// Every exposed built-in query has a typed response conversion.
    pub(crate) typed: bool,
    /// Whether this inquiry is vendor-specific rather than baseline VISCA.
    pub(crate) vendor_specific: bool,
    /// Rationale for aliases, alternate interpretations, and intentional gaps.
    pub(crate) rationale: Option<&'static str>,
    /// The actual request validator, absent for decode-only responses.
    pub(crate) validate: Option<fn(&crate::ProfileSpec) -> crate::Result<()>>,
}

#[cfg(test)]
macro_rules! builtin_inquiry_is_typed {
    (($($typed:tt)*)) => {
        true
    };
}

/// The typed response of a generated inquiry.
macro_rules! builtin_inquiry_response {
    (($response_ty:ty, $data_pattern:tt => $conversion:expr)) => {
        $response_ty
    };
}

/// Implements [`ResponseParser`] for a typed generated inquiry.
macro_rules! impl_builtin_response_parser {
    (($response_ty:ty, $data_pattern:tt => $conversion:expr), $struct:ident, $kind:ident) => {
        impl ResponseParser for $struct {
            type Response = $response_ty;

            fn from_response(resp: Response) -> Result<Self::Response, Error> {
                let result = match resp {
                    Response::Inquiry(InquiryData::$kind $data_pattern) => $conversion,
                    Response::Error(e) => Err(e),
                    _ => Err(Error::UnexpectedResponseType),
                };
                result.map_err(|error| reply_value_error(error, &[]))
            }
        }
    };
}

/// Lifts a decoded [`Response`] into a generated inquiry's typed response.
macro_rules! builtin_inquiry_lift {
    (($($typed:tt)*), $struct:ident, $response:expr) => {
        <$struct as ResponseParser>::from_response($response)
    };
}

/// Runtime admission for a generated inquiry under its noun row's gate.
///
/// The erased dynamic surface carries no compile-time bound, so it reproduces
/// at runtime the gate the static `<noun>().<inquiry>()` call resolves (#684):
/// a base-domain inquiry (`power().state()`, `zoom().position()`, ...) has no
/// `where` clause of its own and inherits its noun's base-domain marker, and a
/// typed one requires every typed-support surface from its noun header and
/// refining row marker.
fn validate_builtin_inquiry_gate(
    profile: &crate::ProfileSpec,
    gate: RowGate,
    inquiry: &'static str,
    typed_inquiry: &'static str,
) -> Result<(), Error> {
    let feature = match gate {
        RowGate::Typed(_) => typed_inquiry,
        RowGate::Always | RowGate::Domain(_) => inquiry,
    };
    gate.validate(profile, feature)
}

/// The facts every generated inquiry command shares with the generic decode
/// path below.
trait GeneratedInquiry: crate::Inquiry {
    /// The response kind the command queries.
    const KIND: InquiryKind;
    /// The address-free request body, without the terminator.
    const BODY: &'static [u8];
    const SOURCE: Source;
    const BOOLEAN: Option<BoolConvention>;

    /// Lifts the decoded response into the typed response.
    fn lift(response: Response) -> Result<Self::Response, Error>;
}

/// Decodes a payload as `C`'s response with the profile-less pan/tilt
/// framing.
fn decode_payload<C: GeneratedInquiry>(payload: &[u8]) -> Result<C::Response, Error> {
    C::lift(dispatch(C::KIND, Payload::new(payload))?)
        .map_err(|error| reply_value_error(error, payload))
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
        crate::ResponseDecoder::for_builtin_profile(C::KIND, profile, C::lift)
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

#[cfg(test)]
#[allow(clippy::expect_used)]
fn inquiry_test_profiles() -> Vec<crate::ProfileSpec> {
    use crate::profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    };
    let profiles = [
        crate::ProfileSpec::from_compile_time::<GenericVisca>(),
        crate::ProfileSpec::from_compile_time::<NearusBRC300>(),
        crate::ProfileSpec::from_compile_time::<PtzOptics30X>(),
        crate::ProfileSpec::from_compile_time::<PtzOpticsG2>(),
        crate::ProfileSpec::from_compile_time::<PtzOpticsG3>(),
        crate::ProfileSpec::from_compile_time::<SonyBRC300>(),
        crate::ProfileSpec::from_compile_time::<SonyBRCH900>(),
        crate::ProfileSpec::from_compile_time::<SonyEVIH100>(),
        crate::ProfileSpec::from_compile_time::<SonyFR7>(),
    ];
    assert_eq!(
        profiles.len(),
        crate::camera::profiles::BUILTIN_PROFILE_FACTS.len()
    );
    profiles
        .into_iter()
        .map(|profile| profile.expect("built-in profile"))
        .collect()
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
                            source: $source:expr,
                            boolean: $boolean:expr,
                            typed: $typed:tt,
                            query: $query:expr,
                            vendor_specific: $vendor_specific:expr,
                            rationale: $rationale:expr $(,)?
                        }
                    )+
                ]
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
        }

        /// Every derive kind is generated from this table too, so new kinds
        /// automatically enter the comparison against canonical decoding.
        #[cfg(test)]
        #[test]
        #[allow(clippy::expect_used)]
        fn derived_inquiries_match_the_canonical_table_on_every_profile_and_corpus_seed() {
            let profiles = inquiry_test_profiles();
            // The nested fuzz package is absent from a published root crate.
            // Always exercise the complete length/value matrix; augment it
            // with every checked-in corpus seed when testing the repository.
            let corpus = match std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/fuzz/corpus/response_parser")) {
                Ok(entries) => entries.map(|entry| std::fs::read(entry.expect("corpus entry").path()).expect("corpus seed")).collect::<Vec<_>>(),
                Err(error) => {
                    assert_eq!(error.kind(), std::io::ErrorKind::NotFound, "cannot read response parser corpus: {error}");
                    Vec::new()
                }
            };
            let mut payloads = (0..=18).map(|length| vec![0; length]).collect::<Vec<_>>();
            payloads.extend((0..=u8::MAX).map(|byte| vec![byte]));
            payloads.extend(corpus.iter().cloned());
            for bytes in &corpus {
                if let Some(crate::protocol::response::BasicResponse {
                    kind: crate::protocol::response::BasicKind::DataReply(payload), ..
                }) = crate::protocol::response::decode_basic(bytes) {
                    payloads.push(payload.as_slice().to_vec());
                }
            }
            // Pin the cases which exposed selector/profile drift directly,
            // even when a corpus curator changes a seed's frame wrapper.
            payloads.extend([
                vec![0, 0, 1, 4],
                vec![0x0f, 7, 5, 0x0a, 8, 0x0e, 7, 9, 6],
                vec![0; 10],
            ]);
            fn check<I: crate::Inquiry<Response = Response>>(
                inquiry: I, kind: InquiryKind, profiles: &[crate::ProfileSpec], payloads: &[Vec<u8>],
            ) {
                let profile_free = inquiry.decoder();
                for payload in payloads {
                    assert_eq!(format!("{:?}", profile_free.decode(payload)),
                        format!("{:?}", dispatch(kind, Payload::new(payload))),
                        "profile-free derived {kind:?} disagrees on {payload:02x?}");
                }
                for profile in profiles {
                    let decoder = inquiry.decoder_for_profile(profile);
                    for payload in payloads {
                        let framing = profile.pan_tilt_coordinates().map(PanTiltFraming::for_conversion);
                        let canonical = match (kind, framing) {
                            (InquiryKind::PanTiltPosition, None) => Err(Error::FeatureNotSupported {
                                feature: "pan/tilt coordinate conversion",
                            }),
                            (_, framing) => dispatch_with_framing(kind, Payload::new(payload), framing.unwrap_or(PanTiltFraming::STANDARD)),
                        };
                        assert_eq!(format!("{:?}", decoder.decode(payload)), format!("{canonical:?}"),
                            "derived {kind:?} on {} disagrees on {payload:02x?}", profile.name());
                    }
                }
            }
            $({
                #[derive(crate::ViscaInquiry)]
                #[visca(opcode = 0, response = $kind)]
                struct Derived;
                check(Derived, InquiryKind::$kind, &profiles, &payloads);
            })*
        }

        /// Decode an inquiry response with the profile-independent decoder
        /// of its kind.
        ///
        /// Generated from the built-in inquiry table, so every
        /// [`InquiryKind`] variant has exactly one decoder arm.
        pub(crate) fn dispatch(kind: InquiryKind, payload: Payload<'_>) -> Result<Response, Error> {
            let data = (|| { match kind {
                $(
                    InquiryKind::$kind => {
                        let $payload = payload;
                        $decode
                    }
                )*
            } })();
            data.map(Response::Inquiry).map_err(|error| reply_value_error(error, payload.as_slice()))
        }

        /// Generated inquiry-kind inventory used by the downstream fuzz
        /// harness. Keeping it beside the defining table makes newly added
        /// decoders enter the fuzz matrix automatically.
        #[cfg(all(feature = "test-utils", any(test, feature = "blocking")))]
        pub(crate) const FUZZ_INQUIRY_KINDS: &[InquiryKind] = &[
            $(InquiryKind::$kind,)*
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
                const SOURCE: Source = $source;
                const BOOLEAN: Option<BoolConvention> = $boolean;

                fn lift(response: Response) -> Result<Self::Response, Error> {
                    builtin_inquiry_lift!($typed, $struct, response)
                }
            }

            const _: () = assert!(
                !<$struct as GeneratedInquiry>::BOOLEAN.is_some() || <$struct as GeneratedInquiry>::SOURCE.is_sourced(),
                "built-in boolean polarity requires positive source evidence",
            );

            impl WireEncode for $struct {
                fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                    FrameWriter::new(camera_id, buffer).bytes(Self::BODY).finish()
                }
            }

            impl_builtin_response_parser!($typed, $struct, $kind);

            impl crate::Request for $struct {
                type Class = crate::request::Inquiry;

                const MAX_SIZE: usize = crate::command::bytes::frame_len(Self::BODY) + if matches!(Self::KIND, InquiryKind::Power) { 1 } else { 0 };
                const TIMEOUT_CLASS: crate::TimeoutClass = crate::TimeoutClass::Inquiry;
                const RETRY_CLASS: crate::RetryClass = crate::RetryClass::Inquiry;
                const CONTROL_CLASS: crate::ControlClass = crate::ControlClass::Normal;

                fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                    #[cfg(test)]
                    increment_generated_inquiry_write_count();
                    WireEncode::write_into(self, camera_id, buffer)
                }

                #[doc(hidden)]
                #[allow(private_interfaces)]
                fn write_for_target(&self, target: &crate::TargetRegistration, buffer: &mut [u8], _authority: crate::requests::RequestContractAuthority) -> crate::Result<usize> {
                    #[cfg(test)]
                    increment_generated_inquiry_write_count();
                    FrameWriter::new(target.target(), buffer).bytes(body_for_profile(Self::KIND, Self::BODY, target.profile())).finish()
                }

                fn validate_for_target(&self, target: &crate::TargetRegistration) -> crate::Result<()> {
                    let profile = target.profile();
                    Self::SOURCE.validate_for_profile(profile, concat!("sourced inquiry ", stringify!($struct)))?;
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

            #[cfg(any(feature = "blocking", feature = "async"))]

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
                    source: <$struct as GeneratedInquiry>::SOURCE,
                    boolean: <$struct as GeneratedInquiry>::BOOLEAN,
                    command: Some(<$struct as BuiltinInquiryCommandMarker>::METADATA),
                    kind: InquiryKind::$kind,
                    bytes: Some(<$struct as GeneratedInquiry>::BODY),
                    query: $query,
                    typed: builtin_inquiry_is_typed!($typed),
                    vendor_specific: $vendor_specific,
                    rationale: $rationale,
                    validate: Some(|profile| <$struct as crate::Request>::validate_for_target(&$struct, &crate::TargetRegistration::from_profile(crate::target::TargetSlot::CAMERA_1, profile.clone()))),
                },
            )+)*
        ];
    };
}

define_builtin_inquiries! {
    queryable {
        /// The current power state of the camera.
        Power {
            /// The power status, including a reported internal circuit error.
            status: PowerStatus,
        } => |payload| Ok(InquiryData::Power {
            status: decode_enum(&payload)?,
        });
        commands [
            /// Inquiry command to get the current power state of the camera.
            PowerInquiry {
                bytes: power::STATE_INQUIRY,
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-power-query", "09 04 00"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-power-query", "09 04 00"), (R1, [PtzOptics30X], "r1-ptzoptics30x-power-query", "09 04 00"), (R7, [SonyFr7], "r7-sonyfr7-power-query", "09 04 00"), (R11, [SonyBrcH900], "r11-sonybrch900-power-query", "09 7E 04 00"), (R8, [SonyEviH100], "r8-sonyevih100-power-query", "09 04 00"), (R12, [SonyBrc300], "r12-sonybrc300-power-query", "09 04 00"), (R21, [NearusBrc300], "r21-nearusbrc300-power-query", "09 04 00"), (R8, [GenericVisca], "r8-genericvisca-power-query", "09 04 00"), (R12, [GenericVisca], "r12-genericvisca-power-query", "09 04 00"), (R21, [GenericVisca], "r21-genericvisca-power-query", "09 04 00")]),
                boolean: None,
                typed: (PowerStatus, { status } => Ok(status)),
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
                source: source!([(R7, [SonyFr7], "r7-sonyfr7-version-query", "09 00 02"), (R11, [SonyBrcH900], "r11-sonybrch900-version-query", "09 00 02"), (R8, [SonyEviH100], "r8-sonyevih100-version-query", "09 00 02"), (R12, [SonyBrc300], "r12-sonybrc300-version-query", "09 00 02"), (R21, [NearusBrc300], "r21-nearusbrc300-version-query", "09 00 02"), (R8, [GenericVisca], "r8-genericvisca-version-query", "09 00 02"), (R12, [GenericVisca], "r12-genericvisca-version-query", "09 00 02"), (R21, [GenericVisca], "r21-genericvisca-version-query", "09 00 02")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-pantiltposition-query", "09 06 12"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-pantiltposition-query", "09 06 12"), (R1, [PtzOptics30X], "r1-ptzoptics30x-pantiltposition-query", "09 06 12"), (R7, [SonyFr7], "r7-sonyfr7-pantiltposition-query", "09 06 12"), (R11, [SonyBrcH900], "r11-sonybrch900-pantiltposition-query", "09 06 12"), (R8, [SonyEviH100], "r8-sonyevih100-pantiltposition-query", "09 06 12"), (R12, [SonyBrc300], "r12-sonybrc300-pantiltposition-query", "09 06 12"), (R21, [NearusBrc300], "r21-nearusbrc300-pantiltposition-query", "09 06 12"), (R8, [GenericVisca], "r8-genericvisca-pantiltposition-query", "09 06 12"), (R12, [GenericVisca], "r12-genericvisca-pantiltposition-query", "09 06 12"), (R21, [GenericVisca], "r21-genericvisca-pantiltposition-query", "09 06 12")]),
                boolean: None,
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
        } => |payload| Ok(InquiryData::ZoomPosition {
            position: Nibbles::<4>::try_from(payload)?.u16_quad(0).ok_or(Error::InvalidResponseFormat)?,
        });
        commands [
            /// Inquiry command to get the current zoom position.
            ZoomPositionInquiry {
                bytes: zoom::POSITION_INQUIRY,
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-zoomposition-query", "09 04 47"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-zoomposition-query", "09 04 47"), (R1, [PtzOptics30X], "r1-ptzoptics30x-zoomposition-query", "09 04 47"), (R7, [SonyFr7], "r7-sonyfr7-zoomposition-query", "09 04 47"), (R8, [SonyEviH100], "r8-sonyevih100-zoomposition-query", "09 04 47"), (R12, [SonyBrc300], "r12-sonybrc300-zoomposition-query", "09 04 47"), (R21, [NearusBrc300], "r21-nearusbrc300-zoomposition-query", "09 04 47"), (R8, [GenericVisca], "r8-genericvisca-zoomposition-query", "09 04 47"), (R12, [GenericVisca], "r12-genericvisca-zoomposition-query", "09 04 47"), (R21, [GenericVisca], "r21-genericvisca-zoomposition-query", "09 04 47")]),
                boolean: None,
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
            position: Nibbles::<4>::try_from(payload)?.u16_quad(0).ok_or(Error::InvalidResponseFormat)?,
        });
        commands [
            /// Inquiry command to get the current focus position.
            FocusPositionInquiry {
                bytes: focus::POSITION_INQUIRY,
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-focusposition-query", "09 04 48"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-focusposition-query", "09 04 48"), (R1, [PtzOptics30X], "r1-ptzoptics30x-focusposition-query", "09 04 48"), (R7, [SonyFr7], "r7-sonyfr7-focusposition-query", "09 04 48"), (R8, [SonyEviH100], "r8-sonyevih100-focusposition-query", "09 04 48"), (R12, [SonyBrc300], "r12-sonybrc300-focusposition-query", "09 04 48"), (R21, [NearusBrc300], "r21-nearusbrc300-focusposition-query", "09 04 48"), (R8, [GenericVisca], "r8-genericvisca-focusposition-query", "09 04 48"), (R12, [GenericVisca], "r12-genericvisca-focusposition-query", "09 04 48"), (R21, [GenericVisca], "r21-genericvisca-focusposition-query", "09 04 48")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-exposuremode-query", "09 04 39"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-exposuremode-query", "09 04 39"), (R1, [PtzOptics30X], "r1-ptzoptics30x-exposuremode-query", "09 04 39"), (R11, [SonyBrcH900], "r11-sonybrch900-exposuremode-query", "09 04 39"), (R8, [SonyEviH100], "r8-sonyevih100-exposuremode-query", "09 04 39"), (R12, [SonyBrc300], "r12-sonybrc300-exposuremode-query", "09 04 39"), (R21, [NearusBrc300], "r21-nearusbrc300-exposuremode-query", "09 04 39"), (R8, [GenericVisca], "r8-genericvisca-exposuremode-query", "09 04 39"), (R12, [GenericVisca], "r12-genericvisca-exposuremode-query", "09 04 39"), (R21, [GenericVisca], "r21-genericvisca-exposuremode-query", "09 04 39")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-exposurecompensation-query", "09 04 4E"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-exposurecompensation-query", "09 04 4E"), (R1, [PtzOptics30X], "r1-ptzoptics30x-exposurecompensation-query", "09 04 4E"), (R8, [SonyEviH100], "r8-sonyevih100-exposurecompensation-query", "09 04 4E"), (R12, [SonyBrc300], "r12-sonybrc300-exposurecompensation-query", "09 04 4E"), (R21, [NearusBrc300], "r21-nearusbrc300-exposurecompensation-query", "09 04 4E"), (R8, [GenericVisca], "r8-genericvisca-exposurecompensation-query", "09 04 4E"), (R12, [GenericVisca], "r12-genericvisca-exposurecompensation-query", "09 04 4E"), (R21, [GenericVisca], "r21-genericvisca-exposurecompensation-query", "09 04 4E")]),
                boolean: None,
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
            on: decode_builtin_bool::<ExposureCompensationModeInquiry>(&payload, "exposure_compensation_mode")?,
        });
        commands [
            /// Inquiry command to get the exposure compensation mode on/off status.
            ExposureCompensationModeInquiry {
                bytes: exposure::COMPENSATION_SWITCH_INQUIRY,
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-exposurecompensationmode-query", "09 04 3E"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-exposurecompensationmode-query", "09 04 3E"), (R1, [PtzOptics30X], "r1-ptzoptics30x-exposurecompensationmode-query", "09 04 3E"), (R8, [SonyEviH100], "r8-sonyevih100-exposurecompensationmode-query", "09 04 3E"), (R12, [SonyBrc300], "r12-sonybrc300-exposurecompensationmode-query", "09 04 3E"), (R21, [NearusBrc300], "r21-nearusbrc300-exposurecompensationmode-query", "09 04 3E"), (R8, [GenericVisca], "r8-genericvisca-exposurecompensationmode-query", "09 04 3E"), (R12, [GenericVisca], "r12-genericvisca-exposurecompensationmode-query", "09 04 3E"), (R21, [GenericVisca], "r21-genericvisca-exposurecompensationmode-query", "09 04 3E")]),
                boolean: Some(BoolConvention::OnIs02),
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-iris-query", "09 04 4B"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-iris-query", "09 04 4B"), (R1, [PtzOptics30X], "r1-ptzoptics30x-iris-query", "09 04 4B"), (R11, [SonyBrcH900], "r11-sonybrch900-iris-query", "09 04 4B"), (R8, [SonyEviH100], "r8-sonyevih100-iris-query", "09 04 4B"), (R12, [SonyBrc300], "r12-sonybrc300-iris-query", "09 04 4B"), (R21, [NearusBrc300], "r21-nearusbrc300-iris-query", "09 04 4B"), (R8, [GenericVisca], "r8-genericvisca-iris-query", "09 04 4B"), (R12, [GenericVisca], "r12-genericvisca-iris-query", "09 04 4B"), (R21, [GenericVisca], "r21-genericvisca-iris-query", "09 04 4B")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-shutter-query", "09 04 4A"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-shutter-query", "09 04 4A"), (R1, [PtzOptics30X], "r1-ptzoptics30x-shutter-query", "09 04 4A"), (R11, [SonyBrcH900], "r11-sonybrch900-shutter-query", "09 04 4A"), (R8, [SonyEviH100], "r8-sonyevih100-shutter-query", "09 04 4A"), (R12, [SonyBrc300], "r12-sonybrc300-shutter-query", "09 04 4A"), (R21, [NearusBrc300], "r21-nearusbrc300-shutter-query", "09 04 4A"), (R8, [GenericVisca], "r8-genericvisca-shutter-query", "09 04 4A"), (R12, [GenericVisca], "r12-genericvisca-shutter-query", "09 04 4A"), (R21, [GenericVisca], "r21-genericvisca-shutter-query", "09 04 4A")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-brightness-query", "09 04 4D"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-brightness-query", "09 04 4D"), (R1, [PtzOptics30X], "r1-ptzoptics30x-brightness-query", "09 04 4D"), (R8, [SonyEviH100], "r8-sonyevih100-brightness-query", "09 04 4D"), (R12, [SonyBrc300], "r12-sonybrc300-brightness-query", "09 04 4D"), (R21, [NearusBrc300], "r21-nearusbrc300-brightness-query", "09 04 4D"), (R8, [GenericVisca], "r8-genericvisca-brightness-query", "09 04 4D"), (R12, [GenericVisca], "r12-genericvisca-brightness-query", "09 04 4D"), (R21, [GenericVisca], "r21-genericvisca-brightness-query", "09 04 4D")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-whitebalancemode-query", "09 04 35"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-whitebalancemode-query", "09 04 35"), (R1, [PtzOptics30X], "r1-ptzoptics30x-whitebalancemode-query", "09 04 35"), (R7, [SonyFr7], "r7-sonyfr7-whitebalancemode-query", "09 04 35"), (R11, [SonyBrcH900], "r11-sonybrch900-whitebalancemode-query", "09 04 35"), (R8, [SonyEviH100], "r8-sonyevih100-whitebalancemode-query", "09 04 35"), (R12, [SonyBrc300], "r12-sonybrc300-whitebalancemode-query", "09 04 35"), (R21, [NearusBrc300], "r21-nearusbrc300-whitebalancemode-query", "09 04 35"), (R8, [GenericVisca], "r8-genericvisca-whitebalancemode-query", "09 04 35"), (R12, [GenericVisca], "r12-genericvisca-whitebalancemode-query", "09 04 35"), (R21, [GenericVisca], "r21-genericvisca-whitebalancemode-query", "09 04 35")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-colortemperature-query", "09 04 20"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-colortemperature-query", "09 04 20"), (R1, [PtzOptics30X], "r1-ptzoptics30x-colortemperature-query", "09 04 20")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-redgain-query", "09 04 43"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-redgain-query", "09 04 43"), (R1, [PtzOptics30X], "r1-ptzoptics30x-redgain-query", "09 04 43"), (R8, [SonyEviH100], "r8-sonyevih100-redgain-query", "09 04 43"), (R12, [SonyBrc300], "r12-sonybrc300-redgain-query", "09 04 43"), (R21, [NearusBrc300], "r21-nearusbrc300-redgain-query", "09 04 43"), (R8, [GenericVisca], "r8-genericvisca-redgain-query", "09 04 43"), (R12, [GenericVisca], "r12-genericvisca-redgain-query", "09 04 43"), (R21, [GenericVisca], "r21-genericvisca-redgain-query", "09 04 43")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-bluegain-query", "09 04 44"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-bluegain-query", "09 04 44"), (R1, [PtzOptics30X], "r1-ptzoptics30x-bluegain-query", "09 04 44"), (R8, [SonyEviH100], "r8-sonyevih100-bluegain-query", "09 04 44"), (R12, [SonyBrc300], "r12-sonybrc300-bluegain-query", "09 04 44"), (R21, [NearusBrc300], "r21-nearusbrc300-bluegain-query", "09 04 44"), (R8, [GenericVisca], "r8-genericvisca-bluegain-query", "09 04 44"), (R12, [GenericVisca], "r12-genericvisca-bluegain-query", "09 04 44"), (R21, [GenericVisca], "r21-genericvisca-bluegain-query", "09 04 44")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-sharpnessmode-query", "09 04 05"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-sharpnessmode-query", "09 04 05"), (R1, [PtzOptics30X], "r1-ptzoptics30x-sharpnessmode-query", "09 04 05")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-saturation-query", "09 04 49"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-saturation-query", "09 04 49"), (R1, [PtzOptics30X], "r1-ptzoptics30x-saturation-query", "09 04 49"), (R8, [SonyEviH100], "r8-sonyevih100-saturation-query", "09 04 49")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-hue-query", "09 04 4F"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-hue-query", "09 04 4F"), (R1, [PtzOptics30X], "r1-ptzoptics30x-hue-query", "09 04 4F"), (R8, [SonyEviH100], "r8-sonyevih100-hue-query", "09 04 4F")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-gain-query", "09 04 4C"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-gain-query", "09 04 4C"), (R1, [PtzOptics30X], "r1-ptzoptics30x-gain-query", "09 04 4C"), (R11, [SonyBrcH900], "r11-sonybrch900-gain-query", "09 04 4C"), (R8, [SonyEviH100], "r8-sonyevih100-gain-query", "09 04 4C"), (R12, [SonyBrc300], "r12-sonybrc300-gain-query", "09 04 4C"), (R21, [NearusBrc300], "r21-nearusbrc300-gain-query", "09 04 4C"), (R8, [GenericVisca], "r8-genericvisca-gain-query", "09 04 4C"), (R12, [GenericVisca], "r12-genericvisca-gain-query", "09 04 4C"), (R21, [GenericVisca], "r21-genericvisca-gain-query", "09 04 4C")]),
                boolean: None,
                typed: (
                    crate::types::GainLevel,
                    { gain } => crate::types::GainLevel::new(gain)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: Some("R8 Sony EVI-H100S/H100V Technical Manual, p.35, CAM_GainPosInq 8x 09 04 4C FF; y0 50 00 00 0p 0q FF. https://www.sony.com/electronics/support/res/manuals/AE4U/AE4U1001M.pdf"),
            }
        ]

        /// The current gain limit setting.
        GainLimit {
            /// Maximum gain limit (0x0=0 to 0xF=15).
            limit: u8,
        } => |payload| Ok(InquiryData::GainLimit {
            limit: Nibbles::<1>::try_from(payload)?.byte(0).ok_or(Error::InvalidResponseFormat)?,
        });
        commands [
            /// Inquiry command to get the current gain limit setting.
            GainLimitInquiry {
                bytes: gain::LIMIT_INQUIRY,
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-gainlimit-query", "09 04 2C"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-gainlimit-query", "09 04 2C"), (R1, [PtzOptics30X], "r1-ptzoptics30x-gainlimit-query", "09 04 2C")]),
                boolean: None,
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
            status: decode_builtin_bool::<BacklightInquiry>(&payload, "backlight_status")?,
        });
        commands [
            /// Inquiry command to get the backlight compensation mode.
            BacklightInquiry {
                bytes: image::BACKLIGHT_INQUIRY,
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-backlight-query", "09 04 33"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-backlight-query", "09 04 33"), (R1, [PtzOptics30X], "r1-ptzoptics30x-backlight-query", "09 04 33"), (R7, [SonyFr7], "r7-sonyfr7-backlight-query", "09 04 33"), (R11, [SonyBrcH900], "r11-sonybrch900-backlight-query", "09 04 33"), (R8, [SonyEviH100], "r8-sonyevih100-backlight-query", "09 04 33"), (R12, [SonyBrc300], "r12-sonybrc300-backlight-query", "09 04 33"), (R21, [NearusBrc300], "r21-nearusbrc300-backlight-query", "09 04 33"), (R8, [GenericVisca], "r8-genericvisca-backlight-query", "09 04 33"), (R12, [GenericVisca], "r12-genericvisca-backlight-query", "09 04 33"), (R21, [GenericVisca], "r21-genericvisca-backlight-query", "09 04 33")]),
                boolean: Some(BoolConvention::OnIs02),
                typed: (bool, { status } => Ok(status)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The vertical image-flip state, independent of horizontal mirroring.
        ImageFlip {
            /// Vertical flip state; horizontal mirroring is not queried.
            state: FlipState,
        } => |payload| Ok(InquiryData::ImageFlip {
            state: FlipState {
                vertical: decode_builtin_bool::<ImageFlipInquiry>(&payload, "vertical image flip")?,
                horizontal: false,
            },
        });
        commands [
            /// Queries the sourced vertical picture-flip register (`09 04 66`).
            ImageFlipInquiry {
                bytes: [INQUIRY, 0x04, 0x66],
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-imageflip-query", "09 04 66"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-imageflip-query", "09 04 66"), (R1, [PtzOptics30X], "r1-ptzoptics30x-imageflip-query", "09 04 66"), (R12, [SonyBrc300], "r12-sonybrc300-imageflip-query", "09 04 66"), (R21, [NearusBrc300], "r21-nearusbrc300-imageflip-query", "09 04 66")]),
                boolean: Some(BoolConvention::OnIs02),
                typed: (FlipState, { state } => Ok(state)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: Some("CAM_ImgFlipInq 09 04 66: R12 BRC-300 and R21 Nearus; R1 legacy PTZOptics picture flip, and R7/R11 Sony sources."),
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
            /// Queries the PTZOptics combined flip register (`09 04 A4`).
            FlipStateInquiry {
                bytes: image::FLIP_COMBINED_INQUIRY,
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-flipstate-query", "09 04 A4"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-flipstate-query", "09 04 A4"), (R1, [PtzOptics30X], "r1-ptzoptics30x-flipstate-query", "09 04 A4")]),
                boolean: None,
                typed: (FlipState, { state } => Ok(state)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R1 PTZOptics CAM_FlipInq returns combined horizontal/vertical flip state; docs/visca_reference.md section 7.10."),
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-noisereduction2dmode-query", "09 04 50"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-noisereduction2dmode-query", "09 04 50"), (R1, [PtzOptics30X], "r1-ptzoptics30x-noisereduction2dmode-query", "09 04 50")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-noisereduction2d-query", "09 04 53"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-noisereduction2d-query", "09 04 53"), (R1, [PtzOptics30X], "r1-ptzoptics30x-noisereduction2d-query", "09 04 53"), (R8, [SonyEviH100], "r8-sonyevih100-noisereduction2d-query", "09 04 53")]),
                boolean: None,
                typed: (
                    crate::command::NoiseReduction2D,
                    { level } => crate::command::NoiseReduction2D::from_byte(level)
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
            /// Every decoder accepts Off (`0`) or a nonzero level through `8`. This
            /// keeps well-formed readback values symmetric with the separately
            /// documented `04 54` setter domain across PTZOptics profiles (#717).
            NoiseReduction3DInquiry {
                bytes: image::NOISE_REDUCTION_3D_INQUIRY,
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-noisereduction3d-query", "09 04 54"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-noisereduction3d-query", "09 04 54"), (R1, [PtzOptics30X], "r1-ptzoptics30x-noisereduction3d-query", "09 04 54")]),
                boolean: None,
                typed: (
                    crate::command::NoiseReduction3D,
                    { level } => crate::command::NoiseReduction3D::from_byte(level)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: Some("The PTZOptics inquiry table identifies 04 54 as the 3D noise-reduction level register."),
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
                source: source!([(B1, [PtzOpticsG2], "b1-ptzopticsg2-focuszone-query", "09 04 AA"), (B1, [PtzOptics30X], "b1-ptzoptics30x-focuszone-query", "09 04 AA")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-autofocussensitivity-query", "09 04 58"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-autofocussensitivity-query", "09 04 58"), (R1, [PtzOptics30X], "r1-ptzoptics30x-autofocussensitivity-query", "09 04 58"), (R8, [SonyEviH100], "r8-sonyevih100-autofocussensitivity-query", "09 04 58")]),
                boolean: None,
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
            position: Nibbles::<4>::try_from(payload)?.u16_quad(0).ok_or(Error::InvalidResponseFormat)?,
        });
        commands [
            /// Inquiry command to get the focus near limit position.
            FocusNearLimitInquiry {
                bytes: focus::NEAR_LIMIT_INQUIRY,
                source: source!([(R8, [SonyEviH100], "r8-sonyevih100-focusnearlimit-query", "09 04 28")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-focusmode-query", "09 04 38"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-focusmode-query", "09 04 38"), (R1, [PtzOptics30X], "r1-ptzoptics30x-focusmode-query", "09 04 38"), (R7, [SonyFr7], "r7-sonyfr7-focusmode-query", "09 04 38"), (R11, [SonyBrcH900], "r11-sonybrch900-focusmode-query", "09 04 38"), (R8, [SonyEviH100], "r8-sonyevih100-focusmode-query", "09 04 38"), (R12, [SonyBrc300], "r12-sonybrc300-focusmode-query", "09 04 38"), (R21, [NearusBrc300], "r21-nearusbrc300-focusmode-query", "09 04 38"), (R8, [GenericVisca], "r8-genericvisca-focusmode-query", "09 04 38"), (R12, [GenericVisca], "r12-genericvisca-focusmode-query", "09 04 38"), (R21, [GenericVisca], "r21-genericvisca-focusmode-query", "09 04 38")]),
                boolean: None,
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
            is_open: decode_builtin_bool::<MenuOpenCloseInquiry>(&payload, "menu_status")?,
        });
        commands [
            /// Inquiry command to get the menu open/close status.
            MenuOpenCloseInquiry {
                bytes: menu::MENU_INQUIRY,
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-menuopenclose-query", "09 06 06"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-menuopenclose-query", "09 06 06"), (R1, [PtzOptics30X], "r1-ptzoptics30x-menuopenclose-query", "09 06 06"), (R7, [SonyFr7], "r7-sonyfr7-menuopenclose-query", "09 06 06")]),
                boolean: Some(BoolConvention::OnIs02),
                typed: (bool, { is_open } => Ok(is_open)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("PTZOptics menu inquiries use category 0x06 to match menu command bytes."),
            }
        ]




        /// The Sony FR7 variable ND-filter density.
        NdFilter {
            /// Variable-ND code, `00` (1/4) through `14` (1/128).
            position: NdFilterValue,
        } => |payload| Ok(InquiryData::NdFilter {
            position: NdFilterValue::new(u16::from(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?))
                .map_err(|error| Error::invalid_response(error.to_string(), payload.as_slice().to_vec()))?,
        });
        commands [
            /// Queries the sourced FR7 variable-ND density register.
            NdFilterInquiry {
                bytes: [INQUIRY, 0x7E, 0x04, 0x42],
                source: source!([(R7, [SonyFr7], "r7-sonyfr7-ndfilter-query", "09 7E 04 42")]),
                boolean: None,
                typed: (NdFilterValue, { position } => Ok(position)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 Sony ILME-FR7/FR7K VISCA Command List v4.00, p.21 ND FILTER VARIABLE: 8x 09 7E 04 42 FF -> y0 50 00 00 0p 0p FF; p.25 codes 00..14 (1/4..1/128). https://pro.sony/s3/2022/09/03065933/VISCA_Command_List_v4.pdf"),
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-pictureeffect-query", "09 04 63"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-pictureeffect-query", "09 04 63"), (R1, [PtzOptics30X], "r1-ptzoptics30x-pictureeffect-query", "09 04 63")]),
                boolean: None,
                typed: (PictureEffectMode, { effect } => Ok(effect)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-autowhitebalancesensitivity-query", "09 04 A9"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-autowhitebalancesensitivity-query", "09 04 A9"), (R1, [PtzOptics30X], "r1-ptzoptics30x-autowhitebalancesensitivity-query", "09 04 A9")]),
                boolean: None,
                typed: (
                    AutoWhiteBalanceSensitivity,
                    { sensitivity } => Ok(sensitivity)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: Some("The sourced PTZOptics register is AWB sensitivity; it grants no tally interpretation."),
            }
        ]

        /// The exposure compensation position.
        ExposureCompensationPosition {
            /// Exposure compensation position value (high-resolution EV adjustment).
            position: ExposureCompensationPosition,
        } => |payload| Ok(InquiryData::ExposureCompensationPosition {
            position: ExposureCompensationPosition::new(u16::from(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)),
        });
        commands [
            /// Inquiry command to get the exposure compensation position.
            ExposureCompensationPositionInquiry {
                bytes: exposure::COMPENSATION_INQUIRY,
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-exposurecompensationposition-query", "09 04 4E"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-exposurecompensationposition-query", "09 04 4E"), (R1, [PtzOptics30X], "r1-ptzoptics30x-exposurecompensationposition-query", "09 04 4E"), (R8, [SonyEviH100], "r8-sonyevih100-exposurecompensationposition-query", "09 04 4E"), (R12, [SonyBrc300], "r12-sonybrc300-exposurecompensationposition-query", "09 04 4E"), (R21, [NearusBrc300], "r21-nearusbrc300-exposurecompensationposition-query", "09 04 4E"), (R8, [GenericVisca], "r8-genericvisca-exposurecompensationposition-query", "09 04 4E"), (R12, [GenericVisca], "r12-genericvisca-exposurecompensationposition-query", "09 04 4E"), (R21, [GenericVisca], "r21-genericvisca-exposurecompensationposition-query", "09 04 4E")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-redtuning-query", "09 04 43"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-redtuning-query", "09 04 43"), (R1, [PtzOptics30X], "r1-ptzoptics30x-redtuning-query", "09 04 43")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-bluetuning-query", "09 04 44"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-bluetuning-query", "09 04 44"), (R1, [PtzOptics30X], "r1-ptzoptics30x-bluetuning-query", "09 04 44")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-gamma-query", "09 04 5B"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-gamma-query", "09 04 5B"), (R1, [PtzOptics30X], "r1-ptzoptics30x-gamma-query", "09 04 5B"), (R8, [SonyEviH100], "r8-sonyevih100-gamma-query", "09 04 5B")]),
                boolean: None,
                typed: (
                    crate::types::GammaLevel,
                    { value } => crate::types::GammaLevel::new(value)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-sharpnessposition-query", "09 04 42"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-sharpnessposition-query", "09 04 42"), (R1, [PtzOptics30X], "r1-ptzoptics30x-sharpnessposition-query", "09 04 42"), (R7, [SonyFr7], "r7-sonyfr7-sharpnessposition-query", "09 04 42"), (R8, [SonyEviH100], "r8-sonyevih100-sharpnessposition-query", "09 04 42")]),
                boolean: None,
                typed: (
                    crate::types::SharpnessLevel,
                    { position } => crate::types::SharpnessLevel::new(position)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
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
                source: Source::Unsourced,
                boolean: None,
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
                source: Source::Unsourced,
                boolean: None,
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
            on: decode_builtin_bool::<UsbAudioInquiry>(&payload, "usb_audio_status")?,
        });
        commands [
            /// Inquiry command to get the USB audio state.
            UsbAudioInquiry {
                bytes: streaming::USB_AUDIO,
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-usbaudio-query", "2A 02 A0 04"), (R1, [PtzOptics30X], "r1-ptzoptics30x-usbaudio-query", "2A 02 A0 04")]),
                boolean: Some(BoolConvention::OnIs02),
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
                source: source!([(R7, [SonyFr7], "r7-sonyfr7-ndfilterpreset-query", "09 7E 01 53")]),
                boolean: None,
                typed: (NdFilterPreset, { preset } => Ok(preset)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("Sony FR7 NDPresetInq; the legacy 09 04 66 register is picture-flip state, not an FR7 ND-filter preset."),
            }
        ]


        /// The red tally light status.
        TallyRed {
            /// Whether the red tally light is on.
            on: bool,
        } => |payload| Ok(InquiryData::TallyRed {
            on: decode_builtin_bool::<TallyRedInquiry>(&payload, "tally_red_status")?,
        });
        commands [
            /// Inquiry command to get the red tally light status.
            TallyRedInquiry {
                bytes: tally::RED_INQUIRY,
                source: source!([(R7, [SonyFr7], "r7-sonyfr7-tallyred-query", "09 7E 01 0A")]),
                boolean: Some(BoolConvention::OnIs02),
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
            on: decode_builtin_bool::<TallyGreenInquiry>(&payload, "tally_green_status")?,
        });
        commands [
            /// Inquiry command to get the green tally light status.
            TallyGreenInquiry {
                bytes: tally::GREEN_INQUIRY,
                source: source!([(R7, [SonyFr7], "r7-sonyfr7-tallygreen-query", "09 7E 04 1A")]),
                boolean: Some(BoolConvention::OnIs02),
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-flickermode-query", "09 04 55"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-flickermode-query", "09 04 55"), (R1, [PtzOptics30X], "r1-ptzoptics30x-flickermode-query", "09 04 55")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-contrast-query", "09 04 A2"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-contrast-query", "09 04 A2"), (R1, [PtzOptics30X], "r1-ptzoptics30x-contrast-query", "09 04 A2")]),
                boolean: None,
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
                source: source!([(R1, [PtzOpticsG2], "r1-ptzopticsg2-luminance-query", "09 04 A1"), (R14, [PtzOpticsG3], "r14-ptzopticsg3-luminance-query", "09 04 A1"), (R1, [PtzOptics30X], "r1-ptzoptics30x-luminance-query", "09 04 A1")]),
                boolean: None,
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
}

// ============================================================
// Helper functions
// ============================================================

/// A constructor rejecting a value read from the camera describes a protocol
/// fault. Callers using the same constructor directly keep its caller-error kind.
pub(crate) fn reply_value_error(error: Error, payload: &[u8]) -> Error {
    if error.kind() == crate::ErrorKind::InvalidParameter {
        Error::invalid_response(format!("camera reply value: {error}"), payload.to_vec())
    } else {
        match error {
            Error::InvalidResponse { expected, actual }
                if actual.is_empty() && !payload.is_empty() =>
            {
                Error::invalid_response(expected, payload.to_vec())
            }
            other => other,
        }
    }
}

/// Require payload to have exactly the specified length.
#[inline]
fn require_len(payload: &Payload<'_>, expected: usize) -> Result<(), Error> {
    if payload.len() != expected {
        return Err(Error::invalid_response_length(expected, payload.as_slice()));
    }
    Ok(())
}

/// The BRC-H900 places power status in its extended category. Keeping this
/// selection beside the generated inquiry preserves canonical public decoding
/// while custom requests continue to encode their declared opcode unchanged.
fn body_for_profile(
    kind: InquiryKind,
    standard: &'static [u8],
    profile: &crate::ProfileSpec,
) -> &'static [u8] {
    if matches!(kind, InquiryKind::Power)
        && matches!(
            profile.capabilities().profile_id,
            Some(crate::profiles::ProfileId::SonyBrcH900)
        )
    {
        &[INQUIRY, 0x7E, 0x04, 0x00]
    } else {
        standard
    }
}

/// Decode a boolean using the polarity declared beside its source reference.
fn decode_builtin_bool<C: GeneratedInquiry>(
    payload: &Payload<'_>,
    label: &'static str,
) -> Result<bool, Error> {
    let convention = C::BOOLEAN.ok_or(Error::InvalidResponseFormat)?;
    payload.parse_bool(label, convention)
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
    fn every_admitted_typed_inquiry_has_source_evidence() {
        let index = crate::source_evidence::checked::Index::parse(include_str!(
            "../../docs/visca_reference.md"
        ))
        .expect("checked positive evidence");
        for profile in inquiry_test_profiles() {
            let model = format!(
                "{:?}",
                profile.capabilities().profile_id.expect("builtin identity")
            );
            for meta in BUILTIN_INQUIRIES {
                if meta.validate.expect("typed query validator")(&profile).is_err() {
                    continue;
                }
                let packet =
                    body_for_profile(meta.kind, meta.bytes.expect("query bytes"), &profile)
                        .iter()
                        .map(|byte| format!("{byte:02X}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                let polarity = meta.boolean.map(|convention| match convention {
                    BoolConvention::OnIs02 => "02=on;03=off",
                    BoolConvention::OnIs03 => "03=on;02=off",
                });
                index
                    .validate(
                        meta.source,
                        meta.name,
                        Some(&model),
                        Some(&packet),
                        polarity,
                    )
                    .unwrap_or_else(|error| panic!("{model}: {error}"));
            }
        }
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn brc_h900_power_query_selects_its_documented_extended_category() {
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRCH900>()
            .expect("BRC-H900");
        let target =
            crate::TargetRegistration::from_profile(crate::target::TargetSlot::CAMERA_1, profile);
        let mut buffer = [0; 6];
        let written = <PowerInquiry as crate::Request>::write_for_target(
            &PowerInquiry,
            &target,
            &mut buffer,
            crate::requests::RequestContractAuthority::new(),
        )
        .expect("extended power inquiry");
        assert_eq!(&buffer[..written], &[0x81, 0x09, 0x7E, 0x04, 0x00, 0xFF]);
        let standard =
            crate::ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2");
        assert_eq!(
            body_for_profile(InquiryKind::Power, PowerInquiry::BODY, &standard),
            &[0x09, 0x04, 0x00]
        );
    }

    #[test]
    fn fr7_variable_nd_query_uses_the_sourced_register_and_payload() {
        use crate::{Inquiry, Request};
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::SonyFR7>().unwrap();
        assert!(NdFilterInquiry
            .validate_for_target(&crate::TargetRegistration::from_profile(
                crate::target::TargetSlot::CAMERA_1,
                profile.clone()
            ))
            .is_ok());
        let mut buffer = [0; 6];
        assert_eq!(
            Request::write_into(&NdFilterInquiry, CameraId::CAMERA_1, &mut buffer).unwrap(),
            6
        );
        assert_eq!(buffer, [0x81, 0x09, 0x7E, 0x04, 0x42, 0xFF]);
        let decoder = NdFilterInquiry.decoder();
        for value in 0..=0x14 {
            assert_eq!(
                decoder
                    .decode(&[0, 0, (value >> 4) as u8, (value & 0x0F) as u8])
                    .unwrap()
                    .value(),
                value
            );
        }
        for invalid in [&[0][..], &[0, 1, 0, 4], &[0, 0, 1, 5]] {
            assert_eq!(
                decoder.decode(invalid).unwrap_err().kind(),
                crate::ErrorKind::Protocol
            );
        }
    }

    #[test]
    fn vertical_and_combined_flip_inquiries_have_separate_gates_and_decoders() {
        use crate::{Inquiry, Request};
        let sony = crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
        let nearus =
            crate::ProfileSpec::from_compile_time::<crate::profiles::NearusBRC300>().unwrap();
        let g2 = crate::ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().unwrap();
        for profile in [&sony, &nearus] {
            assert!(ImageFlipInquiry
                .validate_for_target(&crate::TargetRegistration::from_profile(
                    crate::target::TargetSlot::CAMERA_1,
                    profile.clone()
                ))
                .is_ok());
            assert!(FlipStateInquiry
                .validate_for_target(&crate::TargetRegistration::from_profile(
                    crate::target::TargetSlot::CAMERA_1,
                    profile.clone()
                ))
                .is_err());
        }
        assert!(FlipStateInquiry
            .validate_for_target(&crate::TargetRegistration::from_profile(
                crate::target::TargetSlot::CAMERA_1,
                g2.clone()
            ))
            .is_ok());
        let mut buffer = [0; 5];
        assert_eq!(
            Request::write_into(&ImageFlipInquiry, CameraId::CAMERA_1, &mut buffer).unwrap(),
            5
        );
        assert_eq!(buffer, [0x81, 0x09, 0x04, 0x66, 0xFF]);
        Request::write_into(&FlipStateInquiry, CameraId::CAMERA_1, &mut buffer).unwrap();
        assert_eq!(buffer, [0x81, 0x09, 0x04, 0xA4, 0xFF]);
        let vertical = ImageFlipInquiry.decoder().decode(&[0x02]).unwrap();
        assert_eq!(
            vertical,
            FlipState {
                vertical: true,
                horizontal: false
            }
        );
        let off = ImageFlipInquiry.decoder().decode(&[0x03]).unwrap();
        assert_eq!(
            off,
            FlipState {
                vertical: false,
                horizontal: false
            }
        );
        let both = FlipStateInquiry.decoder().decode(&[0x03]).unwrap();
        assert_eq!(
            both,
            FlipState {
                vertical: true,
                horizontal: true
            }
        );
    }

    #[test]
    fn generated_queryable_inquiries_frame_their_bodies_for_all_camera_ids() {
        for camera_num in 1..=8 {
            let camera_id = CameraId::new(camera_num).expect("valid camera id");
            let frames = encode_each_generated_inquiry(camera_id);
            assert_eq!(frames.len(), 48);
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
        assert_eq!(FUZZ_INQUIRY_KINDS.len(), 48);
        assert_eq!(BUILTIN_INQUIRIES.len(), 48);
        for meta in BUILTIN_INQUIRIES {
            assert!(
                FUZZ_INQUIRY_KINDS.contains(&meta.kind),
                "{} must enter the fuzz matrix",
                meta.name
            );
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
                            BuiltinInquiryQuery::AlternateTypedInterpretation { .. }
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
                BuiltinInquiryQuery::AlternateTypedInterpretation { canonical } => {
                    assert!(meta.rationale.is_some(), "{} needs a rationale", meta.name);
                    assert_eq!(
                        meta.bytes,
                        Some(canonical.bytes()),
                        "{} must share request bytes with canonical {}",
                        meta.name,
                        canonical.name(),
                    );
                }
                BuiltinInquiryQuery::Queryable => {}
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
    fn representative_short_buffer_errors_are_exact() {
        let mut buffer = [0u8; 32];
        for (name, required, result) in [
            (
                "PowerInquiry",
                crate::command::bytes::frame_len(PowerInquiry::BODY),
                WireEncode::write_into(
                    &PowerInquiry,
                    CameraId::CAMERA_1,
                    &mut buffer[..crate::command::bytes::frame_len(PowerInquiry::BODY) - 1],
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
                        &response,
                        Err(Error::InvalidResponse { expected, actual })
                            if actual.as_slice() == payload && expected.contains(parameter)
                                && expected.contains(&raw.to_string())
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
    /// kind is a numeric field (whose range failure is also a protocol error),
    /// or a raw byte that every value decodes.
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
            InquiryKind::ImageFlip,
            InquiryKind::FlipState,
            InquiryKind::NoiseReduction2DMode,
            InquiryKind::FocusZone,
            InquiryKind::AutoFocusSensitivity,
            InquiryKind::FocusMode,
            InquiryKind::MenuOpenClose,
            InquiryKind::AutoWhiteBalanceSensitivity,
            InquiryKind::MotionSyncMode,
            InquiryKind::MotionSyncPreset,
            InquiryKind::UsbAudio,
            InquiryKind::TallyRed,
            InquiryKind::TallyGreen,
            InquiryKind::FlickerMode,
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
                        Err(Error::InvalidParameter { .. } | Error::ParameterOutOfRange { .. })
                    ),
                    "{kind:?} reported a caller error for a camera reply: {result:?}",
                );
            }
        }
    }

    /// Issue #828 L1: padded replies (`00 00 0p 0q`, `00 00 00 0p`) must have
    /// zero padding. The decoders used to drop the leading nibbles, so a
    /// sharpness position of `00 01 00 05` decoded as level 5.
    #[test]
    fn zoom_position_accepts_only_the_documented_four_nibbles() {
        use crate::Inquiry;
        let decoder = ZoomPositionInquiry.decoder();
        assert_eq!(decoder.decode(&[0, 1, 2, 3]).unwrap().value(), 0x0123);
        assert!(matches!(
            decoder.decode(&[0; 8]),
            Err(Error::InvalidResponseLength {
                expected: 4,
                actual: 8,
                ..
            })
        ));
    }

    #[test]
    fn decoded_numeric_range_errors_are_protocol_faults() {
        use crate::Inquiry;
        for (kind, payload) in [
            (InquiryKind::NdFilterPreset, &[0xFF][..]),
            (InquiryKind::ExposureCompensation, &[0, 0, 1, 0][..]),
            (InquiryKind::RedTuning, &[0, 0, 1, 5][..]),
            (InquiryKind::BlueTuning, &[0, 0, 1, 5][..]),
            (InquiryKind::NdFilter, &[0, 0, 1, 5][..]),
        ] {
            assert_eq!(
                parse_inquiry_payload(payload, &kind).unwrap_err().kind(),
                crate::ErrorKind::Protocol,
                "{kind:?}"
            );
        }
        // The table's raw u8 is lifted into the typed bounded value only here.
        for error in [
            GainInquiry.decoder().decode(&[0, 0, 1, 0]).unwrap_err(),
            IrisInquiry
                .decoder()
                .decode(&[0, 0, 0x0F, 0x0F])
                .unwrap_err(),
            HueInquiry.decoder().decode(&[0, 0, 0, 0x0F]).unwrap_err(),
        ] {
            assert_eq!(error.kind(), crate::ErrorKind::Protocol);
        }
        assert_eq!(
            crate::types::DefogLevel::new(0xFF).unwrap_err().kind(),
            crate::ErrorKind::InvalidParameter
        );
    }

    #[test]
    fn padded_replies_reject_nonzero_leading_nibbles() {
        let pair_kinds = [
            InquiryKind::ExposureCompensation,
            InquiryKind::ExposureCompensationPosition,
            InquiryKind::NdFilter,
            InquiryKind::Iris,
            InquiryKind::Shutter,
            InquiryKind::Brightness,
            InquiryKind::RedChannel,
            InquiryKind::BlueChannel,
            InquiryKind::Gain,
            InquiryKind::RedTuning,
            InquiryKind::BlueTuning,
            InquiryKind::SharpnessPosition,
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
        let brc = PanTiltFraming::new(
            crate::capabilities::PanTiltWireCodec::SonyBrc300,
            crate::capabilities::CoordinateSystem::SignedCentered,
        )
        .expect("signed BRC framing");
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
            Ok(Response::Inquiry(InquiryData::Power {
                status: PowerStatus::On
            }))
        ));
    }
}
