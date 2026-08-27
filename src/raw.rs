//! Explicit, typed escape hatches for custom VISCA wire messages.
//!
//! The types in this module are the only raw-request surface in the typed
//! owner API. They are deliberately separate from the request-class markers
//! in [`crate::request`] and from the low-level [`crate::command`] extension
//! surface. A raw value is still admitted only through the async camera's
//! `execute`, `inquire`, or `submit` methods (and their blocking projections);
//! it cannot select a lifecycle ID, target, scheduler priority, or completion
//! kind at submission time.
//!
//! Raw constructors validate and own the complete VISCA frame. The first byte
//! must be a valid VISCA camera address and the final byte must be `0xff`.
//! The camera view supplies the authoritative target during preparation, so a
//! frame addressed to another target is rejected rather than being silently
//! rewritten or inspected to infer target identity. Frames up to
//! [`crate::raw::MAX_BYTES`] are accepted. The first `INLINE_BYTES` fit in inline storage;
//! larger custom frames use one bounded heap-backed `SmallVec` allocation and
//! are copied once into the prepared message. Standard built-in request
//! encoding remains inline and allocation-free.
//!
//! Raw policy is explicit and owned by each value. Timeout, retry, and control
//! classes are required constructor arguments (or can be supplied as a
//! [`crate::raw::Policy`] or [`crate::raw::Spec`]); no default, optional metadata, byte-based
//! inference, or caller-selected operation class exists.

use std::fmt;

use smallvec::SmallVec;

/// Policy and validation types used by the raw constructors.
pub use crate::{
    AffectedAxes, ControlClass, InquiryRoute, ResponseDecoder, RetryClass, TimeoutClass,
};

use crate::{
    completion, request, CameraId, Error, Inquiry as InquiryRequest, OperationCommand, Request,
    Result,
};

/// Number of raw frame bytes retained inline without a heap allocation.
///
/// This matches the protocol engine's standard inline message capacity.
pub(crate) const INLINE_BYTES: usize = 32;

/// Maximum raw frame length accepted by the typed escape hatch.
///
/// The bound is shared with the protocol engine. It is intentionally larger
/// than ordinary VISCA messages so vendor extensions remain possible while
/// still making construction and admission memory-bounded.
pub const MAX_BYTES: usize = 1_024;

const VISCA_TERMINATOR: u8 = 0xff;

/// Explicit timeout, retry, and scheduler-control classes for one raw value.
///
/// The owner lowers these semantic classes to its private runtime policy. The
/// public value never exposes a queue position or a lifecycle identifier.
///
/// The [`ControlClass`] here is the raw request's *own* classification, exactly
/// as a built-in's associated constant is: a camera handle's default class
/// still replaces it (unless it is [`ControlClass::Urgent`]), and a
/// per-submission class still replaces it outright.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Policy {
    timeout: TimeoutClass,
    retry: RetryClass,
    control: ControlClass,
}

/// The caller-facing policy specification used to build a raw request.
///
/// `Spec` is intentionally distinct from [`Policy`]: a specification is the
/// value supplied at a construction boundary, while `Policy` is the compact
/// policy retained by a constructed request. Both carry the same three
/// semantic classes and neither exposes a queue position or lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Spec {
    policy: Policy,
}

impl Spec {
    /// Creates an explicit raw-request specification.
    #[must_use]
    pub const fn new(timeout: TimeoutClass, retry: RetryClass, control: ControlClass) -> Self {
        Self {
            policy: Policy::new(timeout, retry, control),
        }
    }

    /// Returns the normalized policy represented by this specification.
    #[must_use]
    pub const fn policy(self) -> Policy {
        self.policy
    }
}

impl From<Policy> for Spec {
    fn from(policy: Policy) -> Self {
        Self { policy }
    }
}

impl From<Spec> for Policy {
    fn from(spec: Spec) -> Self {
        spec.policy
    }
}

impl Policy {
    /// Creates an explicit raw-request policy.
    #[must_use]
    pub const fn new(timeout: TimeoutClass, retry: RetryClass, control: ControlClass) -> Self {
        Self {
            timeout,
            retry,
            control,
        }
    }

    /// Returns the selected timeout class.
    #[must_use]
    pub const fn timeout_class(self) -> TimeoutClass {
        self.timeout
    }

