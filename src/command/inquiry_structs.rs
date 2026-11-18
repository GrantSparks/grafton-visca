//! Built-in inquiry support generated from crate-local metadata.
//!
//! Built-in inquiries are intentionally not implemented through the public
//! `ViscaInquiry` derive. The single [`define_builtin_inquiries!`] table below
//! is the source of truth for the response discriminants, the decoded response
//! data, the decoder dispatch, the zero-sized inquiry commands and their
//! references to catalogue-owned request bodies, the typed query conversions,
//! and the invariant metadata.
//!
//! Fixed request bytes live in `command::bytes::constants`, including
//! inquiry-only registers and shared command/inquiry prefixes.
//! Each row declares a sourced response kind, its data and decoder, and the
//! one or more commands that query it. Unsupported packets use custom requests
//! and custom decoders instead of compatibility-only built-in response kinds.
//!
//! A command may declare one `parameter: { name: Type = sample, byte: getter }`
//! after its fixed `bytes` prefix. The generated query owns that checked type,
//! appends the getter's byte in both encoding paths, and exposes `new` and an
//! argument accessor. Unit queries retain their zero-sized `Default` form.
//! Source metadata describes the fixed prefix; framing tests use the typed
//! sample to exercise the full request.
//!
//! The profile gate of each typed inquiry is not part of this table: it is the
//! gate of the inquiry's noun-table row, which `crate::command::surface`
//! projects into [`BuiltinInquiryGate`].

use super::exposure_wire::{AntiFlickerMode, ExposureMode};
use super::focus_wire::{AutoFocusSensitivity, FocusMode, FocusZone};
use super::image_wire::{ImageFlipMode, NoiseReduction2DMode, SharpnessMode};
use super::nd_filter_wire::NdFilterDensity;
use super::pan_tilt_wire::PanTiltFraming;
use super::power_wire::PowerStatus;
use super::resolution::PictureEffectMode;
use super::response::{BoolConvention, Nibbles, Payload, Response};
use super::white_balance_wire::{AutoWhiteBalanceSensitivity, WhiteBalanceMode};
use crate::command::bytes::constants::{
    color, exposure as exposure_wire, flip, focus as focus_wire, gain, image as image_wire,
    menu as menu_wire, nd_filter as nd_filter_wire, pan_tilt as pan_tilt_wire, power as power_wire,
    sony_fr7_exposure, sony_fr7_image, sony_fr7_presets, sony_fr7_white_balance,
    streaming as streaming_wire, system as system_wire, tally as tally_wire,
    white_balance as white_balance_wire, zoom as zoom_wire,
};
use crate::command::bytes::FrameWriter;
use crate::command::surface::{inquiry_kind_gate, RowGate};
use crate::command::{encode::WireEncode, ResponseParser};
use crate::source_evidence::{source, Source};
use crate::types::{ExposureCompensationLevel, NdFilterPreset, RgbTuning};
use crate::wire::WireValue;
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

/// The raw inquiry identity behind an explicit alternate interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuiltinInquiryCommand {
    kind: InquiryKind,
    #[cfg(test)]
    name: &'static str,
    #[cfg(test)]
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

pub(crate) trait BuiltinInquiryCommandMarker {
    const METADATA: BuiltinInquiryCommand;
}

/// How a built-in inquiry participates in command generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltinInquiryQuery {
    /// Generates a public query command with a unique fixed request prefix.
    Queryable,
    /// Generates a query command with shared bytes but a different typed
    /// interpretation of the same wire response.
    AlternateTypedInterpretation {
        /// Canonical command for the shared request bytes.
        canonical: BuiltinInquiryCommand,
    },
}

impl BuiltinInquiryQuery {
    /// Reuses the canonical packet's gate for an explicitly declared alternate
    /// interpretation, without a second hand-maintained inquiry-kind map.
    const fn gate_kind(self, own_kind: InquiryKind) -> InquiryKind {
        match self {
            Self::Queryable => own_kind,
            Self::AlternateTypedInterpretation { canonical } => canonical.kind,
        }
    }
}

/// Static metadata for generated built-in inquiry invariants.
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct BuiltinInquiryMetadata {
    /// Human-readable registry entry name.
    pub(crate) name: &'static str,
    pub(crate) source: Source,
    /// Permission inherited from the authoritative noun row.
    pub(crate) gate: RowGate,
    pub(crate) boolean: Option<BoolConvention>,
    /// Generated command identity.
    pub(crate) command: BuiltinInquiryCommand,
    /// Response discriminator used by routing and parsing.
    pub(crate) kind: InquiryKind,
    /// Address-free fixed prefix, without the argument or terminator.
    pub(crate) bytes: &'static [u8],
    /// Command-generation classification.
    pub(crate) query: BuiltinInquiryQuery,
    /// Whether this inquiry is vendor-specific rather than baseline VISCA.
    pub(crate) vendor_specific: bool,
    /// Rationale for aliases, alternate interpretations, and intentional gaps.
    pub(crate) rationale: Option<&'static str>,
    /// The actual request validator.
    pub(crate) validate: fn(&crate::ProfileSpec) -> crate::Result<()>,
    /// Read-only discovery preparation, with the typed grant deliberately bypassed.
    #[cfg(all(feature = "blocking", feature = "serde"))]
    pub(crate) prepare_probe:
        fn(&crate::TargetRegistration) -> crate::Result<Vec<ProbePreparedInquiry>>,
}

/// One canonical raw inquiry prepared for the private hardware evidence probe.
#[cfg(all(test, feature = "blocking", feature = "serde"))]
pub(crate) struct ProbePreparedInquiry {
    pub(crate) argument: Option<u8>,
    pub(crate) request: Vec<u8>,
    pub(crate) inquiry: crate::raw::InquiryRequest<ProbeReply>,
}

/// The actual data payload and the canonical typed decoder's independent result.
#[cfg(all(test, feature = "blocking", feature = "serde"))]
pub(crate) struct ProbeReply {
    pub(crate) payload: Vec<u8>,
    pub(crate) decoded: crate::Result<String>,
}

#[cfg(all(test, feature = "blocking", feature = "serde"))]
fn prepare_probe<C>(target: &crate::TargetRegistration) -> crate::Result<Vec<ProbePreparedInquiry>>
where
    C: GeneratedInquiry,
    C::Response: std::fmt::Debug + 'static,
{
    C::probe_samples(target.profile())?
        .into_iter()
        .map(|command| {
            // Discovery keeps parameter validation and target-specific framing.
            // Capture raw replies even when the profile cannot bind a typed
            // decoder, and record that refusal alongside the received bytes.
            command.probe_validate_arguments(target.profile())?;
            let decoder = command.decoder_for_profile(target.profile());
            let mut request = vec![0; C::MAX_SIZE];
            let len = command.write_for_target(
                target,
                &mut request,
                crate::requests::RequestContractAuthority::new(),
            )?;
            request.truncate(len);
            // GeneratedInquiry is an Inquiry<Class = InquiryClass>; no plain
            // request or operation can enter this factory. A vendor inquiry
            // need not start with 09 (the sourced UAC query starts with 2A).
            let inquiry = crate::raw::InquiryRequest::with_context(
                &request,
                decoder,
                |decoder, payload| {
                    Ok(ProbeReply {
                        payload: payload.to_vec(),
                        decoded: decoder
                            .as_ref()
                            .map_err(Clone::clone)
                            .and_then(|decoder| decoder.decode(payload))
                            .map(|value| format!("{value:?}")),
                    })
                },
                crate::request::InquiryRetryClass::Never,
                crate::SubmissionClass::Normal,
            )?
            .with_payload_shape(command.payload_shape_for_profile(target.profile()));
            Ok(ProbePreparedInquiry {
                argument: command.query_argument(),
                request,
                inquiry,
            })
        })
        .collect()
}

/// A checked query argument may also depend on the selected profile's inventory.
trait InquiryArgument {
    fn validate(self, profile: &crate::ProfileSpec) -> crate::Result<()>;

    #[cfg(all(test, feature = "blocking", feature = "serde"))]
    fn probe_samples(profile: &crate::ProfileSpec) -> crate::Result<Vec<Self>>
    where
        Self: Sized;
}

impl InquiryArgument for crate::types::PresetNumber {
    fn validate(self, profile: &crate::ProfileSpec) -> crate::Result<()> {
        crate::request::builtin::validate_preset(profile, self.value())
    }

    #[cfg(all(test, feature = "blocking", feature = "serde"))]
    fn probe_samples(profile: &crate::ProfileSpec) -> crate::Result<Vec<Self>> {
        let presets =
            profile
                .capabilities()
                .presets
                .as_ref()
                .ok_or(Error::FeatureNotSupported {
                    feature: "preset query arguments",
                })?;
        let mut samples = vec![Self::MIN];
        if presets.highest != Self::MIN.value() {
            samples.push(Self::new(presets.highest)?);
        }
        Ok(samples)
    }
}

impl InquiryArgument for crate::command::KneeParameter {
    fn validate(self, _profile: &crate::ProfileSpec) -> crate::Result<()> {
        // This closed selector has no narrower profile-dependent value domain.
        Ok(())
    }

    #[cfg(all(test, feature = "blocking", feature = "serde"))]
    fn probe_samples(_profile: &crate::ProfileSpec) -> crate::Result<Vec<Self>> {
        Ok((u8::MIN..=u8::MAX)
            .filter_map(|value| Self::from_wire(value).ok())
            .collect())
    }
}

/// Declare either a zero-sized query or a query owning one checked argument.
macro_rules! builtin_inquiry_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, Default)]
        pub struct $name;
    };
    ($(#[$meta:meta])* $name:ident, $argument:ident: $ty:ty) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name {
            $argument: $ty,
        }

        impl $name {
            /// Constructs the inquiry with its validated argument.
            #[must_use]
            pub const fn new($argument: $ty) -> Self { Self { $argument } }

            /// Returns the validated argument this inquiry addresses.
            #[must_use]
            pub const fn $argument(self) -> $ty { self.$argument }
        }
    };
}

macro_rules! builtin_inquiry_argument_len {
    () => {
        0
    };
    ($argument:ident) => {
        1
    };
}

#[cfg(test)]
macro_rules! builtin_inquiry_sample {
    ($name:ident) => {
        $name
    };
    ($name:ident, $sample:expr) => {
        $name::new($sample)
    };
}

/// The typed response of a generated inquiry.
macro_rules! builtin_inquiry_response {
    (($response_ty:ty, $data_pattern:tt => $conversion:expr)) => {
        $response_ty
    };
}

/// Implements [`crate::wire::ResponseParser`] for a typed generated inquiry.
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

/// Lifts a decoded [`crate::wire::Response`] into a generated inquiry's typed response.
macro_rules! builtin_inquiry_lift {
    (($($typed:tt)*), $struct:ident, $response:expr) => {
        <$struct as ResponseParser>::from_response($response)
    };
}

