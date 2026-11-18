//! One checked recovery table covering every public error variant.
#![allow(clippy::expect_used, clippy::panic)]
use super::{Certainty, Error, ErrorKind, FailureContext, FailureStage, OperationId, Recovery};
use std::{borrow::Cow, collections::BTreeSet, error::Error as _, io, sync::Arc, time::Duration};

struct Case {
    variant: &'static str,
    condition: &'static str,
    error: Error,
    kind: ErrorKind,
    context: Option<FailureContext>,
    recovery: Recovery,
}
fn context(stage: FailureStage, certainty: Certainty) -> Option<FailureContext> {
    Some(FailureContext::new(stage, certainty))
}
fn cases() -> Vec<Case> {
    use Certainty::{FailedConclusively, NotAccepted, StillLive, Unconfirmed};
    use FailureStage::{CancellationAttempt, Observation, PreAdmission, Session, Terminal};
    use Recovery::{Abandon, Reconcile, ReopenSession, WaitAgain};
    let capacity = Recovery::Resubmit {
        after: Duration::from_millis(200),
    };
    let timeout = Recovery::Resubmit {
        after: Duration::from_secs(2),
    };
    let transport = Recovery::Resubmit {
        after: Duration::from_millis(50),
    };
    macro_rules! case {
        ($variant:ident, $condition:literal, $error:expr, $kind:ident, $context:expr, $recovery:expr) => {
            Case {
                variant: stringify!($variant),
                condition: $condition,
                error: $error,
                kind: ErrorKind::$kind,
                context: $context,
                recovery: $recovery,
            }
        };
    }
    vec![
        case!(
            ConnectionFailed,
            "",
            Error::connection_failed(
                "camera",
                Arc::new(io::Error::from(io::ErrorKind::ConnectionRefused))
            ),
            IoRefused,
            None,
            Abandon
        ),
        case!(
            CameraNotAddressed,
            "",
            Error::CameraNotAddressed {
                port: "/dev/ttyVISCA0".into(),
                camera: crate::types::IndividualCameraId::CAMERA_2,
                addressed: 1,
            },
            InvalidParameter,
            None,
            Abandon
        ),
        case!(
            SerialChainTooLong,
            "",
            Error::SerialChainTooLong {
                port: "/dev/ttyVISCA0".into(),
                reported: 8,
            },
            InvalidParameter,
            None,
            Abandon
        ),
        case!(
            ConnectionClosed,
            "",
            Error::ConnectionClosed { source: None },
            IoClosed,
            None,
            ReopenSession
        ),
        case!(
            CommandPending,
            "ACK received",
            Error::CommandPending,
            Busy,
            None,
            WaitAgain
        ),
        case!(
            InvalidResponse,
            "",
            Error::InvalidResponse {
                expected: "reply".into(),
                actual: vec![]
            },
            Protocol,
            None,
            Abandon
        ),
        case!(
            FeatureNotSupported,
            "",
            Error::feature_not_supported("feature"),
            Unsupported,
            None,
            Abandon
        ),
        case!(
            Io,
            "other OS error",
            Error::from(io::Error::other("cause")),
            Other,
            None,
            Abandon
        ),
        case!(
            SyntaxError,
            "",
            Error::SyntaxError,
            InvalidParameter,
            None,
            Abandon
        ),
        case!(
            CommandBufferFull,
            "",
            Error::CommandBufferFull,
            BufferFull,
            None,
            capacity
        ),
        case!(
            CommandCancelled,
            "",
            Error::CommandCancelled,
            Cancelled,
            None,
            Abandon
        ),
        case!(NoSocket, "", Error::NoSocket, BufferFull, None, capacity),
        case!(
            CommandNotExecutable,
            "",
            Error::CommandNotExecutable,
            NotExecutable,
            None,
            Abandon
        ),
        case!(
            InvalidResponseFormat,
            "",
            Error::InvalidResponseFormat,
            Protocol,
            None,
            Abandon
        ),
        case!(
            InvalidResponseLength,
            "",
            Error::invalid_response_length(1, &[]),
            Protocol,
            None,
            Abandon
        ),
        case!(
            UnexpectedResponseType,
            "",
            Error::UnexpectedResponseType,
            Protocol,
            None,
            Abandon
        ),
        case!(
            Unknown,
            "",
            Error::Unknown { code: 0x7f },
            Protocol,
            None,
            Abandon
        ),
        case!(
            InvalidRequest,
            "",
            Error::InvalidRequest {
                reason: "request".into()
            },
            InvalidParameter,
            None,
            Abandon
        ),
        case!(
            MessageLengthError,
            "",
            Error::MessageLengthError,
            Protocol,
            None,
            Abandon
        ),
        case!(
            TransportError,
            "no uncertain effect",
            Error::transport_error(io::Error::other("send")),
            Transport,
            None,
            transport
        ),
        case!(
            TransmissionNotSent,
            "first attempt, invalid request",
            Error::TransmissionNotSent {
                source: Arc::new(Error::InvalidRequest {
                    reason: "framing".into()
                }),
                context: FailureContext::new(Terminal, NotAccepted)
            },
            InvalidParameter,
            context(Terminal, NotAccepted),
            Abandon
        ),
        case!(
            TransmissionNotSent,
            "earlier attempt uncertain",
            Error::TransmissionNotSent {
                source: Arc::new(Error::InvalidRequest {
                    reason: "framing".into()
                }),
                context: FailureContext::new(Terminal, Unconfirmed)
            },
            InvalidParameter,
            context(Terminal, Unconfirmed),
            Reconcile
        ),
        case!(
            InquiryFailed,
            "timeout cause",
            Error::InquiryFailed {
                source: Arc::new(Error::io_timeout())
            },
            Timeout,
            context(Terminal, FailedConclusively),
            timeout
        ),
        case!(
            InquiryFailed,
            "command-scoped source",
            Error::InquiryFailed {
                source: Arc::new(Error::command_failed_after_ack(Error::NoSocket))
            },
            Unconfirmed,
            context(Terminal, FailedConclusively),
            Abandon
        ),
        case!(
            InquiryFailed,
            "still-live source",
            Error::InquiryFailed {
                source: Arc::new(Error::observation_timeout(OperationId::from_raw(1)))
            },
            Timeout,
            context(Terminal, FailedConclusively),
            timeout
        ),
        case!(
            InquiryFailed,
            "session ended",
            Error::InquiryFailed {
                source: Arc::new(Error::OwnerTerminated)
            },
            IoClosed,
            context(Terminal, FailedConclusively),
            ReopenSession
        ),
        case!(
            InvalidParameter,
            "",
            Error::invalid_parameter("parameter", "value", "reason"),
            InvalidParameter,
            None,
            Abandon
        ),
        case!(
            BufferTooSmall,
            "",
            Error::buffer_too_small(2, 1),
            InvalidParameter,
            None,
            Abandon
        ),
        case!(
            ParameterOutOfRange,
            "",
            Error::parameter_out_of_range("value", 2, 0, 1),
            InvalidParameter,
            None,
            Abandon
        ),
        case!(
            Timeout,
            "terminal, not accepted",
            Error::timeout(Terminal, NotAccepted),
            Timeout,
            context(Terminal, NotAccepted),
            timeout
        ),
        case!(
            MotionSuperseded,
            "never sent",
            Error::motion_superseded(
                crate::AffectedAxes::ZOOM,
                FailureContext::new(Terminal, NotAccepted)
            ),
            Cancelled,
            context(Terminal, NotAccepted),
            Abandon
        ),
        case!(
            SettlementSuperseded,
            "deliberate observation end",
            Error::settlement_superseded(OperationId::from_raw(1), crate::AffectedAxes::ZOOM),
            Cancelled,
            None,
            Abandon
        ),
        case!(
            SettlementObservationFailed,
            "live session",
            Error::settlement_observation_failed(OperationId::from_raw(1), Error::SyntaxError),
            InvalidParameter,
            context(Observation, Unconfirmed),
            Reconcile
        ),
        case!(
            CancellationObservationFailed,
            "operation still live",
            Error::CancellationObservationFailed {
                source: Box::new(Error::io_timeout())
            },
            Timeout,
            context(CancellationAttempt, StillLive),
            WaitAgain
        ),
        case!(
            ObservationTimeout,
            "",
            Error::observation_timeout(OperationId::from_raw(1)),
            Timeout,
            context(Observation, StillLive),
            WaitAgain
        ),
        case!(
            ControlReserveExhausted,
            "",
            Error::control_reserve_exhausted(crate::IndividualCameraId::CAMERA_1, 1),
            BufferFull,
            None,
            capacity
        ),
        case!(
            InquiryShapeCapacityExhausted,
            "",
            Error::InquiryShapeCapacityExhausted {
                camera: crate::IndividualCameraId::CAMERA_1,
                capacity: 16,
            },
            BufferFull,
            context(PreAdmission, NotAccepted),
            Abandon
        ),
        case!(
            MaxRetriesExceeded,
            "",
            Error::MaxRetriesExceeded,
            Timeout,
            None,
            Abandon
        ),
        case!(
            CancellationNotSupported,
            "",
            Error::CancellationNotSupported,
            Unsupported,
            None,
            Abandon
        ),
        case!(
            InvalidState,
            "",
            Error::InvalidState {
                reason: "state".into()
            },
            NotExecutable,
            None,
            Abandon
        ),
        case!(
            OwnerTerminated,
            "",
            Error::OwnerTerminated,
            IoClosed,
            None,
            ReopenSession
        ),
        case!(
            DiagnosticSubscriberLimit,
            "",
            Error::DiagnosticSubscriberLimit { capacity: 1 },
            Other,
            None,
            Abandon
        ),
        case!(
            RuntimeShutdown,
            "deliberate",
            Error::RuntimeShutdown,
            IoClosed,
            None,
            Abandon
        ),
        case!(
            CancellationUnconfirmed,
            "",
            Error::CancellationUnconfirmed,
            Unconfirmed,
            context(CancellationAttempt, Unconfirmed),
            Reconcile
        ),
        case!(
            UnsequencedCommandUnconfirmed,
            "",
            Error::UnsequencedCommandUnconfirmed,
            Unconfirmed,
            context(Terminal, Unconfirmed),
            Reconcile
        ),
        case!(
            InquiryCorrelationLost,
            "lane latched",
            Error::inquiry_correlation_lost(crate::IndividualCameraId::CAMERA_1),
            NotExecutable,
            context(Terminal, NotAccepted),
            Abandon
        ),
        case!(
            CommandCorrelationLost,
            "lane latched",
            Error::command_correlation_lost(crate::IndividualCameraId::CAMERA_1),
            NotExecutable,
            context(Terminal, NotAccepted),
            Abandon
        ),
        case!(
            CommandFailedAfterAck,
            "",
            Error::command_failed_after_ack(Error::NoSocket),
            Unconfirmed,
            context(Terminal, Unconfirmed),
            Reconcile
        ),
        case!(
            RequestUnconfirmed,
            "later rejection after a lost ACK",
            Error::request_unconfirmed(Error::CommandBufferFull),
            Unconfirmed,
            context(Terminal, Unconfirmed),
            Reconcile
        ),
        case!(
            RuntimeIdentityExhausted,
            "",
            Error::RuntimeIdentityExhausted,
            Other,
            None,
            Abandon
        ),
        case!(
            RuntimeQueueFull,
            "",
            Error::runtime_queue_full(1),
            BufferFull,
            None,
            capacity
        ),
        case!(
            StreamPoisoned,
            "",
            Error::stream_poisoned(Error::from(io::Error::other("wire position"))),
            IoClosed,
            None,
            ReopenSession
        ),
        case!(
            InvalidCameraId,
            "",
            Error::invalid_camera_id(0),
            InvalidParameter,
            None,
            Abandon
        ),
        case!(
            ResponseTooLarge,
            "",
            Error::response_too_large(1),
            Protocol,
            None,
            Abandon
        ),
        case!(
            InvalidAddress,
            "",
            Error::InvalidAddress {
                reason: Cow::Borrowed("address")
            },
            InvalidParameter,
            None,
            Abandon
        ),
        case!(
            UnsupportedTransport,
            "",
            Error::UnsupportedTransport {
                profile: crate::camera::profiles::ProfileId::SonyFr7,
                transport: crate::camera::TransportKind::Tcp
            },
            Unsupported,
            None,
            Abandon
        ),
        case!(
            MissingRuntime,
            "",
            Error::MissingRuntime,
            Unsupported,
            None,
            Abandon
        ),
        case!(
            WithContext,
            "pending command",
            Error::CommandPending.with_context("request"),
            Busy,
            None,
            WaitAgain
        ),
        case!(
            Timeout,
            "inquiry reply",
            Error::timeout(Terminal, FailedConclusively),
            Timeout,
            context(Terminal, FailedConclusively),
            timeout
        ),
        case!(
            Timeout,
            "command outcome unknown",
            Error::timeout(Terminal, Unconfirmed),
            Timeout,
            context(Terminal, Unconfirmed),
            Reconcile
        ),
        case!(
            Timeout,
            "write outcome unknown",
            Error::io_timeout(),
            Timeout,
            context(Session, Unconfirmed),
            Reconcile
        ),
        case!(
            Timeout,
            "cancel attempt deadline",
            Error::timeout(CancellationAttempt, StillLive),
            Timeout,
            context(CancellationAttempt, StillLive),
            WaitAgain
        ),
        case!(
            TransportError,
            "uncertain write",
            Error::transport_error(Error::io_timeout()),
            Transport,
            context(Session, Unconfirmed),
            Reconcile
        ),
        case!(
            MotionSuperseded,
            "earlier attempt uncertain",
            Error::motion_superseded(
                crate::AffectedAxes::ZOOM,
                FailureContext::new(Terminal, Unconfirmed)
            ),
            Cancelled,
            context(Terminal, Unconfirmed),
            Reconcile
        ),
    ]
}