    /// Returns the selected retry class.
    #[must_use]
    pub const fn retry_class(self) -> RetryClass {
        self.retry
    }

    /// Returns the selected scheduler-control class.
    #[must_use]
    pub const fn control_class(self) -> ControlClass {
        self.control
    }
}

/// A bounded owned raw VISCA frame.
///
/// This helper is intentionally private to the public raw request types. It
/// keeps the four request classes structurally distinct, so a plain frame,
/// inquiry, targeted operation, and applied-only operation cannot be
/// interchanged by changing a runtime enum.
#[derive(Clone, PartialEq, Eq)]
struct Wire(SmallVec<[u8; INLINE_BYTES]>);

impl Wire {
    fn new(bytes: impl AsRef<[u8]>) -> Result<Self> {
        let bytes = bytes.as_ref();
        validate_wire(bytes)?;
        Ok(Self(SmallVec::from_slice(bytes)))
    }

    #[must_use]
    fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    fn write_into(&self, buffer: &mut [u8]) -> Result<usize> {
        if buffer.len() < self.len() {
            return Err(Error::BufferTooSmall {
                required: self.len(),
                actual: buffer.len(),
            });
        }
        buffer[..self.len()].copy_from_slice(self.as_bytes());
        Ok(self.len())
    }
}

impl fmt::Debug for Wire {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Wire")
            .field(&self.as_bytes())
            .finish()
    }
}

fn validate_wire(bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::ResponseTooLarge {
            max_size: MAX_BYTES,
        });
    }
    if bytes.len() < 2 {
        return Err(Error::InvalidRequest(
            "raw VISCA frame must contain an address and terminator".into(),
        ));
    }
    if !(0x81..=0x88).contains(&bytes[0]) {
        return Err(Error::InvalidRequest(
            "raw VISCA frame must begin with an address byte in 0x81..=0x88".into(),
        ));
    }
    if bytes.last().copied() != Some(VISCA_TERMINATOR) {
        return Err(Error::InvalidRequest(
            "raw VISCA frame must end with the 0xff terminator".into(),
        ));
    }
    Ok(())
}

fn validate_axes(axes: AffectedAxes) -> Result<()> {
    if axes.is_empty() {
        return Err(Error::InvalidRequest(
            "raw operation affected axes must be non-empty".into(),
        ));
    }
    Ok(())
}

/// A raw plain command with no operation lifecycle handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plain {
    wire: Wire,
    policy: Policy,
}

impl Plain {
    /// Creates a bounded raw plain command with explicit protocol policy.
    pub fn new(
        bytes: impl AsRef<[u8]>,
        timeout: TimeoutClass,
        retry: RetryClass,
        control: ControlClass,
    ) -> Result<Self> {
        Self::with_policy(bytes, Policy::new(timeout, retry, control))
    }

    /// Creates a bounded raw plain command from an explicit policy value.
    pub fn with_policy<P>(bytes: impl AsRef<[u8]>, policy: P) -> Result<Self>
    where
        P: Into<Policy>,
    {
        Ok(Self {
            wire: Wire::new(bytes)?,
            policy: policy.into(),
        })
    }

    /// Returns the exact frame bytes retained by this value.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.wire.as_bytes()
    }

    /// Returns this value's explicit policy.
    #[must_use]
    pub const fn policy(&self) -> Policy {
        self.policy
    }

    /// Returns this value's explicit timeout class.
    #[must_use]
    pub const fn timeout_class(&self) -> TimeoutClass {
        self.policy.timeout_class()
    }

    /// Returns this value's explicit retry class.
    #[must_use]
    pub const fn retry_class(&self) -> RetryClass {
        self.policy.retry_class()
    }

    /// Returns this value's explicit control class.
    #[must_use]
    pub const fn control_class(&self) -> ControlClass {
        self.policy.control_class()
    }
}

impl Request for Plain {
    type Class = request::Plain;

    // The trait retains associated constants for fixed built-in/downstream
    // request types. Raw values override the instance accessors below because
    // their policy is intentionally selected at construction.
    const MAX_SIZE: usize = MAX_BYTES;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
    const RETRY_CLASS: RetryClass = RetryClass::Never;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn timeout_class(&self) -> TimeoutClass {
        self.policy.timeout_class()
    }