/// The typed profile-bound temperature decoder returns Kelvin directly; the
/// profile-independent InquiryData field always remains a wire-table index.
macro_rules! builtin_profile_lift {
    (ColorTemperatureInquiry) => {
        fn bound_decoder(
            profile: &crate::ProfileSpec,
        ) -> crate::Result<crate::ResponseDecoder<Self::Response>> {
            crate::ResponseDecoder::for_color_temperature(profile, Ok)
        }
    };
    ($struct:ident) => {};
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
        RowGate::Typed(_) | RowGate::DomainAndTyped(_, _) => typed_inquiry,
        RowGate::Always | RowGate::Domain(_) => inquiry,
    };
    gate.validate(profile, feature)
}

/// Frame the canonical fixed prefix and its optional checked argument.
fn write_query_frame(
    camera_id: CameraId,
    body: &[u8],
    argument: Option<u8>,
    buffer: &mut [u8],
) -> Result<usize, Error> {
    let frame = FrameWriter::new(camera_id, buffer).bytes(body);
    match argument {
        Some(argument) => frame.byte(argument),
        None => frame,
    }
    .finish()
}

/// The facts every generated inquiry command shares with the generic decode
/// path below.
pub(crate) trait GeneratedInquiry: crate::Inquiry {
    /// The response kind the command queries.
    const KIND: InquiryKind;
    /// The canonical noun gate, resolved once from this raw inquiry kind.
    const GATE: RowGate = match crate::command::surface::inquiry_kind_gate_if_present(Self::KIND) {
        Some(gate) => gate,
        None => inquiry_kind_gate(Self::GATE_KIND),
    };
    /// The same raw kind, or its table-declared canonical interpretation.
    const GATE_KIND: InquiryKind;
    /// The address-free fixed prefix, without the argument or terminator.
    const BODY: &'static [u8];
    const SOURCE: Source;
    const BOOLEAN: Option<BoolConvention>;

    /// The optional argument, already validated by its public value type.
    fn query_argument(&self) -> Option<u8> {
        None
    }

    /// A valid representative used by generated metadata and framing tests.
    #[cfg(test)]
    fn sample() -> Self
    where
        Self: Sized;

    #[cfg(all(test, feature = "blocking", feature = "serde"))]
    fn probe_samples(_profile: &crate::ProfileSpec) -> crate::Result<Vec<Self>>
    where
        Self: Sized,
    {
        Ok(vec![Self::sample()])
    }

    #[cfg(all(test, feature = "blocking", feature = "serde"))]
    fn probe_validate_arguments(&self, _profile: &crate::ProfileSpec) -> crate::Result<()> {
        Ok(())
    }

    /// Lifts the decoded response into the typed response.
    fn lift(response: Response) -> Result<Self::Response, Error>;

    /// Selects the typed decoder without rewriting profile-independent fields.
    fn bound_decoder(
        profile: &crate::ProfileSpec,
    ) -> crate::Result<crate::ResponseDecoder<Self::Response>>
    where
        Self::Response: 'static,
        Self: Sized,
    {
        if Self::KIND == InquiryKind::PanTiltPosition {
            crate::ResponseDecoder::for_builtin_profile(Self::KIND, profile, Self::lift)
        } else {
            Ok(crate::ResponseDecoder::from_fn(decode_payload::<Self>))
        }
    }
}

/// Decodes a payload as `C`'s response with the profile-less pan/tilt
/// framing.
fn decode_payload<C: GeneratedInquiry>(payload: &[u8]) -> Result<C::Response, Error> {
    C::lift(dispatch(C::KIND, Payload::new(payload))?)
        .map_err(|error| reply_value_error(error, payload))
}