/// Captured from 1a6c49ef before replacing the independent classifications.
/// Each source row is checked bare and through every classification wrapper.
#[test]
fn every_wrapper_preserves_the_characterized_classification() {
    use std::fmt::Write as _;

    let mut rendered = String::new();
    for case in cases() {
        let source = case.error;
        let mut wrapped = vec![
            ("bare".to_owned(), source.clone()),
            (
                "WithContext".to_owned(),
                source.clone().with_context("outer"),
            ),
            (
                "InquiryFailed".to_owned(),
                Error::InquiryFailed {
                    source: Arc::new(source.clone()),
                },
            ),
            (
                "TransportError".to_owned(),
                Error::TransportError {
                    source: Arc::new(source.clone()),
                },
            ),
            (
                "SettlementObservationFailed".to_owned(),
                Error::settlement_observation_failed(OperationId::from_raw(1), source.clone()),
            ),
            (
                "CancellationObservationFailed".to_owned(),
                Error::CancellationObservationFailed {
                    source: Box::new(source.clone()),
                },
            ),
        ];
        for certainty in [
            Certainty::NotAccepted,
            Certainty::FailedConclusively,
            Certainty::StillLive,
            Certainty::Unconfirmed,
        ] {
            wrapped.push((
                format!("TransmissionNotSent/{certainty:?}"),
                Error::TransmissionNotSent {
                    source: Arc::new(source.clone()),
                    context: FailureContext::new(FailureStage::Terminal, certainty),
                },
            ));
        }
        for (wrapper, error) in wrapped {
            writeln!(
                rendered,
                "{}/{}/{wrapper}\t{:?}\t{}\t{:?}\t{:?}",
                case.variant,
                case.condition,
                error.kind(),
                error.requires_new_session(),
                error.failure_context(),
                error.recovery(),
            )
            .expect("string write");
        }
    }
    assert_eq!(
        rendered.lines().collect::<Vec<_>>(),
        include_str!("../testdata/issue_1015_error_classification.txt")
            .lines()
            .collect::<Vec<_>>(),
    );
}