    fn retry_class(&self) -> RetryClass {
        self.policy.retry_class()
    }

    fn control_class(&self) -> ControlClass {
        self.policy.control_class()
    }

    fn encoded_size(&self) -> usize {
        self.wire.len()
    }

    fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize> {
        self.wire.write_into(buffer)
    }
}

/// A raw inquiry with an explicit response route and typed decoder.
pub struct Inquiry<R> {
    wire: Wire,
    route: InquiryRoute,
    decoder: ResponseDecoder<R>,
    policy: Policy,
}

impl<R> Inquiry<R> {
    /// Creates a bounded raw inquiry with explicit route, decoder, and policy.
    ///
    /// `ResponseDecoder::with_context` can be used when the decoder needs
    /// immutable per-instance context; the context is moved into this inquiry
    /// and remains outside the protocol engine.
    pub fn new(
        bytes: impl AsRef<[u8]>,
        route: InquiryRoute,
        decoder: ResponseDecoder<R>,
        timeout: TimeoutClass,
        retry: RetryClass,
        control: ControlClass,
    ) -> Result<Self> {
        Self::with_policy(bytes, route, decoder, Policy::new(timeout, retry, control))
    }

    /// Creates a raw inquiry from explicit route, decoder, and policy values.
    pub fn with_policy<P>(
        bytes: impl AsRef<[u8]>,
        route: InquiryRoute,
        decoder: ResponseDecoder<R>,
        policy: P,
    ) -> Result<Self>
    where
        P: Into<Policy>,
    {
        validate_route(route)?;
        Ok(Self {
            wire: Wire::new(bytes)?,
            route,
            decoder,
            policy: policy.into(),
        })
    }

    /// Creates a raw inquiry using an allocation-free function decoder.
    pub fn from_fn(
        bytes: impl AsRef<[u8]>,
        route: InquiryRoute,
        decoder: fn(&[u8]) -> Result<R>,
        timeout: TimeoutClass,
        retry: RetryClass,
        control: ControlClass,
    ) -> Result<Self> {
        Self::new(
            bytes,
            route,
            ResponseDecoder::from_fn(decoder),
            timeout,
            retry,
            control,
        )
    }

    /// Creates a raw inquiry whose decoder owns immutable context.
    pub fn with_context<C>(
        bytes: impl AsRef<[u8]>,
        route: InquiryRoute,
        context: C,
        decoder: fn(&C, &[u8]) -> Result<R>,
        timeout: TimeoutClass,
        retry: RetryClass,
        control: ControlClass,
    ) -> Result<Self>
    where
        C: Send + Sync + 'static,
        R: 'static,
    {
        Self::new(
            bytes,
            route,
            ResponseDecoder::with_context(context, decoder),
            timeout,
            retry,
            control,
        )
    }

    /// Returns the exact frame bytes retained by this value.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.wire.as_bytes()
    }

    /// Returns this inquiry's explicit response-correlation route.
    #[must_use]
    pub const fn route(&self) -> InquiryRoute {
        self.route
    }

    /// Returns this inquiry's explicit policy.
    #[must_use]
    pub const fn policy(&self) -> Policy {
        self.policy
    }

    /// Returns this inquiry's explicit timeout class.
    #[must_use]
    pub const fn timeout_class(&self) -> TimeoutClass {
        self.policy.timeout_class()
    }

    /// Returns this inquiry's explicit retry class.
    #[must_use]
    pub const fn retry_class(&self) -> RetryClass {
        self.policy.retry_class()
    }

    /// Returns this inquiry's explicit control class.
    #[must_use]
    pub const fn control_class(&self) -> ControlClass {
        self.policy.control_class()
    }
}

impl<R> fmt::Debug for Inquiry<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Inquiry")
            .field("wire", &self.wire)
            .field("route", &self.route)
            .field("decoder", &self.decoder)
            .field("policy", &self.policy)
            .finish()
    }
}

impl<R> Clone for Inquiry<R> {
    fn clone(&self) -> Self {
        Self {
            wire: self.wire.clone(),
            route: self.route,
            decoder: self.decoder.clone(),
            policy: self.policy,
        }
    }
}