/// The decoder a generated inquiry uses for a validated profile.
///
/// Pan/tilt framing and color-temperature steps depend on the profile. Other
/// inquiries keep the allocation-free profile-less decoder.
fn profile_decoder<C>(
    profile: &crate::ProfileSpec,
) -> crate::Result<crate::ResponseDecoder<C::Response>>
where
    C: GeneratedInquiry,
    C::Response: 'static,
{
    C::bound_decoder(profile)
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

/// Select the same per-axis position grammar used by the canonical decoder.
/// An unsupported profile supplies no correlation proof.
pub(crate) fn payload_shape_for_profile(
    kind: InquiryKind,
    profile: Option<&crate::ProfileSpec>,
) -> crate::PayloadShape {
    match (kind, profile) {
        (InquiryKind::PanTiltPosition, Some(profile)) => profile
            .pan_tilt_coordinates()
            .map(PanTiltFraming::for_conversion)
            .map_or(crate::PayloadShape::Unknown, |framing| {
                crate::PayloadShape::Nibbles(framing.position_nibbles())
            }),
        _ => base_payload_shape(kind),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
fn inquiry_test_profiles() -> Vec<crate::ProfileSpec> {
    use crate::profiles::{
        GenericVisca, NearusBrc300, PtzOptics30x, PtzOpticsG2, PtzOpticsG3, SonyBrc300,
        SonyBrcH900, SonyEviH100, SonyFr7,
    };
    let profiles = [
        crate::ProfileSpec::from_compile_time::<GenericVisca>(),
        crate::ProfileSpec::from_compile_time::<NearusBrc300>(),
        crate::ProfileSpec::from_compile_time::<PtzOptics30x>(),
        crate::ProfileSpec::from_compile_time::<PtzOpticsG2>(),
        crate::ProfileSpec::from_compile_time::<PtzOpticsG3>(),
        crate::ProfileSpec::from_compile_time::<SonyBrc300>(),
        crate::ProfileSpec::from_compile_time::<SonyBrcH900>(),
        crate::ProfileSpec::from_compile_time::<SonyEviH100>(),
        crate::ProfileSpec::from_compile_time::<SonyFr7>(),
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
                shape: $shape:expr;
                commands [
                    $(
                        $(#[$meta:meta])*
                        $struct:ident {
                            bytes: $bytes:expr,
                            $(parameter: { $argument:ident: $argument_ty:ty = $sample:expr, byte: $argument_byte:ident },)?
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
        /// [`crate::wire::Response::Inquiry(...)`](Response::Inquiry).
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
        #[allow(clippy::expect_used, clippy::panic)]
        fn derived_inquiries_match_the_canonical_table_on_every_profile_and_corpus_seed() {
            let profiles = inquiry_test_profiles();
            // The nested fuzz package is absent from a published root crate.
            // Always exercise the complete length/value matrix; augment it
            // with every checked-in corpus seed when testing the repository.
            let corpus = match std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/fuzz/corpus/response_parser")) {
                Ok(entries) => entries.map(|entry| std::fs::read(entry.expect("corpus entry").path()).expect("corpus seed")).collect::<Vec<_>>(),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
                Err(error) => panic!("cannot read response parser corpus: {error}"),
            };
            let mut payloads = (0..=18).map(|length| vec![0; length]).collect::<Vec<_>>();
            payloads.extend((0..=u8::MAX).map(|byte| vec![byte]));
            payloads.extend((0..=u8::MAX).map(|byte| vec![0, 0, byte >> 4, byte & 0x0f]));
            payloads.extend((1..=18).map(|length| vec![0x10; length]));
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
                vec![0, 7, 0x0d, 0], // R7 Kelvin minimum.
                vec![0, 1], // R7 common preset speed minimum.
                vec![0, 0, 0x0a], // R7 duration minimum.
            ]);
            fn check<I: crate::Inquiry<Response = Response>>(
                inquiry: I, kind: InquiryKind, profiles: &[crate::ProfileSpec], payloads: &[Vec<u8>],
            ) {
                let profile_free = inquiry.decoder();
                let shape = inquiry.payload_shape();
                assert_eq!(shape, base_payload_shape(kind));
                let mut accepted = false;
                for payload in payloads {
                    if profile_free.decode(payload).is_ok() {
                        accepted = true;
                        assert!(shape.admits(payload), "shape rejected canonical {kind:?} payload {payload:02x?}");
                    }
                    assert_eq!(format!("{:?}", profile_free.decode(payload)),
                        format!("{:?}", dispatch(kind, Payload::new(payload))),
                        "profile-free derived {kind:?} disagrees on {payload:02x?}");
                }
                assert!(accepted, "no canonical payload accepted for {kind:?}");
                for profile in profiles {
                    let shape = inquiry.payload_shape_for_profile(profile);
                    assert_eq!(shape, payload_shape_for_profile(kind, Some(profile)));
                    let framing = profile.pan_tilt_coordinates().map(PanTiltFraming::for_conversion);
                    let decoder = inquiry.decoder_for_profile(profile);
                    if kind == InquiryKind::PanTiltPosition && framing.is_none() {
                        assert!(matches!(decoder, Err(Error::FeatureNotSupported { .. })));
                        continue;
                    }
                    let decoder = decoder.expect("profile facts for derived decoder");
                    let mut accepted = false;
                    for payload in payloads {
                        if decoder.decode(payload).is_ok() {
                            accepted = true;
                            assert!(shape.admits(payload), "shape rejected canonical {kind:?} on {}: {payload:02x?}", profile.name());
                        }
                        let canonical = dispatch_with_framing(kind, Payload::new(payload), framing.unwrap_or(PanTiltFraming::STANDARD));
                        assert_eq!(format!("{:?}", decoder.decode(payload)), format!("{canonical:?}"),
                            "derived {kind:?} on {} disagrees on {payload:02x?}", profile.name());
                    }
                    assert!(accepted, "no canonical {kind:?} payload accepted for {}", profile.name());
                }
            }
            fn check_builtin<I: crate::Inquiry>(
                inquiry: I, kind: InquiryKind, profiles: &[crate::ProfileSpec], payloads: &[Vec<u8>],
            ) {
                assert_eq!(inquiry.payload_shape(), base_payload_shape(kind));
                if !matches!(kind, InquiryKind::PanTiltPosition | InquiryKind::ColorTemperature) {
                    let decoder = inquiry.decoder();
                    for payload in payloads {
                        let public = crate::wire::parse_inquiry_payload(payload, &kind);
                        let typed = decoder.decode(payload);
                        assert_eq!(public.is_ok(), typed.is_ok(),
                            "public and typed {kind:?} disagree on {payload:02x?}");
                        for error in public.err().into_iter().chain(typed.err()) {
                            assert_eq!(error.kind(), crate::ErrorKind::Protocol,
                                "camera reply {kind:?} {payload:02x?}: {error:?}");
                        }
                    }
                }
                for profile in profiles {
                    let shape = inquiry.payload_shape_for_profile(profile);
                    assert_eq!(shape, payload_shape_for_profile(kind, Some(profile)));
                    let missing_conversion = match kind {
                        InquiryKind::PanTiltPosition => profile.pan_tilt_coordinates().is_none(),
                        InquiryKind::ColorTemperature => profile.capabilities().white_balance.as_ref()
                            .is_none_or(|domain| domain.color_temperature_steps.is_empty()),
                        _ => false,
                    };
                    let decoder = inquiry.decoder_for_profile(profile);
                    if missing_conversion {
                        assert!(matches!(decoder, Err(Error::FeatureNotSupported { .. })));
                        continue;
                    }
                    let decoder = decoder.expect("profile facts for built-in decoder");
                    let mut accepted = false;
                    for payload in payloads {
                        if decoder.decode(payload).is_ok() {
                            accepted = true;
                            assert!(shape.admits(payload), "shape rejected built-in {kind:?} payload {payload:02x?}");
                        }
                    }
                    assert!(accepted, "no built-in {kind:?} canonical payload accepted for {}", profile.name());
                }
            }
            $({
                #[derive(crate::ViscaInquiry)]
                #[visca(opcode = 0, response = $kind)]
                struct Derived;
                check(Derived, InquiryKind::$kind, &profiles, &payloads);
                $(check_builtin(<$struct as GeneratedInquiry>::sample(), InquiryKind::$kind, &profiles, &payloads);)+
            })*
        }

        /// Structural data-payload domain generated beside its canonical decoder.
        pub(crate) const fn base_payload_shape(kind: InquiryKind) -> crate::PayloadShape {
            match kind {
                $(InquiryKind::$kind => $shape,)*
            }
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
            // Test inventories also exercise private alternate interpretations.
            #[allow(unused_imports)]
            pub use super::{$($($struct,)+)*};
        }

        $($(
            builtin_inquiry_type!($(#[$meta])* $struct $(, $argument: $argument_ty)?);

            const _: RowGate = <$struct as GeneratedInquiry>::GATE;

            impl GeneratedInquiry for $struct {
                const KIND: InquiryKind = InquiryKind::$kind;
                const GATE_KIND: InquiryKind = ($query).gate_kind(InquiryKind::$kind);
                const BODY: &'static [u8] = &$bytes;
                const SOURCE: Source = $source;
                const BOOLEAN: Option<BoolConvention> = $boolean;

                $(fn query_argument(&self) -> Option<u8> {
                    Some(self.$argument.$argument_byte())
                })?

                #[cfg(test)]
                fn sample() -> Self { builtin_inquiry_sample!($struct $(, $sample)?) }

                $(#[cfg(all(test, feature = "blocking", feature = "serde"))]
                fn probe_samples(profile: &crate::ProfileSpec) -> crate::Result<Vec<Self>> {
                    <$argument_ty as InquiryArgument>::probe_samples(profile)
                        .map(|samples| samples.into_iter().map(Self::new).collect())
                }

                #[cfg(all(test, feature = "blocking", feature = "serde"))]
                fn probe_validate_arguments(&self, profile: &crate::ProfileSpec) -> crate::Result<()> {
                    InquiryArgument::validate(self.$argument, profile)
                })?

                fn lift(response: Response) -> Result<Self::Response, Error> {
                    builtin_inquiry_lift!($typed, $struct, response)
                }

                builtin_profile_lift!($struct);
            }

            const _: () = assert!(
                !<$struct as GeneratedInquiry>::BOOLEAN.is_some() || <$struct as GeneratedInquiry>::SOURCE.is_sourced(),
                "built-in boolean polarity requires positive source evidence",
            );

            impl WireEncode for $struct {
                fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                    write_query_frame(camera_id, Self::BODY, <Self as GeneratedInquiry>::query_argument(self), buffer)
                }
            }

            impl_builtin_response_parser!($typed, $struct, $kind);

            impl crate::Request for $struct {
                type Class = crate::request::InquiryClass;

                const MAX_SIZE: usize = crate::command::bytes::frame_len(maximum_body(Self::KIND, Self::BODY))
                    + builtin_inquiry_argument_len!($($argument)?);
                fn policy(&self) -> crate::request::InquiryPolicy {
                    crate::request::InquiryPolicy::new(crate::request::InquiryRetryClass::Inquiry, crate::request::SubmissionClass::Normal)
                }

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
                    write_query_frame(target.target().into(), body_for_profile(Self::KIND, Self::BODY, target.profile()), <Self as GeneratedInquiry>::query_argument(self), buffer)
                }

                fn validate_for_target(&self, target: &crate::TargetRegistration) -> crate::Result<()> {
                    let profile = target.profile();
                    validate_builtin_inquiry_gate(
                        profile,
                        <Self as GeneratedInquiry>::GATE,
                        concat!("inquiry ", stringify!($struct)),
                        concat!("typed inquiry ", stringify!($struct)),
                    )?;
                    Self::SOURCE.validate_for_profile(profile, concat!("sourced inquiry ", stringify!($struct)))?;
                    $(InquiryArgument::validate(self.$argument, profile)?;)?
                    Ok(())
                }
            }

            impl crate::Inquiry for $struct {
                type Response = builtin_inquiry_response!($typed);


                fn payload_shape(&self) -> crate::PayloadShape {
                    base_payload_shape(Self::KIND)
                }

                fn payload_shape_for_profile(&self, profile: &crate::ProfileSpec) -> crate::PayloadShape {
                    payload_shape_for_profile(Self::KIND, Some(profile))
                }

                fn decoder(&self) -> crate::ResponseDecoder<Self::Response> {
                    crate::ResponseDecoder::from_fn(decode_payload::<Self>)
                }

                fn decoder_for_profile(
                    &self,
                    profile: &crate::ProfileSpec,
                ) -> crate::Result<crate::ResponseDecoder<Self::Response>> {
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

            impl BuiltinInquiryCommandMarker for $struct {
                const METADATA: BuiltinInquiryCommand = BuiltinInquiryCommand {
                    kind: InquiryKind::$kind,
                    #[cfg(test)]
                    name: stringify!($struct),
                    #[cfg(test)]
                    bytes: <$struct as GeneratedInquiry>::BODY,
                };
            }
        )+)*

        /// Every generated command's name, request body and encoded frame.
        #[cfg(test)]
        fn encode_each_generated_inquiry(
            camera_id: CameraId,
        ) -> Vec<(&'static str, Vec<u8>, Result<Vec<u8>, Error>)> {
            let encode = |command: &dyn WireEncode| {
                let mut buffer = [0u8; 32];
                command
                    .write_into(camera_id, &mut buffer)
                    .map(|len| buffer[..len].to_vec())
            };
            vec![
                $($(
                    {
                        let command = <$struct as GeneratedInquiry>::sample();
                        let mut body = <$struct as GeneratedInquiry>::BODY.to_vec();
                        body.extend(<$struct as GeneratedInquiry>::query_argument(&command));
                        (stringify!($struct), body, encode(&command))
                    },
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
                    gate: <$struct as GeneratedInquiry>::GATE,
                    boolean: <$struct as GeneratedInquiry>::BOOLEAN,
                    command: <$struct as BuiltinInquiryCommandMarker>::METADATA,
                    kind: InquiryKind::$kind,
                    bytes: <$struct as GeneratedInquiry>::BODY,
                    query: $query,
                    vendor_specific: $vendor_specific,
                    rationale: $rationale,
                    validate: |profile| <$struct as crate::Request>::validate_for_target(&<$struct as GeneratedInquiry>::sample(), &crate::TargetRegistration::from_profile(crate::target::TargetSlot::CAMERA_1, profile.clone())),
                    #[cfg(all(feature = "blocking", feature = "serde"))]
                    prepare_probe: prepare_probe::<$struct>,
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
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the current power state of the camera.
            PowerInquiry {
                bytes: power_wire::STATE_INQUIRY,
                source: source!([(R1, "r1-power-query"), (R14, "r14-ptzopticsg3-power-query"), (R7, "r7-sonyfr7-power-query"), (R11, "r11-sonybrch900-power-query"), (R8, "r8-sonyevih100-power-query"), (R12, "r12-sonybrc300-power-query"), (R21, "r21-nearusbrc300-power-query")]),
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
        shape: crate::PayloadShape::Bytes(7);
        commands [
            /// Inquiry command to get the camera version information.
            VersionInquiry {
                bytes: system_wire::VERSION_INQUIRY,
                source: source!([(R7, "r7-sonyfr7-version-query"), (R11, "r11-sonybrch900-version-query"), (R8, "r8-sonyevih100-version-query"), (R12, "r12-sonybrc300-version-query"), (R21, "r21-nearusbrc300-version-query")]),
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
        shape: crate::PayloadShape::Nibbles(PanTiltFraming::STANDARD.position_nibbles());
        commands [
            /// Inquiry command to get the current pan/tilt position.
            PanTiltPositionInquiry {
                bytes: pan_tilt_wire::POSITION_INQUIRY,
                source: source!([(R1, "r1-pantiltposition-query"), (R14, "r14-ptzopticsg3-pantiltposition-query"), (R7, "r7-sonyfr7-pantiltposition-query"), (R11, "r11-sonybrch900-pantiltposition-query"), (R8, "r8-sonyevih100-pantiltposition-query"), (R12, "r12-sonybrc300-pantiltposition-query"), (R21, "r21-nearusbrc300-pantiltposition-query")]),
                boolean: None,
                typed: (
                    crate::camera::PanTiltUnits,
                    { pan, tilt } => Ok(crate::camera::PanTiltUnits { pan, tilt })
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current zoom position.
        ZoomPosition {
            /// Zoom position value.
            position: crate::types::ZoomPosition,
        } => |payload| Ok(InquiryData::ZoomPosition {
            position: crate::types::ZoomPosition::new(Nibbles::<4>::try_from(payload)?.value())?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the current zoom position.
            ZoomPositionInquiry {
                bytes: zoom_wire::POSITION_INQUIRY,
                source: source!([(R1, "r1-zoomposition-query"), (R14, "r14-ptzopticsg3-zoomposition-query"), (R7, "r7-sonyfr7-zoomposition-query"), (R8, "r8-sonyevih100-zoomposition-query"), (R12, "r12-sonybrc300-zoomposition-query"), (R21, "r21-nearusbrc300-zoomposition-query"), (R11, "r11-sonybrch900-zoomposition-query")]),
                boolean: None,
                typed: (
                    crate::types::ZoomPosition,
                    { position } => Ok(position)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current focus position.
        FocusPosition {
            /// Focus position value.
            position: crate::types::FocusPosition,
        } => |payload| Ok(InquiryData::FocusPosition {
            position: crate::types::FocusPosition::new(Nibbles::<4>::try_from(payload)?.value())?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the current focus position.
            FocusPositionInquiry {
                bytes: focus_wire::POSITION_INQUIRY,
                source: source!([(R1, "r1-focusposition-query"), (R14, "r14-ptzopticsg3-focusposition-query"), (R7, "r7-sonyfr7-focusposition-query"), (R8, "r8-sonyevih100-focusposition-query"), (R12, "r12-sonybrc300-focusposition-query"), (R21, "r21-nearusbrc300-focusposition-query"), (R11, "r11-sonybrch900-focusposition-query")]),
                boolean: None,
                typed: (
                    crate::types::FocusPosition,
                    { position } => Ok(position)
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
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the current exposure mode setting.
            ExposureModeInquiry {
                bytes: exposure_wire::MODE_INQUIRY,
                source: source!([(R1, "r1-exposuremode-query"), (R14, "r14-ptzopticsg3-exposuremode-query"), (R11, "r11-sonybrch900-exposuremode-query"), (R8, "r8-sonyevih100-exposuremode-query"), (R12, "r12-sonybrc300-exposuremode-query"), (R21, "r21-nearusbrc300-exposuremode-query")]),
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
            value: ExposureCompensationLevel,
        } => |payload| Ok(InquiryData::ExposureCompensation {
            value: ExposureCompensationLevel::from_wire(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the current exposure compensation value.
            ExposureCompensationInquiry {
                bytes: exposure_wire::COMPENSATION_INQUIRY,
                source: source!([(R1, "r1-exposurecompensation-query"), (R14, "r14-ptzopticsg3-exposurecompensation-query"), (R8, "r8-sonyevih100-exposurecompensation-query"), (R12, "r12-sonybrc300-exposurecompensation-query"), (R21, "r21-nearusbrc300-exposurecompensation-query")]),
                boolean: None,
                typed: (
                    ExposureCompensationLevel,
                    { value } => Ok(value)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The exposure compensation mode on/off status.
        ExposureCompensationMode {
            /// Whether exposure compensation is enabled.
            on: bool,
        } => |payload| Ok(InquiryData::ExposureCompensationMode {
            on: decode_builtin_bool::<ExposureCompensationModeInquiry>(&payload, "exposure_compensation_mode")?,
        });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the exposure compensation mode on/off status.
            ExposureCompensationModeInquiry {
                bytes: exposure_wire::COMPENSATION_SWITCH_INQUIRY,
                source: source!([(R1, "r1-exposurecompensationmode-query"), (R14, "r14-ptzopticsg3-exposurecompensationmode-query"), (R8, "r8-sonyevih100-exposurecompensationmode-query"), (R12, "r12-sonybrc300-exposurecompensationmode-query"), (R21, "r21-nearusbrc300-exposurecompensationmode-query")]),
                boolean: Some(crate::command::exposure_wire::ExposureCompensation::CONVENTION),
                typed: (bool, { on } => Ok(on)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current iris position value.
        Iris {
            /// Checked iris position.
            position: crate::types::IrisLevel,
        } => |payload| Ok(InquiryData::Iris {
            position: crate::types::IrisLevel::new(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the current iris position value.
            IrisInquiry {
                bytes: exposure_wire::IRIS_INQUIRY,
                source: source!([(R1, "r1-iris-query"), (R14, "r14-ptzopticsg3-iris-query"), (R11, "r11-sonybrch900-iris-query"), (R8, "r8-sonyevih100-iris-query"), (R12, "r12-sonybrc300-iris-query"), (R21, "r21-nearusbrc300-iris-query")]),
                boolean: None,
                typed: (
                    crate::types::IrisLevel,
                    { position } => Ok(position)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current shutter speed setting.
        Shutter {
            /// Profile-specific shutter code; the profile's shutter table maps it to an exposure time.
            position: crate::types::ShutterSpeed,
        } => |payload| Ok(InquiryData::Shutter {
            position: crate::types::ShutterSpeed::new(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the current shutter speed setting.
            ShutterInquiry {
                bytes: exposure_wire::SHUTTER_INQUIRY,
                source: source!([(R1, "r1-shutter-query"), (R14, "r14-ptzopticsg3-shutter-query"), (R11, "r11-sonybrch900-shutter-query"), (R8, "r8-sonyevih100-shutter-query"), (R12, "r12-sonybrc300-shutter-query"), (R21, "r21-nearusbrc300-shutter-query")]),
                boolean: None,
                typed: (
                    crate::types::ShutterSpeed,
                    { position } => Ok(position)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current brightness adjustment value.
        Brightness {
            /// Bright position `pq` (`00 00 0p 0q`); every byte is a position.
            position: crate::types::BrightnessLevel,
        } => |payload| Ok(InquiryData::Brightness {
            position: crate::types::BrightnessLevel::new(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the current brightness adjustment value.
            BrightnessInquiry {
                bytes: exposure_wire::BRIGHT_INQUIRY,
                source: source!([(R1, "r1-brightness-query"), (R14, "r14-ptzopticsg3-brightness-query"), (R8, "r8-sonyevih100-brightness-query"), (R12, "r12-sonybrc300-brightness-query"), (R21, "r21-nearusbrc300-brightness-query")]),
                boolean: None,
                typed: (
                    crate::types::BrightnessLevel,
                    { position } => Ok(position)
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
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the current white balance mode.
            WhiteBalanceModeInquiry {
                bytes: white_balance_wire::MODE_INQUIRY,
                source: source!([(R1, "r1-whitebalancemode-query"), (R14, "r14-ptzopticsg3-whitebalancemode-query"), (R7, "r7-sonyfr7-whitebalancemode-query"), (R11, "r11-sonybrch900-whitebalancemode-query"), (R8, "r8-sonyevih100-whitebalancemode-query"), (R12, "r12-sonybrc300-whitebalancemode-query"), (R21, "r21-nearusbrc300-whitebalancemode-query")]),
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
            /// Color-temperature wire step; the target profile converts it to Kelvin.
            temperature: u16,
        } => |payload| Ok(InquiryData::ColorTemperature {
            temperature: u16::from(decode_byte(&payload)?),
        });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Reads color temperature as physical Kelvin.
            ///
            /// `decoder()` uses the built-in PTZOptics step table;
            /// `decoder_for_profile()` uses the selected profile's table.
            ColorTemperatureInquiry {
                bytes: color::TEMPERATURE_INQUIRY,
                source: source!([(R1, "r1-colortemperature-query"), (R14, "r14-ptzopticsg3-colortemperature-query")]),
                boolean: None,
                typed: (
                    crate::types::Kelvin,
                    { temperature } => crate::command::color::decode_color_temperature(temperature)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current red gain value.
        RedChannel {
            /// Red channel absolute gain value.
            gain: crate::types::RgbGain,
        } => |payload| Ok(InquiryData::RedChannel {
            gain: crate::types::RgbGain::new(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the current red gain value.
            RedGainInquiry {
                bytes: color::RED_GAIN_INQUIRY,
                source: source!([(R1, "r1-redgain-query"), (R14, "r14-ptzopticsg3-redgain-query"), (R8, "r8-sonyevih100-redgain-query"), (R12, "r12-sonybrc300-redgain-query"), (R21, "r21-nearusbrc300-redgain-query")]),
                boolean: None,
                typed: (
                    crate::types::RgbGain,
                    { gain } => Ok(gain)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: Some("CAM_RGainInq shares bytes with RedTuningInquiry; this entry exposes the absolute red-channel gain interpretation."),
            }
        ]

        /// The current blue gain value.
        BlueChannel {
            /// Blue channel absolute gain value.
            gain: crate::types::RgbGain,
        } => |payload| Ok(InquiryData::BlueChannel {
            gain: crate::types::RgbGain::new(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the current blue gain value.
            BlueGainInquiry {
                bytes: color::BLUE_GAIN_INQUIRY,
                source: source!([(R1, "r1-bluegain-query"), (R14, "r14-ptzopticsg3-bluegain-query"), (R8, "r8-sonyevih100-bluegain-query"), (R12, "r12-sonybrc300-bluegain-query"), (R21, "r21-nearusbrc300-bluegain-query")]),
                boolean: None,
                typed: (
                    crate::types::RgbGain,
                    { gain } => Ok(gain)
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
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the current sharpness mode setting.
            SharpnessModeInquiry {
                bytes: image_wire::SHARPNESS_MODE_INQUIRY,
                source: source!([(R1, "r1-sharpnessmode-query"), (R14, "r14-ptzopticsg3-sharpnessmode-query")]),
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
            level: crate::types::SaturationLevel,
        } => |payload| Ok(InquiryData::Saturation {
            level: crate::types::SaturationLevel::new(Nibbles::<4>::try_from(payload)?.zero_extended_nibble()?)?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the current color saturation level.
            SaturationInquiry {
                bytes: color::SATURATION_INQUIRY,
                source: source!([(R1, "r1-saturation-query"), (R14, "r14-ptzopticsg3-saturation-query"), (R8, "r8-sonyevih100-saturation-query")]),
                boolean: None,
                typed: (
                    crate::types::SaturationLevel,
                    { level } => Ok(level)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current hue adjustment value.
        Hue {
            /// Hue value (0x0=0 to 0xE=14).
            hue: crate::types::HueLevel,
        } => |payload| Ok(InquiryData::Hue {
            hue: crate::types::HueLevel::new(Nibbles::<4>::try_from(payload)?.zero_extended_nibble()?)?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the current hue adjustment value.
            HueInquiry {
                bytes: color::HUE_INQUIRY,
                source: source!([(R1, "r1-hue-query"), (R14, "r14-ptzopticsg3-hue-query"), (R8, "r8-sonyevih100-hue-query")]),
                boolean: None,
                typed: (
                    crate::types::HueLevel,
                    { hue } => Ok(hue)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The current gain value.
        Gain {
            /// Checked gain position, retaining both `pq` digits.
            gain: crate::types::GainLevel,
        } => |payload| Ok(InquiryData::Gain {
            gain: crate::types::GainLevel::new(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the current gain value.
            GainInquiry {
                bytes: gain::INQUIRY,
                source: source!([(R1, "r1-gain-query"), (R14, "r14-ptzopticsg3-gain-query"), (R11, "r11-sonybrch900-gain-query"), (R8, "r8-sonyevih100-gain-query"), (R12, "r12-sonybrc300-gain-query"), (R21, "r21-nearusbrc300-gain-query")]),
                boolean: None,
                typed: (
                    crate::types::GainLevel,
                    { gain } => Ok(gain)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: Some("R8 Sony EVI-H100S/H100V Technical Manual, p.35, CAM_GainPosInq 8x 09 04 4C FF; y0 50 00 00 0p 0q FF. https://www.sony.com/electronics/support/res/manuals/AE4U/AE4U1001M.pdf"),
            }
        ]

        /// The current gain limit setting.
        GainLimit {
            /// Maximum gain limit (0x0=0 to 0xF=15).
            limit: crate::types::GainLimit,
        } => |payload| Ok(InquiryData::GainLimit {
            limit: crate::types::GainLimit::new(Nibbles::<1>::try_from(payload)?.value())?,
        });
        shape: crate::PayloadShape::Nibbles(1);
        commands [
            /// Inquiry command to get the current gain limit setting.
            GainLimitInquiry {
                bytes: gain::LIMIT_INQUIRY,
                source: source!([(R1, "r1-gainlimit-query"), (R14, "r14-ptzopticsg3-gainlimit-query")]),
                boolean: None,
                typed: (
                    crate::types::GainLimit,
                    { limit } => Ok(limit)
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
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the backlight compensation mode.
            BacklightInquiry {
                bytes: image_wire::BACKLIGHT_INQUIRY,
                source: source!([(R1, "r1-backlight-query"), (R14, "r14-ptzopticsg3-backlight-query"), (R7, "r7-sonyfr7-backlight-query"), (R11, "r11-sonybrch900-backlight-query"), (R8, "r8-sonyevih100-backlight-query"), (R12, "r12-sonybrc300-backlight-query"), (R21, "r21-nearusbrc300-backlight-query")]),
                boolean: Some(crate::command::image_wire::BacklightCommand::CONVENTION),
                typed: (bool, { status } => Ok(status)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: None,
            }
        ]

        /// The vertical image-flip state, independent of horizontal mirroring.
        ImageFlip {
            /// Vertical flip state; horizontal mirroring is not queried.
            state: bool,
        } => |payload| Ok(InquiryData::ImageFlip {
            state: decode_builtin_bool::<ImageFlipInquiry>(&payload, "vertical image flip")?,
        });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Queries the sourced vertical picture-flip register (`09 04 66`).
            ImageFlipInquiry {
                bytes: flip::VERTICAL_INQUIRY,
                source: source!([(R1, "r1-imageflip-query"), (R14, "r14-ptzopticsg3-imageflip-query"), (R12, "r12-sonybrc300-imageflip-query"), (R21, "r21-nearusbrc300-imageflip-query")]),
                boolean: Some(crate::command::flip::ImageFlip::CONVENTION),
                typed: (bool, { state } => Ok(state)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: Some("CAM_ImgFlipInq 09 04 66: R12 BRC-300 and R21 Nearus; R1 legacy PTZOptics picture flip, and R7/R11 Sony sources."),
            }
        ]

        /// The combined horizontal and vertical flip state.
        FlipState {
            /// Combined horizontal and vertical flip state.
            state: ImageFlipMode,
        } => |payload| Ok(InquiryData::FlipState {
            state: decode_enum::<ImageFlipMode>(&payload)?,
        });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Queries the PTZOptics combined flip register (`09 04 A4`).
            FlipStateInquiry {
                bytes: image_wire::FLIP_COMBINED_INQUIRY,
                source: source!([(R1, "r1-flipstate-query"), (R14, "r14-ptzopticsg3-flipstate-query")]),
                boolean: None,
                typed: (ImageFlipMode, { state } => Ok(state)),
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
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the 2D noise reduction mode.
            NoiseReduction2DModeInquiry {
                bytes: image_wire::NOISE_REDUCTION_2D_MODE_INQUIRY,
                source: source!([(R1, "r1-noisereduction2dmode-query"), (R14, "r14-ptzopticsg3-noisereduction2dmode-query")]),
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
            level: crate::command::NoiseReduction2D,
        } => |payload| Ok(InquiryData::NoiseReduction2D {
            level: crate::command::NoiseReduction2D::from_wire(decode_byte(&payload)?)?,
        });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the 2D noise reduction level.
            NoiseReduction2DInquiry {
                bytes: image_wire::NOISE_REDUCTION_2D_INQUIRY,
                source: source!([(R1, "r1-noisereduction2d-query"), (R14, "r14-ptzopticsg3-noisereduction2d-query"), (R8, "r8-sonyevih100-noisereduction2d-query")]),
                boolean: None,
                typed: (
                    crate::command::NoiseReduction2D,
                    { level } => Ok(level)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
                rationale: Some("The PTZOptics inquiry table identifies 04 53 as the 2D noise-reduction level register."),
            }
        ]

        /// The 3D noise reduction level.
        NoiseReduction3D {
            /// 3D noise reduction level.
            level: crate::command::NoiseReduction3D,
        } => |payload| Ok(InquiryData::NoiseReduction3D {
            level: crate::command::NoiseReduction3D::from_wire(decode_byte(&payload)?)?,
        });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the 3D noise reduction level.
            ///
            /// Every decoder accepts Off (`0`) or a nonzero level through `8`. This
            /// keeps well-formed readback values symmetric with the separately
            /// documented `04 54` setter domain across PTZOptics profiles (#717).
            NoiseReduction3DInquiry {
                bytes: image_wire::NOISE_REDUCTION_3D_INQUIRY,
                source: source!([(R1, "r1-noisereduction3d-query"), (R14, "r14-ptzopticsg3-noisereduction3d-query")]),
                boolean: None,
                typed: (
                    crate::command::NoiseReduction3D,
                    { level } => Ok(level)
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
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the current focus zone selection.
            FocusZoneInquiry {
                bytes: focus_wire::ZONE_INQUIRY,
                source: source!([(B1, "b1-focuszone-query")]),
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
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the auto-focus sensitivity setting.
            AutoFocusSensitivityInquiry {
                bytes: focus_wire::AF_SENSITIVITY_INQUIRY,
                source: source!([(R1, "r1-autofocussensitivity-query"), (R14, "r14-ptzopticsg3-autofocussensitivity-query"), (R8, "r8-sonyevih100-autofocussensitivity-query")]),
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
            position: crate::types::FocusPosition,
        } => |payload| Ok(InquiryData::FocusNearLimit {
            position: crate::types::FocusPosition::new(Nibbles::<4>::try_from(payload)?.value())?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the focus near limit position.
            FocusNearLimitInquiry {
                bytes: focus_wire::NEAR_LIMIT_INQUIRY,
                source: source!([(R8, "r8-sonyevih100-focusnearlimit-query")]),
                boolean: None,
                typed: (
                    crate::types::FocusPosition,
                    { position } => Ok(position)
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
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the current focus mode.
            FocusModeInquiry {
                bytes: focus_wire::MODE_INQUIRY,
                source: source!([(R1, "r1-focusmode-query"), (R14, "r14-ptzopticsg3-focusmode-query"), (R7, "r7-sonyfr7-focusmode-query"), (R11, "r11-sonybrch900-focusmode-query"), (R8, "r8-sonyevih100-focusmode-query"), (R12, "r12-sonybrc300-focusmode-query"), (R21, "r21-nearusbrc300-focusmode-query")]),
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
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the menu open/close status.
            MenuOpenCloseInquiry {
                bytes: menu_wire::MENU_INQUIRY,
                source: source!([(R1, "r1-menuopenclose-query"), (R14, "r14-ptzopticsg3-menuopenclose-query"), (R7, "r7-sonyfr7-menuopenclose-query"), (R11, "r11-sonybrch900-menuopenclose-query")]),
                boolean: Some(crate::command::menu_wire::SetMenuDisplay::CONVENTION),
                typed: (bool, { is_open } => Ok(is_open)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("PTZOptics menu inquiries use category 0x06 to match menu command bytes."),
            }
        ]




        /// The Sony FR7 variable ND-filter density.
        NdFilter {
            /// Variable-ND code, `00` (1/4) through `14` (1/128).
            position: NdFilterDensity,
        } => |payload| Ok(InquiryData::NdFilter {
            position: NdFilterDensity::new(u16::from(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?))
                .map_err(|error| Error::invalid_response(error.to_string(), payload.as_slice().to_vec()))?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Queries the sourced FR7 variable-ND density register.
            NdFilterInquiry {
                bytes: nd_filter_wire::POSITION_INQUIRY,
                source: source!([(R7, "r7-sonyfr7-ndfilter-query")]),
                boolean: None,
                typed: (NdFilterDensity, { position } => Ok(position)),
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
            effect: crate::command::resolution::decode_picture_effect_code(decode_byte(&payload)?)?,
        });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the current picture effect mode.
            PictureEffectInquiry {
                bytes: image_wire::PICTURE_EFFECT_INQUIRY,
                source: source!([(R1, "r1-pictureeffect-query"), (R14, "r14-ptzopticsg3-pictureeffect-query")]),
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
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the auto white balance sensitivity setting.
            AutoWhiteBalanceSensitivityInquiry {
                bytes: white_balance_wire::AWB_SENSITIVITY_INQUIRY,
                source: source!([(R1, "r1-autowhitebalancesensitivity-query"), (R14, "r14-ptzopticsg3-autowhitebalancesensitivity-query")]),
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

        /// The red channel tuning level.
        RedTuning {
            /// Red channel tuning level (-10 to +10).
            level: RgbTuning,
        } => |payload| Ok(InquiryData::RedTuning {
            level: RgbTuning::from_wire(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the red channel tuning level.
            RedTuningInquiry {
                bytes: color::RED_GAIN_INQUIRY,
                source: source!([(R1, "r1-redtuning-query"), (R14, "r14-ptzopticsg3-redtuning-query")]),
                boolean: None,
                typed: (
                    RgbTuning,
                    { level } => Ok(level)
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
            level: RgbTuning,
        } => |payload| Ok(InquiryData::BlueTuning {
            level: RgbTuning::from_wire(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the blue channel tuning level.
            BlueTuningInquiry {
                bytes: color::BLUE_GAIN_INQUIRY,
                source: source!([(R1, "r1-bluetuning-query"), (R14, "r14-ptzopticsg3-bluetuning-query")]),
                boolean: None,
                typed: (
                    RgbTuning,
                    { level } => Ok(level)
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
            value: crate::types::GammaLevel,
        } => |payload| Ok(InquiryData::Gamma {
            value: crate::types::GammaLevel::new(decode_byte(&payload)?)?,
        });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the current gamma curve setting.
            GammaInquiry {
                bytes: image_wire::GAMMA_INQUIRY,
                source: source!([(R1, "r1-gamma-query"), (R14, "r14-ptzopticsg3-gamma-query"), (R8, "r8-sonyevih100-gamma-query")]),
                boolean: None,
                typed: (
                    crate::types::GammaLevel,
                    { value } => Ok(value)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]





        /// The current sharpness position.
        SharpnessPosition {
            /// Current sharpness position value (`pq`).
            position: crate::types::SharpnessLevel,
        } => |payload| Ok(InquiryData::SharpnessPosition {
            position: crate::types::SharpnessLevel::new(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the current sharpness position.
            SharpnessPositionInquiry {
                bytes: image_wire::SHARPNESS_INQUIRY,
                source: source!([(R1, "r1-sharpnessposition-query"), (R14, "r14-ptzopticsg3-sharpnessposition-query"), (R7, "r7-sonyfr7-sharpnessposition-query"), (R8, "r8-sonyevih100-sharpnessposition-query")]),
                boolean: None,
                typed: (
                    crate::types::SharpnessLevel,
                    { position } => Ok(position)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: false,
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
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the USB audio state.
            UsbAudioInquiry {
                bytes: streaming_wire::USB_AUDIO,
                source: source!([(R1, "r1-usbaudio-query")]),
                boolean: Some(crate::command::streaming_wire::UsbAudioCommand::CONVENTION),
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
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the ND filter preset setting.
            NdFilterPresetInquiry {
                bytes: nd_filter_wire::PRESET_INQUIRY,
                source: source!([(R7, "r7-sonyfr7-ndfilterpreset-query")]),
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
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the red tally light status.
            TallyRedInquiry {
                bytes: tally_wire::RED_INQUIRY,
                source: source!([(R7, "r7-sonyfr7-tallyred-query")]),
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
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the green tally light status.
            TallyGreenInquiry {
                bytes: tally_wire::GREEN_INQUIRY,
                source: source!([(R7, "r7-sonyfr7-tallygreen-query")]),
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
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Inquiry command to get the current flicker mode setting.
            FlickerModeInquiry {
                bytes: exposure_wire::FLICKER_MODE_INQUIRY,
                source: source!([(R1, "r1-flickermode-query"), (R14, "r14-ptzopticsg3-flickermode-query")]),
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
            level: crate::types::ContrastLevel,
        } => |payload| Ok(InquiryData::Contrast {
            level: crate::types::ContrastLevel::new(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the current contrast level.
            ContrastInquiry {
                bytes: image_wire::CONTRAST_INQUIRY,
                source: source!([(R1, "r1-contrast-query"), (R14, "r14-ptzopticsg3-contrast-query")]),
                boolean: None,
                typed: (
                    crate::types::ContrastLevel,
                    { level } => Ok(level)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]

        /// The current luminance level.
        Luminance {
            /// Current luminance (image processing brightness) level.
            level: crate::types::LuminanceLevel,
        } => |payload| Ok(InquiryData::Luminance {
            level: crate::types::LuminanceLevel::new(Nibbles::<4>::try_from(payload)?.zero_extended_u8()?)?,
        });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Inquiry command to get the current luminance level.
            LuminanceInquiry {
                bytes: image_wire::LUMINANCE_INQUIRY,
                source: source!([(R1, "r1-luminance-query"), (R14, "r14-ptzopticsg3-luminance-query")]),
                boolean: None,
                typed: (
                    crate::types::LuminanceLevel,
                    { level } => Ok(level)
                ),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: None,
            }
        ]
        /// Sony FR7 ae level (R7 p. 21).
        Fr7AeLevel { /// Source-backed typed readback.
            value: crate::types::AeLevel,
        } => |payload| Ok(InquiryData::Fr7AeLevel { value: crate::command::sony_fr7_exposure::decode_ae_level(payload)? });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Reads Sony FR7 ae level (R7 p. 21).
            Fr7AeLevelInquiry {
                bytes: exposure_wire::COMPENSATION_INQUIRY,
                source: source!(R7, "r7-fr7aelevelinquiry"),
                boolean: None,
                typed: (crate::types::AeLevel, { value } => Ok(value)),
                query: BuiltinInquiryQuery::AlternateTypedInterpretation { canonical: <ExposureCompensationInquiry as BuiltinInquiryCommandMarker>::METADATA },
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 auto gain (R7 p. 21).
        Fr7AutoGain { /// Source-backed typed readback.
            value: bool,
        } => |payload| Ok(InquiryData::Fr7AutoGain { value: decode_builtin_bool::<Fr7AutoGainInquiry>(&payload, "Fr7AutoGain")? });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Reads Sony FR7 auto gain (R7 p. 21).
            Fr7AutoGainInquiry {
                bytes: sony_fr7_exposure::AUTO_GAIN_INQUIRY,
                source: source!(R7, "r7-fr7autogaininquiry"),
                boolean: Some(crate::command::sony_fr7_exposure::Fr7AutoGain::CONVENTION),
                typed: (bool, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 auto shutter (R7 p. 21).
        Fr7AutoShutter { /// Source-backed typed readback.
            value: bool,
        } => |payload| Ok(InquiryData::Fr7AutoShutter { value: decode_builtin_bool::<Fr7AutoShutterInquiry>(&payload, "Fr7AutoShutter")? });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Reads Sony FR7 auto shutter (R7 p. 21).
            Fr7AutoShutterInquiry {
                bytes: sony_fr7_exposure::AUTO_SHUTTER_INQUIRY,
                source: source!(R7, "r7-fr7autoshutterinquiry"),
                boolean: Some(crate::command::sony_fr7_exposure::Fr7AutoShutter::CONVENTION),
                typed: (bool, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 auto iris (R7 p. 21).
        Fr7AutoIris { /// Source-backed typed readback.
            value: bool,
        } => |payload| Ok(InquiryData::Fr7AutoIris { value: decode_builtin_bool::<Fr7AutoIrisInquiry>(&payload, "Fr7AutoIris")? });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Reads Sony FR7 auto iris (R7 p. 21).
            Fr7AutoIrisInquiry {
                bytes: sony_fr7_exposure::AUTO_IRIS_INQUIRY,
                source: source!(R7, "r7-fr7autoirisinquiry"),
                boolean: Some(crate::command::sony_fr7_exposure::Fr7AutoIris::CONVENTION),
                typed: (bool, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 preset white (R7 p. 21).
        Fr7PresetWhite { /// Source-backed typed readback.
            value: crate::types::Kelvin,
        } => |payload| Ok(InquiryData::Fr7PresetWhite { value: crate::command::sony_fr7_white_balance::decode_white_kelvin(payload)? });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Reads Sony FR7 preset white (R7 p. 21).
            Fr7PresetWhiteInquiry {
                bytes: sony_fr7_white_balance::PRESET_WHITE_INQUIRY,
                source: source!(R7, "r7-fr7presetwhiteinquiry"),
                boolean: None,
                typed: (crate::types::Kelvin, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 memory temperature (R7 p. 21).
        Fr7MemoryColorTemperature { /// Source-backed typed readback.
            value: crate::types::Kelvin,
        } => |payload| Ok(InquiryData::Fr7MemoryColorTemperature { value: crate::command::sony_fr7_white_balance::decode_white_kelvin(payload)? });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Reads Sony FR7 memory temperature (R7 p. 21).
            Fr7MemoryColorTemperatureInquiry {
                bytes: sony_fr7_white_balance::MEMORY_COLOR_TEMPERATURE_INQUIRY,
                source: source!(R7, "r7-fr7memorycolortemperatureinquiry"),
                boolean: None,
                typed: (crate::types::Kelvin, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 memory red gain (R7 p. 21).
        Fr7MemoryRedGain { /// Source-backed typed readback.
            value: crate::types::WhiteBalanceGain,
        } => |payload| Ok(InquiryData::Fr7MemoryRedGain { value: crate::command::sony_fr7_white_balance::decode_white_balance_gain(payload)? });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Reads Sony FR7 memory red gain (R7 p. 21).
            Fr7MemoryRedGainInquiry {
                bytes: sony_fr7_white_balance::MEMORY_RED_GAIN_INQUIRY,
                source: source!(R7, "r7-fr7memoryredgaininquiry"),
                boolean: None,
                typed: (crate::types::WhiteBalanceGain, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 memory blue gain (R7 p. 21).
        Fr7MemoryBlueGain { /// Source-backed typed readback.
            value: crate::types::WhiteBalanceGain,
        } => |payload| Ok(InquiryData::Fr7MemoryBlueGain { value: crate::command::sony_fr7_white_balance::decode_white_balance_gain(payload)? });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Reads Sony FR7 memory blue gain (R7 p. 21).
            Fr7MemoryBlueGainInquiry {
                bytes: sony_fr7_white_balance::MEMORY_BLUE_GAIN_INQUIRY,
                source: source!(R7, "r7-fr7memorybluegaininquiry"),
                boolean: None,
                typed: (crate::types::WhiteBalanceGain, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 memory tint (R7 p. 21).
        Fr7MemoryTint { /// Source-backed typed readback.
            value: crate::types::WhiteBalanceOffset,
        } => |payload| Ok(InquiryData::Fr7MemoryTint { value: crate::command::sony_fr7_white_balance::decode_tint_offset(payload)? });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Reads Sony FR7 memory tint (R7 p. 21).
            Fr7MemoryTintInquiry {
                bytes: sony_fr7_white_balance::MEMORY_TINT_INQUIRY,
                source: source!(R7, "r7-fr7memorytintinquiry"),
                boolean: None,
                typed: (crate::types::WhiteBalanceOffset, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 atw temperature offset (R7 p. 21).
        Fr7AtwColorTemperatureOffset { /// Source-backed typed readback.
            value: crate::types::WhiteBalanceOffset,
        } => |payload| Ok(InquiryData::Fr7AtwColorTemperatureOffset { value: crate::command::sony_fr7_white_balance::decode_tint_offset(payload)? });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Reads Sony FR7 atw temperature offset (R7 p. 21).
            Fr7AtwColorTemperatureOffsetInquiry {
                bytes: sony_fr7_white_balance::ATW_COLOR_TEMPERATURE_OFFSET_INQUIRY,
                source: source!(R7, "r7-fr7atwcolortemperatureoffsetinquiry"),
                boolean: None,
                typed: (crate::types::WhiteBalanceOffset, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 atw tint offset (R7 p. 21).
        Fr7AtwTintOffset { /// Source-backed typed readback.
            value: crate::types::WhiteBalanceOffset,
        } => |payload| Ok(InquiryData::Fr7AtwTintOffset { value: crate::command::sony_fr7_white_balance::decode_tint_offset(payload)? });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Reads Sony FR7 atw tint offset (R7 p. 21).
            Fr7AtwTintOffsetInquiry {
                bytes: sony_fr7_white_balance::ATW_TINT_OFFSET_INQUIRY,
                source: source!(R7, "r7-fr7atwtintoffsetinquiry"),
                boolean: None,
                typed: (crate::types::WhiteBalanceOffset, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 master black (R7 p. 21).
        Fr7MasterBlack { /// Source-backed typed readback.
            value: crate::types::BlackLevel,
        } => |payload| Ok(InquiryData::Fr7MasterBlack { value: crate::command::sony_fr7_image::decode_black_level(payload)? });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Reads Sony FR7 master black (R7 p. 21).
            Fr7MasterBlackInquiry {
                bytes: sony_fr7_image::MASTER_BLACK_INQUIRY,
                source: source!(R7, "r7-fr7masterblackinquiry"),
                boolean: None,
                typed: (crate::types::BlackLevel, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 red black (R7 p. 21).
        Fr7RedBlack { /// Source-backed typed readback.
            value: crate::types::BlackLevel,
        } => |payload| Ok(InquiryData::Fr7RedBlack { value: crate::command::sony_fr7_image::decode_black_level(payload)? });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Reads Sony FR7 red black (R7 p. 21).
            Fr7RedBlackInquiry {
                bytes: sony_fr7_image::RED_BLACK_INQUIRY,
                source: source!(R7, "r7-fr7redblackinquiry"),
                boolean: None,
                typed: (crate::types::BlackLevel, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 blue black (R7 p. 21).
        Fr7BlueBlack { /// Source-backed typed readback.
            value: crate::types::BlackLevel,
        } => |payload| Ok(InquiryData::Fr7BlueBlack { value: crate::command::sony_fr7_image::decode_black_level(payload)? });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Reads Sony FR7 blue black (R7 p. 21).
            Fr7BlueBlackInquiry {
                bytes: sony_fr7_image::BLUE_BLACK_INQUIRY,
                source: source!(R7, "r7-fr7blueblackinquiry"),
                boolean: None,
                typed: (crate::types::BlackLevel, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 detail (R7 p. 21).
        Fr7Detail { /// Source-backed typed readback.
            value: bool,
        } => |payload| Ok(InquiryData::Fr7Detail { value: decode_builtin_bool::<Fr7DetailInquiry>(&payload, "Fr7Detail")? });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Reads Sony FR7 detail (R7 p. 21).
            Fr7DetailInquiry {
                bytes: sony_fr7_image::DETAIL_INQUIRY,
                source: source!(R7, "r7-fr7detailinquiry"),
                boolean: Some(crate::command::sony_fr7_image::Fr7DetailSetting::CONVENTION),
                typed: (bool, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 knee (R7 p. 22).
        Fr7Knee { /// Source-backed typed readback.
            value: bool,
        } => |payload| Ok(InquiryData::Fr7Knee { value: decode_builtin_bool::<Fr7KneeInquiry>(&payload, "Fr7Knee")? });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Reads Sony FR7 knee (R7 p. 22).
            Fr7KneeInquiry {
                bytes: sony_fr7_image::KNEE_INQUIRY,
                source: source!(R7, "r7-fr7kneeinquiry"),
                boolean: Some(crate::command::sony_fr7_image::Fr7KneeSetting::CONVENTION),
                typed: (bool, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 knee mode (R7 p. 22).
        Fr7KneeMode { /// Source-backed typed readback.
            value: crate::command::KneeMode,
        } => |payload| Ok(InquiryData::Fr7KneeMode { value: crate::command::sony_fr7_image::decode_fr7_knee_mode(payload)? });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Reads Sony FR7 knee mode (R7 p. 22).
            Fr7KneeModeInquiry {
                bytes: sony_fr7_image::KNEE_MODE_INQUIRY,
                source: source!(R7, "r7-fr7kneemodeinquiry"),
                boolean: None,
                typed: (crate::command::KneeMode, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 knee slope (R7 p. 22).
        Fr7KneeSlope { /// Source-backed typed readback.
            value: crate::types::KneeSlope,
        } => |payload| Ok(InquiryData::Fr7KneeSlope { value: crate::command::sony_fr7_image::decode_knee_slope(payload)? });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Reads Sony FR7 knee slope (R7 p. 22).
            Fr7KneeSlopeInquiry {
                bytes: sony_fr7_image::KNEE_SLOPE_INQUIRY,
                source: source!(R7, "r7-fr7kneeslopeinquiry"),
                boolean: None,
                typed: (crate::types::KneeSlope, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 knee point (R7 p. 22).
        Fr7KneePoint { /// Source-backed typed readback.
            value: crate::types::KneePoint,
        } => |payload| Ok(InquiryData::Fr7KneePoint { value: crate::command::sony_fr7_image::decode_knee_point(payload)? });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Reads Sony FR7 knee point (R7 p. 22).
            Fr7KneePointInquiry {
                bytes: sony_fr7_image::KNEE_POINT_INQUIRY,
                source: source!(R7, "r7-fr7kneepointinquiry"),
                boolean: None,
                typed: (crate::types::KneePoint, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 knee permission (R7 p. 22).
        Fr7KneePermission { /// Source-backed typed readback.
            value: crate::command::KneePermission,
        } => |payload| Ok(InquiryData::Fr7KneePermission { value: crate::command::sony_fr7_image::decode_fr7_knee_permission(payload)? });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Reads Sony FR7 knee permission (R7 p. 22).
            Fr7KneePermissionInquiry {
                bytes: sony_fr7_image::KNEE_PERMISSION_INQUIRY,
                parameter: { parameter: crate::command::KneeParameter = crate::command::KneeParameter::Setting, byte: to_wire },
                source: source!(R7, "r7-fr7kneepermissioninquiry"),
                boolean: None,
                typed: (crate::command::KneePermission, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 clear image position (R7 p. 22).
        Fr7ClearImageZoomPosition { /// Source-backed typed readback.
            value: crate::types::ClearImageZoomPosition,
        } => |payload| Ok(InquiryData::Fr7ClearImageZoomPosition { value: crate::command::sony_fr7_zoom::decode_clear_image_zoom_position(payload)? });
        shape: crate::PayloadShape::Nibbles(4);
        commands [
            /// Reads Sony FR7 clear image position (R7 p. 22).
            Fr7ClearImageZoomPositionInquiry {
                bytes: zoom_wire::POSITION_INQUIRY,
                source: source!(R7, "r7-fr7clearimagezoompositioninquiry"),
                boolean: None,
                typed: (crate::types::ClearImageZoomPosition, { value } => Ok(value)),
                query: BuiltinInquiryQuery::AlternateTypedInterpretation { canonical: <ZoomPositionInquiry as BuiltinInquiryCommandMarker>::METADATA },
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 speed selection (R7 p. 22).
        PresetSpeedSelection { /// Source-backed typed readback.
            value: crate::command::PresetSpeedSelection,
        } => |payload| Ok(InquiryData::PresetSpeedSelection { value: crate::command::sony_fr7_presets::decode_preset_speed_selection(payload)? });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Reads Sony FR7 speed selection (R7 p. 22).
            Fr7PresetSpeedSelectionInquiry {
                bytes: sony_fr7_presets::PRESET_SPEED_SELECTION_INQUIRY,
                source: source!(R7, "r7-fr7presetspeedselectioninquiry"),
                boolean: None,
                typed: (crate::command::PresetSpeedSelection, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 common speed (R7 p. 22).
        Fr7CommonPresetSpeed { /// Source-backed typed readback.
            value: crate::types::PresetSpeed,
        } => |payload| Ok(InquiryData::Fr7CommonPresetSpeed { value: crate::command::sony_fr7_presets::decode_common_preset_speed(payload)? });
        shape: crate::PayloadShape::Nibbles(2);
        commands [
            /// Reads Sony FR7 common speed (R7 p. 22).
            Fr7CommonPresetSpeedInquiry {
                bytes: sony_fr7_presets::COMMON_PRESET_SPEED_INQUIRY,
                source: source!(R7, "r7-fr7commonpresetspeedinquiry"),
                boolean: None,
                typed: (crate::types::PresetSpeed, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 separate mode (R7 p. 22).
        Fr7SeparatePresetMode { /// Source-backed typed readback.
            value: crate::command::PresetSeparateMode,
        } => |payload| Ok(InquiryData::Fr7SeparatePresetMode { value: crate::command::sony_fr7_presets::decode_separate_preset_mode(payload)? });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Reads Sony FR7 separate mode (R7 p. 22).
            Fr7SeparatePresetModeInquiry {
                bytes: sony_fr7_presets::SEPARATE_PRESET_MODE_INQUIRY,
                parameter: { preset: crate::types::PresetNumber = crate::types::PresetNumber::MIN, byte: value },
                source: source!(R7, "r7-fr7separatepresetmodeinquiry"),
                boolean: None,
                typed: (crate::command::PresetSeparateMode, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 separate speed (R7 p. 22).
        Fr7SeparatePresetSpeed { /// Source-backed typed readback.
            value: crate::types::PresetSpeed,
        } => |payload| Ok(InquiryData::Fr7SeparatePresetSpeed { value: crate::command::sony_fr7_presets::decode_separate_preset_speed(payload)? });
        shape: crate::PayloadShape::Bytes(1);
        commands [
            /// Reads Sony FR7 separate speed (R7 p. 22).
            Fr7SeparatePresetSpeedInquiry {
                bytes: sony_fr7_presets::SEPARATE_PRESET_SPEED_INQUIRY,
                parameter: { preset: crate::types::PresetNumber = crate::types::PresetNumber::MIN, byte: value },
                source: source!(R7, "r7-fr7separatepresetspeedinquiry"),
                boolean: None,
                typed: (crate::types::PresetSpeed, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
            }
        ]

        /// Sony FR7 separate duration (R7 p. 22).
        Fr7SeparatePresetDuration { /// Source-backed typed readback.
            value: std::time::Duration,
        } => |payload| Ok(InquiryData::Fr7SeparatePresetDuration { value: crate::command::sony_fr7_presets::decode_separate_preset_duration(payload)? });
        shape: crate::PayloadShape::Nibbles(3);
        commands [
            /// Reads Sony FR7 separate duration (R7 p. 22).
            Fr7SeparatePresetDurationInquiry {
                bytes: sony_fr7_presets::SEPARATE_PRESET_DURATION_INQUIRY,
                parameter: { preset: crate::types::PresetNumber = crate::types::PresetNumber::MIN, byte: value },
                source: source!(R7, "r7-fr7separatepresetdurationinquiry"),
                boolean: None,
                typed: (std::time::Duration, { value } => Ok(value)),
                query: BuiltinInquiryQuery::Queryable,
                vendor_specific: true,
                rationale: Some("R7 v4 source-backed FR7 value domain and payload grammar."),
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

/// Bound target-selected queries by the longest body their fact can select.
const fn maximum_body(kind: InquiryKind, standard: &'static [u8]) -> &'static [u8] {
    match kind {
        InquiryKind::Power => crate::capabilities::PowerStatusQuery::maximum_body(),
        _ => standard,
    }
}

/// Selects profile-owned packet dialects without consulting built-in identity.
/// Custom requests continue to encode their declared opcode unchanged.
pub(crate) fn body_for_profile(
    kind: InquiryKind,
    standard: &'static [u8],
    profile: &crate::ProfileSpec,
) -> &'static [u8] {
    match (kind, profile.capabilities().power) {
        (InquiryKind::Power, Some(power)) => power.status_query.body(),
        _ => standard,
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

/// Decodes a one-byte code reply through the value enum's `WireValue` table.
///
/// Every discrete code reply decodes through the enum's own discriminants, so
/// an unknown code is [`Error::InvalidResponse`] for every inquiry (#809).
fn decode_enum<T: WireValue<Wire = u8>>(payload: &Payload<'_>) -> Result<T, Error> {
    T::from_wire(decode_byte(payload)?)
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

    builtin_inquiry_type!(
        /// Test the generated argument type without registering a production query.
        ParameterTypeProof, preset: crate::PresetNumber
    );

    #[test]
    fn parameterized_query_type_preserves_its_checked_argument_and_frame_capacity() {
        const QUERY: ParameterTypeProof = ParameterTypeProof::new(crate::PresetNumber::MIN);
        assert_eq!(QUERY.preset(), crate::PresetNumber::MIN);
        assert_eq!(builtin_inquiry_argument_len!(preset), 1);
        let mut buffer = [0; 7];
        let len = write_query_frame(
            CameraId::CAMERA_2,
            &[0x09, 0x7e, 0x04, 0x32],
            Some(QUERY.preset().value()),
            &mut buffer,
        )
        .unwrap();
        assert_eq!(len, buffer.len());
        assert_eq!(
            buffer,
            [
                0x82,
                0x09,
                0x7e,
                0x04,
                0x32,
                crate::PresetNumber::MIN.value(),
                0xff
            ]
        );
        assert!(matches!(
            write_query_frame(
                CameraId::CAMERA_2,
                &[0x09, 0x7e, 0x04, 0x32],
                Some(QUERY.preset().value()),
                &mut [0; 6]
            ),
            Err(Error::BufferTooSmall { .. })
        ));
    }

    #[test]
    fn every_admitted_typed_inquiry_has_source_evidence() {
        let index = crate::source_evidence::checked::Index::parse(include_str!(
            "../../docs/visca_reference.md"
        ))
        .expect("checked positive evidence");
        for profile in inquiry_test_profiles() {
            let model = profile.capabilities().profile_id.expect("builtin identity");
            for meta in BUILTIN_INQUIRIES {
                if (meta.validate)(&profile).is_err() {
                    continue;
                }
                let packet = body_for_profile(meta.kind, meta.bytes, &profile)
                    .iter()
                    .map(|byte| format!("{byte:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let polarity = meta.boolean.map(|convention| match convention {
                    BoolConvention::OnIs02 => "02=on;03=off",
                    BoolConvention::OnIs03 => "03=on;02=off",
                });
                index
                    .validate(meta.source, meta.name, Some(model), Some(&packet), polarity)
                    .unwrap_or_else(|error| panic!("{model:?}: {error}"));
            }
        }
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn brc_h900_power_query_selects_its_documented_extended_category() {
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBrcH900>()
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

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn power_query_dialect_survives_export_and_accepts_custom_selection() {
        use crate::capabilities::PowerStatusQuery;
        const _: () = assert!(
            <PowerInquiry as crate::Request>::MAX_SIZE
                >= crate::command::bytes::frame_len(PowerStatusQuery::maximum_body())
        );
        for id in crate::profiles::ProfileId::all() {
            let mut facts = id.raw_facts();
            let selected = facts
                .capabilities
                .power
                .as_ref()
                .expect("power facts")
                .status_query;
            assert_eq!(
                selected,
                if *id == crate::profiles::ProfileId::SonyBrcH900 {
                    PowerStatusQuery::Extended
                } else {
                    PowerStatusQuery::Standard
                }
            );
            facts.capabilities.profile_id = None;
            for query in [selected, PowerStatusQuery::Extended] {
                facts
                    .capabilities
                    .power
                    .as_mut()
                    .expect("power facts")
                    .status_query = query;
                let profile =
                    crate::ProfileSpec::from_facts(facts.clone()).expect("custom power dialect");
                let target = crate::TargetRegistration::from_profile(
                    crate::target::TargetSlot::CAMERA_1,
                    profile,
                );
                let mut buffer = [0; <PowerInquiry as crate::Request>::MAX_SIZE];
                let written = <PowerInquiry as crate::Request>::write_for_target(
                    &PowerInquiry,
                    &target,
                    &mut buffer,
                    crate::requests::RequestContractAuthority::new(),
                )
                .expect("target-selected power query");
                let expected: &[u8] = match query {
                    PowerStatusQuery::Standard => &[0x81, 0x09, 0x04, 0x00, 0xFF],
                    PowerStatusQuery::Extended => &[0x81, 0x09, 0x7E, 0x04, 0x00, 0xFF],
                };
                assert_eq!(&buffer[..written], expected, "{id:?}");
            }
        }
    }

    #[test]
    fn fr7_variable_nd_query_uses_the_sourced_register_and_payload() {
        use crate::{Inquiry, Request};
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::SonyFr7>().unwrap();
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
        let sony = crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBrc300>().unwrap();
        let nearus =
            crate::ProfileSpec::from_compile_time::<crate::profiles::NearusBrc300>().unwrap();
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
        assert!(vertical);
        let off = ImageFlipInquiry.decoder().decode(&[0x03]).unwrap();
        assert!(!off);
        let both = FlipStateInquiry.decoder().decode(&[0x03]).unwrap();
        assert_eq!(both, ImageFlipMode::Both);
    }

    #[test]
    fn generated_queryable_inquiries_frame_their_bodies_for_all_camera_ids() {
        for camera_num in 1..=8 {
            let camera_id = CameraId::new(camera_num).expect("valid camera id");
            let frames = encode_each_generated_inquiry(camera_id);
            assert_eq!(frames.len(), 71);
            for (name, body, frame) in frames {
                let frame = frame.unwrap_or_else(|error| panic!("{name} must encode: {error:?}"));
                let mut expected = vec![camera_id.to_address_byte()];
                expected.extend_from_slice(&body);
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
        assert_eq!(FUZZ_INQUIRY_KINDS.len(), 71);
        assert_eq!(BUILTIN_INQUIRIES.len(), 71);
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
            let left_bytes = left.bytes;
            for right in BUILTIN_INQUIRIES.iter().skip(i + 1) {
                let right_bytes = right.bytes;
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
                        canonical.bytes(),
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

            {
                let command = meta.command;
                assert_eq!(
                    meta.name,
                    command.name(),
                    "{} command discriminator must match metadata name",
                    meta.name,
                );
                assert_eq!(
                    meta.bytes,
                    command.bytes(),
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
        for (kind, parameter, maximum, centre) in [
            (
                InquiryKind::ExposureCompensation,
                "ExposureCompensationLevel",
                14,
                7,
            ),
            (InquiryKind::RedTuning, "RgbTuning", 20, 10),
            (InquiryKind::BlueTuning, "RgbTuning", 20, 10),
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
                                && expected.contains(&format!(" = {} ", i16::from(raw) - centre))
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
                Response::Inquiry(InquiryData::ExposureCompensation { value }) => value.value(),
                Response::Inquiry(InquiryData::RedTuning { level })
                | Response::Inquiry(InquiryData::BlueTuning { level }) => level.value(),
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
            Ok(Response::Inquiry(InquiryData::GainLimit { limit })) if limit.value() == 0x0f
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
        for (wire, expected) in [
            (0x00, ImageFlipMode::Off),
            (0x01, ImageFlipMode::Horizontal),
            (0x02, ImageFlipMode::Vertical),
            (0x03, ImageFlipMode::Both),
        ] {
            let response = parse_inquiry_payload(&[wire], &InquiryKind::FlipState);
            assert!(
                matches!(
                    &response,
                    Ok(Response::Inquiry(InquiryData::FlipState { state }))
                        if *state == expected
                ),
                "documented flip mode {wire:#04X} must decode as {expected:?}, got {response:?}"
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

    /// Exhaust every structurally valid numeric payload, including all four
    /// nibble words, through the public payload/frame and typed entry points.
    #[test]
    fn checked_numeric_parsers_agree_on_every_payload_in_their_shape() {
        fn check<C: GeneratedInquiry>() {
            let kind = C::KIND;
            let (length, radix) = match base_payload_shape(kind) {
                crate::PayloadShape::Nibbles(length) => (length, 16u32),
                crate::PayloadShape::Bytes(length) => (length, 256u32),
                crate::PayloadShape::Unknown => panic!("numeric inquiry has no shape: {kind:?}"),
            };
            assert!(length <= 4, "numeric sweep must remain bounded");
            let mut bytes = [0u8; 4];
            let mut frame = [0u8; 7];
            frame[0] = 0x90;
            frame[1] = 0x50;
            frame[length + 2] = crate::wire::VISCA_TERMINATOR;
            for code in 0..radix.pow(u32::try_from(length).unwrap()) {
                let mut remaining = code;
                for byte in bytes[..length].iter_mut().rev() {
                    *byte = u8::try_from(remaining % radix).unwrap();
                    remaining /= radix;
                }
                let payload = &bytes[..length];
                frame[2..length + 2].copy_from_slice(payload);
                let public = parse_inquiry_payload(payload, &kind);
                let framed = Response::parse_with_type(&frame[..length + 3], &kind);
                let typed = decode_payload::<C>(payload);
                assert_eq!(public.is_ok(), typed.is_ok(), "{kind:?} {payload:02x?}");
                assert_eq!(
                    framed.is_ok(),
                    typed.is_ok(),
                    "framed {kind:?} {payload:02x?}"
                );
                for error in public
                    .err()
                    .into_iter()
                    .chain(framed.err())
                    .chain(typed.err())
                {
                    assert_eq!(
                        error.kind(),
                        crate::ErrorKind::Protocol,
                        "{kind:?} {payload:02x?}: {error:?}"
                    );
                }
            }
        }
        check::<ZoomPositionInquiry>();
        check::<FocusPositionInquiry>();
        check::<ExposureCompensationInquiry>();
        check::<IrisInquiry>();
        check::<ShutterInquiry>();
        check::<BrightnessInquiry>();
        check::<RedGainInquiry>();
        check::<BlueGainInquiry>();
        check::<SaturationInquiry>();
        check::<HueInquiry>();
        check::<GainInquiry>();
        check::<GainLimitInquiry>();
        check::<NoiseReduction2DInquiry>();
        check::<NoiseReduction3DInquiry>();
        check::<FocusNearLimitInquiry>();
        check::<RedTuningInquiry>();
        check::<BlueTuningInquiry>();
        check::<GammaInquiry>();
        check::<SharpnessPositionInquiry>();
        check::<ContrastInquiry>();
        check::<LuminanceInquiry>();
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
        // BRC-H900 gain codes through 0x19 are representable; the decoder
        // must distinguish that expanded wire domain from corrupt values.
        for (payload, expected) in [([0, 0, 1, 0], 0x10), ([0, 0, 1, 9], 0x19)] {
            assert_eq!(
                GainInquiry.decoder().decode(&payload).unwrap().value(),
                expected
            );
        }
        // Canonical parsing and typed accessors share the checked table values.
        for error in [
            GainInquiry.decoder().decode(&[0, 0, 1, 0x0A]).unwrap_err(),
            IrisInquiry
                .decoder()
                .decode(&[0, 0, 0x0F, 0x0F])
                .unwrap_err(),
            HueInquiry.decoder().decode(&[0, 0, 0, 0x0F]).unwrap_err(),
        ] {
            assert_eq!(error.kind(), crate::ErrorKind::Protocol);
        }
    }

    #[test]
    fn padded_replies_reject_nonzero_leading_nibbles() {
        let pair_kinds = [
            InquiryKind::ExposureCompensation,
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
            matches!(contrast, Err(Error::InvalidResponse { .. })),
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