#[test]
fn every_public_variant_has_one_checked_recovery_contract() {
    let source = crate::test_support::source_scan::production_code(include_str!("error.rs"));
    let body = source
        .split_once("pub enum Error {")
        .expect("Error declaration")
        .1
        .split_once("\n}")
        .expect("Error end")
        .0;
    let declared: BTreeSet<_> = body
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("    ")?;
            rest.starts_with(char::is_uppercase).then(|| {
                rest.split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                    .next()
                    .expect("variant")
                    .to_owned()
            })
        })
        .collect();
    let all = cases();
    let covered: BTreeSet<_> = all.iter().map(|case| case.variant.to_owned()).collect();
    assert_eq!(
        covered, declared,
        "every public Error variant needs a recovery table row"
    );
    for case in all {
        assert_eq!(case.error.kind(), case.kind, "{}: kind", case.variant);
        assert_eq!(
            case.error.failure_context(),
            case.context,
            "{}: context",
            case.variant
        );
        assert_eq!(
            case.error.recovery(),
            case.recovery,
            "{} / {}",
            case.variant,
            case.condition
        );
        assert_eq!(
            case.error.clone().with_context("outer").recovery(),
            case.recovery
        );
    }
}

#[test]
fn timeout_certainty_decides_recovery_at_every_stage() {
    for stage in [
        FailureStage::PreAdmission,
        FailureStage::Observation,
        FailureStage::Terminal,
        FailureStage::CancellationAttempt,
        FailureStage::Session,
    ] {
        for (certainty, expected) in [
            (
                Certainty::NotAccepted,
                Recovery::Resubmit {
                    after: Duration::from_secs(2),
                },
            ),
            (
                Certainty::FailedConclusively,
                Recovery::Resubmit {
                    after: Duration::from_secs(2),
                },
            ),
            (Certainty::StillLive, Recovery::WaitAgain),
            (Certainty::Unconfirmed, Recovery::Reconcile),
        ] {
            assert_eq!(Error::timeout(stage, certainty).recovery(), expected);
        }
    }
}