impl<R> Request for Inquiry<R>
where
    R: Send + 'static,
{
    type Class = request::Inquiry;
    const MAX_SIZE: usize = MAX_BYTES;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Inquiry;
    const RETRY_CLASS: RetryClass = RetryClass::Inquiry;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn timeout_class(&self) -> TimeoutClass {
        self.policy.timeout_class()
    }

    fn retry_class(&self) -> RetryClass {
        self.policy.retry_class()
    }

    fn control_class(&self) -> ControlClass {
        self.policy.control_class()
    }

    fn encoded_size(&self) -> usize {
        self.wire.len()
    }

    fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize> {
        self.wire.write_into(buffer)
    }
}

impl<R> InquiryRequest for Inquiry<R>
where
    R: Send + 'static,
{
    type Response = R;

    fn route(&self) -> InquiryRoute {
        self.route
    }

    fn decoder(&self) -> ResponseDecoder<Self::Response> {
        self.decoder.clone()
    }
}

/// A raw operation with a meaningful physical target and settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Targeted {
    wire: Wire,
    axes: AffectedAxes,
    policy: Policy,
}

impl Targeted {
    /// Creates a bounded raw targeted operation with explicit non-empty axes
    /// and protocol policy.
    pub fn new(
        bytes: impl AsRef<[u8]>,
        axes: AffectedAxes,
        timeout: TimeoutClass,
        retry: RetryClass,
        control: ControlClass,
    ) -> Result<Self> {
        Self::with_policy(bytes, axes, Policy::new(timeout, retry, control))
    }

    /// Creates a raw targeted operation from an explicit policy value.
    pub fn with_policy<P>(bytes: impl AsRef<[u8]>, axes: AffectedAxes, policy: P) -> Result<Self>
    where
        P: Into<Policy>,
    {
        validate_axes(axes)?;
        Ok(Self {
            wire: Wire::new(bytes)?,
            axes,
            policy: policy.into(),
        })
    }

    /// Returns the exact frame bytes retained by this value.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.wire.as_bytes()
    }

    /// Returns this operation's explicit affected axes.
    #[must_use]
    pub const fn affected_axes(&self) -> AffectedAxes {
        self.axes
    }

    /// Returns this operation's explicit policy.
    #[must_use]
    pub const fn policy(&self) -> Policy {
        self.policy
    }

    /// Returns this operation's explicit timeout class.
    #[must_use]
    pub const fn timeout_class(&self) -> TimeoutClass {
        self.policy.timeout_class()
    }

    /// Returns this operation's explicit retry class.
    #[must_use]
    pub const fn retry_class(&self) -> RetryClass {
        self.policy.retry_class()
    }

    /// Returns this operation's explicit control class.
    #[must_use]
    pub const fn control_class(&self) -> ControlClass {
        self.policy.control_class()
    }
}

impl Request for Targeted {
    type Class = request::Operation<completion::Targeted>;
    const MAX_SIZE: usize = MAX_BYTES;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Movement;
    const RETRY_CLASS: RetryClass = RetryClass::Movement;
    const CONTROL_CLASS: ControlClass = ControlClass::User;

    fn timeout_class(&self) -> TimeoutClass {
        self.policy.timeout_class()
    }

    fn retry_class(&self) -> RetryClass {
        self.policy.retry_class()
    }

    fn control_class(&self) -> ControlClass {
        self.policy.control_class()
    }

    fn encoded_size(&self) -> usize {
        self.wire.len()
    }

    fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize> {
        self.wire.write_into(buffer)
    }

    fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<()> {
        if profile.supports_axes(self.axes) {
            Ok(())
        } else {
            Err(Error::FeatureNotSupported {
                feature: "raw targeted operation axes",
            })
        }
    }
}

impl OperationCommand<completion::Targeted> for Targeted {
    fn affected_axes(&self) -> AffectedAxes {
        self.axes
    }
}

/// A raw operation whose terminal protocol application is its only lifecycle
/// observation. It intentionally has no `settled` method or target inference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedOnly {
    wire: Wire,
    axes: AffectedAxes,
    policy: Policy,
}