#[cfg(any(feature = "blocking", feature = "async"))]
#[test]
fn cancellation_attempt_source_cannot_change_the_still_live_context() {
    for source in [
        Error::io_timeout(),
        Error::io_timeout().with_context("cancel write"),
        Error::transport_error(Error::io_timeout()),
    ] {
        let error = Error::cancellation_observation_failed(source);
        assert!(matches!(error, Error::CancellationObservationFailed { .. }));
        assert_eq!(
            error.failure_context(),
            context(FailureStage::CancellationAttempt, Certainty::StillLive)
        );
        assert_eq!(error.recovery(), Recovery::WaitAgain);
    }
}

#[cfg(any(feature = "blocking", feature = "async"))]
#[test]
fn caller_deadlines_are_exact_and_query_normalization_preserves_other_failures() {
    for error in [
        Error::observation_timeout(OperationId::from_raw(1)),
        Error::admission_timeout(),
        Error::query_timeout(),
    ] {
        assert!(error.is_caller_deadline());
        assert_eq!(
            error.into_query_timeout().failure_context(),
            Error::query_timeout().failure_context()
        );
    }
    for error in [
        Error::InquiryShapeCapacityExhausted {
            camera: crate::IndividualCameraId::CAMERA_1,
            capacity: 16,
        },
        Error::settlement_superseded(OperationId::from_raw(1), crate::AffectedAxes::ZOOM),
        Error::settlement_observation_failed(OperationId::from_raw(1), Error::io_timeout()),
        Error::motion_superseded(
            crate::AffectedAxes::ZOOM,
            FailureContext::new(FailureStage::PreAdmission, Certainty::NotAccepted),
        ),
        Error::timeout(FailureStage::Observation, Certainty::Unconfirmed),
    ] {
        let before = format!("{error:?}");
        assert!(!error.is_caller_deadline());
        assert_eq!(format!("{:?}", error.into_query_timeout()), before);
    }
}

fn table() -> String {
    let mut output = String::from("<!-- BEGIN GENERATED RECOVERY VERDICTS -->\n| Error / condition | Kind | Stage / certainty | Recovery |\n| --- | --- | --- | --- |\n");
    for case in cases() {
        let name = if case.condition.is_empty() {
            case.variant.to_owned()
        } else {
            format!("{} / {}", case.variant, case.condition)
        };
        let facts = case.context.map_or_else(
            || "—".to_owned(),
            |context| format!("{:?} / {:?}", context.stage, context.certainty),
        );
        let verdict = match case.error.recovery() {
            Recovery::Resubmit { after } => format!("Resubmit after {after:?}"),
            verdict => format!("{verdict:?}"),
        };
        output.push_str(&format!(
            "| `{name}` | `{:?}` | {facts} | `{verdict}` |\n",
            case.error.kind()
        ));
    }
    output.push_str("<!-- END GENERATED RECOVERY VERDICTS -->");
    output
}

fn documented_recovery_rows(document: &str) -> Vec<&str> {
    let start = document
        .find("<!-- BEGIN GENERATED RECOVERY VERDICTS -->")
        .expect("recovery table begin marker");
    let tail = &document[start..];
    let end = tail
        .find("<!-- END GENERATED RECOVERY VERDICTS -->")
        .expect("recovery table end marker")
        + "<!-- END GENERATED RECOVERY VERDICTS -->".len();
    // Git checkouts may use CRLF. Preserve every row's contents while
    // comparing logical lines rather than platform-specific line endings.
    tail[..end].lines().collect()
}