impl AppliedOnly {
    /// Creates a bounded raw applied-only operation with explicit non-empty
    /// axes and protocol policy.
    pub fn new(
        bytes: impl AsRef<[u8]>,
        axes: AffectedAxes,
        timeout: TimeoutClass,
        retry: RetryClass,
        control: ControlClass,
    ) -> Result<Self> {
        Self::with_policy(bytes, axes, Policy::new(timeout, retry, control))
    }

    /// Creates a raw applied-only operation from an explicit policy value.
    pub fn with_policy<P>(bytes: impl AsRef<[u8]>, axes: AffectedAxes, policy: P) -> Result<Self>
    where
        P: Into<Policy>,
    {
        validate_axes(axes)?;
        Ok(Self {
            wire: Wire::new(bytes)?,
            axes,
            policy: policy.into(),
        })
    }

    /// Returns the exact frame bytes retained by this value.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.wire.as_bytes()
    }

    /// Returns this operation's explicit affected axes.
    #[must_use]
    pub const fn affected_axes(&self) -> AffectedAxes {
        self.axes
    }

    /// Returns this operation's explicit policy.
    #[must_use]
    pub const fn policy(&self) -> Policy {
        self.policy
    }

    /// Returns this operation's explicit timeout class.
    #[must_use]
    pub const fn timeout_class(&self) -> TimeoutClass {
        self.policy.timeout_class()
    }

    /// Returns this operation's explicit retry class.
    #[must_use]
    pub const fn retry_class(&self) -> RetryClass {
        self.policy.retry_class()
    }

    /// Returns this operation's explicit control class.
    #[must_use]
    pub const fn control_class(&self) -> ControlClass {
        self.policy.control_class()
    }
}

impl Request for AppliedOnly {
    type Class = request::Operation<completion::AppliedOnly>;
    const MAX_SIZE: usize = MAX_BYTES;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
    const RETRY_CLASS: RetryClass = RetryClass::Never;
    const CONTROL_CLASS: ControlClass = ControlClass::User;

    fn timeout_class(&self) -> TimeoutClass {
        self.policy.timeout_class()
    }

    fn retry_class(&self) -> RetryClass {
        self.policy.retry_class()
    }

    fn control_class(&self) -> ControlClass {
        self.policy.control_class()
    }

    fn encoded_size(&self) -> usize {
        self.wire.len()
    }

    fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize> {
        self.wire.write_into(buffer)
    }

    fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<()> {
        if profile.supports_axes(self.axes) {
            Ok(())
        } else {
            Err(Error::FeatureNotSupported {
                feature: "raw applied-only operation axes",
            })
        }
    }
}

impl OperationCommand<completion::AppliedOnly> for AppliedOnly {
    fn affected_axes(&self) -> AffectedAxes {
        self.axes
    }
}