#[test]
fn recovery_documentation_matches_the_exhaustive_table() {
    let expected = table();
    for document in [
        include_str!("../docs/observability_and_recovery.md").to_owned(),
        include_str!("error.rs")
            .lines()
            .filter_map(|line| line.strip_prefix("//! "))
            .collect::<Vec<_>>()
            .join("\n"),
    ] {
        assert_eq!(
            documented_recovery_rows(&document),
            expected.lines().collect::<Vec<_>>(),
            "expected generated section:\n{expected}"
        );
    }
}

#[test]
fn recovery_documentation_accepts_crlf_and_still_detects_changed_rows() {
    let expected = table();
    let crlf_document = format!(
        "Introduction\r\n{}\r\nAfterward",
        expected.replace('\n', "\r\n")
    );
    let expected_rows = expected.lines().collect::<Vec<_>>();
    assert_eq!(documented_recovery_rows(&crlf_document), expected_rows);
    assert_eq!(documented_recovery_rows(&expected), expected_rows);

    let changed_row = crlf_document.replacen("| `ConnectionClosed` |", "| `WrongCondition` |", 1);
    assert_ne!(documented_recovery_rows(&changed_row), expected_rows);
    let changed_spacing =
        crlf_document.replacen("| Error / condition |", "| Error / condition  |", 1);
    assert_ne!(documented_recovery_rows(&changed_spacing), expected_rows);
}

#[test]
fn os_conditions_and_fatal_observation_sources_have_distinct_verdicts() {
    for kind in [io::ErrorKind::TimedOut, io::ErrorKind::WouldBlock] {
        assert_eq!(
            Error::from(io::Error::from(kind)).recovery(),
            Recovery::Resubmit {
                after: Duration::from_secs(2)
            }
        );
    }
    for kind in [
        io::ErrorKind::ConnectionRefused,
        io::ErrorKind::ConnectionReset,
        io::ErrorKind::Interrupted,
    ] {
        assert_eq!(
            Error::from(io::Error::from(kind)).recovery(),
            Recovery::Abandon
        );
    }
    for error in [
        Error::settlement_observation_failed(OperationId::from_raw(1), Error::OwnerTerminated),
        Error::CancellationObservationFailed {
            source: Box::new(Error::OwnerTerminated),
        },
    ] {
        assert_eq!(error.recovery(), Recovery::ReopenSession);
    }
    for case in cases() {
        if case
            .context
            .is_some_and(|context| context.certainty == Certainty::Unconfirmed)
            && !case.error.requires_new_session()
        {
            assert_eq!(case.error.recovery(), Recovery::Reconcile);
        }
    }
}

#[cfg(any(feature = "blocking", feature = "async"))]
#[test]
fn cached_cancellation_keeps_canonical_operation_and_session_outcomes_exact() {
    for source in [
        Error::CancellationUnconfirmed,
        Error::UnsequencedCommandUnconfirmed,
        Error::CancellationNotSupported,
        Error::OwnerTerminated,
        Error::RuntimeShutdown,
        Error::timeout(FailureStage::CancellationAttempt, Certainty::StillLive),
        Error::CancellationObservationFailed {
            source: Box::new(Error::io_timeout()),
        },
    ] {
        let before = format!("{source:?}");
        let result = Error::cancellation_observation_failed(source);
        assert_eq!(format!("{result:?}"), before);
    }
}

#[cfg(any(feature = "blocking", feature = "async"))]
#[test]
fn caller_timeout_constructors_are_replay_safe_in_their_facades() {
    for (error, stage) in [
        (Error::admission_timeout(), FailureStage::PreAdmission),
        (Error::query_timeout(), FailureStage::Observation),
    ] {
        assert_eq!(
            error.failure_context(),
            context(stage, Certainty::NotAccepted)
        );
        assert_eq!(
            error.recovery(),
            Recovery::Resubmit {
                after: Duration::from_secs(2)
            }
        );
    }
}

#[test]
fn inquiry_terminal_context_overrides_arbitrary_nested_source_certainty() {
    for (cause, expected) in [
        (
            Error::io_timeout(),
            Recovery::Resubmit {
                after: Duration::from_secs(2),
            },
        ),
        (
            Error::observation_timeout(OperationId::from_raw(1)),
            Recovery::Resubmit {
                after: Duration::from_secs(2),
            },
        ),
        (
            Error::CommandPending,
            Recovery::Resubmit {
                after: Duration::from_millis(50),
            },
        ),
        (
            Error::command_failed_after_ack(Error::NoSocket),
            Recovery::Abandon,
        ),
        (Error::UnsequencedCommandUnconfirmed, Recovery::Abandon),
        (
            Error::transport_error(Error::io_timeout()),
            Recovery::Resubmit {
                after: Duration::from_millis(50),
            },
        ),
        (Error::OwnerTerminated, Recovery::ReopenSession),
    ] {
        let kind = cause.kind();
        let requires_new_session = cause.requires_new_session();
        let source = Arc::new(cause.with_context("custom transport"));
        let error = Error::InquiryFailed {
            source: source.clone(),
        };
        assert_eq!(
            error.failure_context(),
            context(FailureStage::Terminal, Certainty::FailedConclusively)
        );
        assert_eq!(error.recovery(), expected);
        assert_eq!(error.kind(), kind);
        assert_eq!(error.requires_new_session(), requires_new_session);
        let retained_source = error
            .source()
            .and_then(|source| source.downcast_ref::<Arc<Error>>())
            .expect("inquiry wrapper exposes its original typed source");
        assert!(Arc::ptr_eq(retained_source, &source));
        assert_ne!(error.recovery(), Recovery::Reconcile);
        assert_ne!(error.recovery(), Recovery::WaitAgain);
    }
}

#[test]
fn terminal_wrappers_preserve_permanent_sources_through_context() {
    for source in [
        Error::MaxRetriesExceeded,
        Error::InquiryShapeCapacityExhausted {
            camera: crate::IndividualCameraId::CAMERA_1,
            capacity: 16,
        },
    ] {
        let source = Arc::new(source.with_context("preparation refused"));
        let inquiry = Error::InquiryFailed {
            source: source.clone(),
        };
        assert_eq!(inquiry.recovery(), Recovery::Abandon);
        for (certainty, expected) in [
            (Certainty::NotAccepted, Recovery::Abandon),
            (Certainty::FailedConclusively, Recovery::Abandon),
            (Certainty::StillLive, Recovery::WaitAgain),
            (Certainty::Unconfirmed, Recovery::Reconcile),
        ] {
            let error = Error::TransmissionNotSent {
                source: source.clone(),
                context: FailureContext::new(FailureStage::Terminal, certainty),
            };
            assert_eq!(error.recovery(), expected);
        }
    }
}