fn validate_route(route: InquiryRoute) -> Result<()> {
    // `InquiryRoute::RAW` (identifier zero) is the explicit raw FIFO route;
    // non-zero values are application/built-in route identifiers. Rebuilding
    // non-zero values through the checked constructor keeps the reserved
    // identifier rule in one place and makes the validation visible at the
    // raw boundary.
    if route.is_raw() {
        Ok(())
    } else {
        InquiryRoute::try_custom(route.identifier()).map(|_| ())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{completion, Inquiry as InquiryTrait, OperationCommand, PlainCommand, Request};

    fn assert_plain<T: PlainCommand>() {}
    fn assert_inquiry<T: InquiryTrait>() {}
    fn assert_targeted<T: OperationCommand<completion::Targeted>>() {}
    fn assert_applied<T: OperationCommand<completion::AppliedOnly>>() {}

    fn decode_first(payload: &[u8]) -> Result<u8> {
        payload
            .first()
            .copied()
            .ok_or_else(|| Error::InvalidRequest("empty test payload".into()))
    }

    #[test]
    fn classes_are_structurally_distinct_and_encoding_is_exact() {
        assert_plain::<Plain>();
        assert_inquiry::<Inquiry<u8>>();
        assert_targeted::<Targeted>();
        assert_applied::<AppliedOnly>();

        let command = Plain::new(
            [0x81, 0x01, 0x02, 0xff],
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .expect("valid raw command");
        assert_eq!(command.bytes(), &[0x81, 0x01, 0x02, 0xff]);
        assert_eq!(command.encoded_size(), 4);
        assert_eq!(command.timeout_class(), TimeoutClass::Quick);
        let mut encoded = [0_u8; 4];
        assert_eq!(
            command
                .write_into(CameraId::CAMERA_1, &mut encoded)
                .expect("exact encoding"),
            4
        );
        assert_eq!(encoded, [0x81, 0x01, 0x02, 0xff]);

        let inquiry = Inquiry::new(
            [0x81, 0x09, 0x04, 0xff],
            InquiryRoute::try_custom(7).expect("non-zero route"),
            ResponseDecoder::from_fn(decode_first),
            TimeoutClass::Inquiry,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .expect("valid raw inquiry");
        assert_eq!(inquiry.route(), InquiryRoute::custom(7));
        assert_eq!(inquiry.decoder().decode(&[0x42]).expect("decode"), 0x42);

        let context_inquiry = Inquiry::<u8>::with_context(
            [0x81, 0x09, 0x05, 0xff],
            InquiryRoute::RAW,
            0x10_u8,
            |context, payload| {
                payload
                    .first()
                    .map(|value| value.saturating_add(*context))
                    .ok_or_else(|| Error::InvalidRequest("empty test payload".into()))
            },
            TimeoutClass::Inquiry,
            RetryClass::Inquiry,
            ControlClass::Normal,
        )
        .expect("context decoder");
        assert_eq!(
            context_inquiry
                .decoder()
                .decode(&[0x02])
                .expect("context decode"),
            0x12
        );

        let axes = AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM);
        let targeted = Targeted::new(
            [0x81, 0x01, 0x06, 0xff],
            axes,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        )
        .expect("valid targeted operation");
        assert_eq!(targeted.affected_axes(), axes);

        let applied = AppliedOnly::with_policy(
            [0x81, 0x01, 0x07, 0xff],
            AffectedAxes::ZOOM,
            Policy::new(TimeoutClass::Quick, RetryClass::Never, ControlClass::Urgent),
        )
        .expect("valid applied-only operation");
        assert_eq!(applied.affected_axes(), AffectedAxes::ZOOM);
        assert_eq!(applied.control_class(), ControlClass::Urgent);
    }

    #[test]
    fn construction_rejects_invalid_wire_axes_and_route_specs() {
        assert!(Plain::new(
            [],
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .is_err());
        assert!(Plain::new(
            [0x80, 0x01, 0xff],
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .is_err());
        assert!(Plain::new(
            [0x81, 0x01, 0x00],
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .is_err());
        assert!(Plain::new(
            vec![0x81; MAX_BYTES + 1],
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .is_err());
        assert!(InquiryRoute::try_custom(0).is_err());
        assert!(AffectedAxes::from_bits(0).is_err());
    }

    /// An otherwise-valid frame one byte over the bound is rejected *by the
    /// size branch*.
    ///
    /// The fixture carries a legal address byte and a legal terminator, so no
    /// other rule in `validate_wire` can satisfy the assertion on its behalf;
    /// deleting the `MAX_BYTES` branch fails this test.
    #[test]
    fn oversize_but_otherwise_valid_frame_is_rejected_by_the_size_bound() {
        fn frame(len: usize) -> Vec<u8> {
            let mut bytes = vec![0x00; len];
            bytes[0] = 0x81;
            bytes[len - 1] = VISCA_TERMINATOR;
            bytes
        }

        let oversize = frame(MAX_BYTES + 1);
        assert_eq!(oversize[0], 0x81, "fixture must carry a legal address byte");
        assert_eq!(
            oversize.last().copied(),
            Some(VISCA_TERMINATOR),
            "fixture must carry a legal terminator"
        );

        let error = Plain::new(
            oversize,
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .expect_err("a frame longer than MAX_BYTES must be rejected");
        assert!(
            matches!(
                error,
                Error::ResponseTooLarge {
                    max_size: MAX_BYTES
                }
            ),
            "expected the frame-size bound to reject the frame, got {error:?}"
        );

        // The same fixture at exactly the bound is accepted, so the assertion
        // above cannot pass by way of an unrelated tightening.
        let at_bound = frame(MAX_BYTES);
        let accepted = Plain::new(
            at_bound.clone(),
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .expect("a frame of exactly MAX_BYTES must be accepted");
        assert_eq!(accepted.bytes(), at_bound.as_slice());

        // Every constructor shares `validate_wire`, so the bound holds for the
        // axis- and route-carrying values too.
        assert!(matches!(
            Targeted::new(
                frame(MAX_BYTES + 1),
                AffectedAxes::PAN_TILT,
                TimeoutClass::Movement,
                RetryClass::Movement,
                ControlClass::User,
            )
            .expect_err("oversize targeted frame"),
            Error::ResponseTooLarge {
                max_size: MAX_BYTES
            }
        ));
        assert!(matches!(
            AppliedOnly::new(
                frame(MAX_BYTES + 1),
                AffectedAxes::ZOOM,
                TimeoutClass::Movement,
                RetryClass::Movement,
                ControlClass::User,
            )
            .expect_err("oversize applied-only frame"),
            Error::ResponseTooLarge {
                max_size: MAX_BYTES
            }
        ));
    }

    /// `validate_axes` is reachable and enforced through every public
    /// axis-carrying constructor.
    ///
    /// `AffectedAxes::NONE` and any `BitAnd` of disjoint sets are constructible
    /// without going through the checked constructors, so the raw boundary is
    /// the only thing keeping an empty axis set out of an operation.
    #[test]
    fn axis_carrying_constructors_reject_an_empty_axis_set() {
        const VALID: [u8; 4] = [0x81, 0x01, 0x06, 0xff];

        let disjoint = AffectedAxes::PAN_TILT & AffectedAxes::ZOOM;
        assert!(
            disjoint.is_empty(),
            "fixture assumption: intersecting disjoint sets yields the empty set"
        );

        for (label, axes) in [("NONE", AffectedAxes::NONE), ("BitAnd", disjoint)] {
            let targeted = Targeted::new(
                VALID,
                axes,
                TimeoutClass::Movement,
                RetryClass::Movement,
                ControlClass::User,
            )
            .expect_err("empty axes must not build a targeted operation");
            assert!(
                matches!(&targeted, Error::InvalidRequest(message) if message.contains("non-empty")),
                "{label}: expected a non-empty axes rejection, got {targeted:?}"
            );

            let targeted_with_policy = Targeted::with_policy(
                VALID,
                axes,
                Policy::new(
                    TimeoutClass::Movement,
                    RetryClass::Movement,
                    ControlClass::User,
                ),
            )
            .expect_err("empty axes must not build a targeted operation from a policy");
            assert!(matches!(targeted_with_policy, Error::InvalidRequest(_)));

            let applied = AppliedOnly::new(
                VALID,
                axes,
                TimeoutClass::Movement,
                RetryClass::Movement,
                ControlClass::User,
            )
            .expect_err("empty axes must not build an applied-only operation");
            assert!(
                matches!(&applied, Error::InvalidRequest(message) if message.contains("non-empty")),
                "{label}: expected a non-empty axes rejection, got {applied:?}"
            );

            let applied_with_policy = AppliedOnly::with_policy(
                VALID,
                axes,
                Policy::new(
                    TimeoutClass::Movement,
                    RetryClass::Movement,
                    ControlClass::User,
                ),
            )
            .expect_err("empty axes must not build an applied-only operation from a policy");
            assert!(matches!(applied_with_policy, Error::InvalidRequest(_)));
        }

        // A single named axis still constructs, so the assertions above are
        // about emptiness rather than about the constructors being broken.
        assert!(Targeted::new(
            VALID,
            AffectedAxes::PAN_TILT,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        )
        .is_ok());
    }

    #[test]
    fn owner_target_is_not_inferred_or_rewritten_from_raw_bytes() {
        let command = Plain::new(
            [0x82, 0x01, 0xff],
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .expect("valid frame for another explicit target");
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>()
            .expect("profile");
        let error = crate::prepared::prepare_command(
            &command,
            CameraId::CAMERA_1,
            &profile,
            crate::OperationalTuning::new(),
            crate::prepared::ClassSelection::Request,
        )
        .expect_err("the owner target must reject a mismatched wire address");
        assert!(matches!(error, Error::InvalidRequest(_)));
        assert_eq!(command.bytes(), &[0x82, 0x01, 0xff]);
    }
}