fn display_characterization_cases() -> Vec<(&'static str, &'static str, Error)> {
    let mut all: Vec<_> = cases()
        .into_iter()
        .map(|case| (case.variant, case.condition, case.error))
        .collect();
    all.extend([
        (
            "ConnectionClosed",
            "typed source",
            Error::connection_closed(Some(Arc::new(Error::InvalidRequest {
                reason: "connection source".into(),
            }))),
        ),
        (
            "InvalidResponse",
            "nonempty bytes and escaped text",
            Error::InvalidResponse {
                expected: "reply {with braces}\n\"quoted\"".into(),
                actual: vec![0x90, 0, 0xff],
            },
        ),
        (
            "InvalidResponseLength",
            "nonempty payload",
            Error::invalid_response_length(7, &[0, 0x52, 0xff]),
        ),
        (
            "InvalidResponseLength",
            "truncated payload hex",
            Error::invalid_response_length(7, &[0xab; 40]),
        ),
        (
            "ParameterOutOfRange",
            "signed bounds",
            Error::parameter_out_of_range("offset", i32::MIN, i32::MIN, i32::MAX),
        ),
        (
            "WithContext",
            "nested contextual source",
            Error::InvalidAddress {
                reason: "host {name}\n\"quoted\"".into(),
            }
            .with_context("first context")
            .with_context("outer {context}"),
        ),
    ]);
    for code in [0, 1, 15, 255] {
        all.push(("Unknown", "hex padding", Error::Unknown { code }));
    }
    for stage in [
        FailureStage::PreAdmission,
        FailureStage::Observation,
        FailureStage::Terminal,
        FailureStage::CancellationAttempt,
        FailureStage::Session,
    ] {
        all.push((
            "Timeout",
            "stage formatting",
            Error::timeout(stage, Certainty::Unconfirmed),
        ));
        all.push((
            "MotionSuperseded",
            "stage and axis formatting",
            Error::motion_superseded(
                crate::AffectedAxes::PAN_TILT
                    | crate::AffectedAxes::ZOOM
                    | crate::AffectedAxes::FOCUS,
                FailureContext::new(stage, Certainty::NotAccepted),
            ),
        ));
    }
    all
}

fn rendered_display_contract() -> String {
    use std::fmt::Write as _;

    let mut rendered = String::new();
    for (variant, condition, error) in display_characterization_cases() {
        let text = error.to_string();
        // The derived messages do not apply the caller's formatting flags to
        // their complete outer message, including when individual fields use
        // explicit debug/hex formatting.
        assert_eq!(format!("{error:>80}"), text, "{variant}: width");
        assert_eq!(format!("{error:#.8}"), text, "{variant}: precision");
        writeln!(rendered, "{variant}/{condition}\t{text:?}").expect("string write");
    }
    rendered
}

/// Captured from thiserror 2.0.21's derived Display at 1eedf4e0, using
/// Rust 1.98.0 before replacing the derive. The literal golden is independent
/// of the manual formatter and has no regeneration path through it.
#[test]
fn every_error_display_matches_the_prior_derived_literal_golden() {
    assert_eq!(
        rendered_display_contract().lines().collect::<Vec<_>>(),
        include_str!("../testdata/issue_993_error_display_golden.txt")
            .lines()
            .collect::<Vec<_>>(),
    );
}

/// The old-derived capture established these concrete source types. Keep
/// each shared or boxed source at exactly the same layer and address; only
/// ConnectionFailed intentionally projects through its Arc (#993).
#[test]
fn other_error_sources_keep_the_prior_derived_wrapper_identity() {
    for (variant, condition, error) in display_characterization_cases() {
        let exposed = std::error::Error::source(&error);
        match &error {
            Error::ConnectionFailed { .. } => continue,
            Error::Io { source } => {
                let retained = exposed
                    .and_then(|source| source.downcast_ref::<Arc<io::Error>>())
                    .expect("Io keeps its shared io::Error source");
                assert!(std::ptr::eq(retained, source), "{variant}/{condition}");
            }
            Error::ConnectionClosed {
                source: Some(source),
            }
            | Error::TransportError { source }
            | Error::TransmissionNotSent { source, .. }
            | Error::InquiryFailed { source }
            | Error::StreamPoisoned { source } => {
                let retained = exposed
                    .and_then(|source| source.downcast_ref::<Arc<Error>>())
                    .expect("wrapper keeps its shared Error source");
                assert!(std::ptr::eq(retained, source), "{variant}/{condition}");
            }
            Error::SettlementObservationFailed { source, .. }
            | Error::CancellationObservationFailed { source }
            | Error::CommandFailedAfterAck { source }
            | Error::RequestUnconfirmed { source }
            | Error::WithContext { source, .. } => {
                let retained = exposed
                    .and_then(|source| source.downcast_ref::<Box<Error>>())
                    .expect("wrapper keeps its boxed Error source");
                assert!(std::ptr::eq(retained, source), "{variant}/{condition}");
            }
            _ => assert!(exposed.is_none(), "{variant}/{condition}"),
        }
    }
}
