//! Datagram receive outcomes and the one truncation-detection policy.
//!
//! # Policy
//!
//! A valid-looking prefix of a datagram is not a valid VISCA response, so a
//! datagram transport never hands bytes to the protocol decoder unless the
//! whole datagram fitted in the caller's buffer:
//!
//! - Built-in UDP transports learn truncation from the operating system
//!   ([`recv_datagram`]): on Unix a one-byte sentinel extends the destination,
//!   on Windows Winsock reports `WSAEMSGSIZE`. An exact-size datagram is
//!   therefore accepted.
//! - A receiver that cannot observe truncation (a custom
//!   [`AsyncDatagram`](crate::transport::async_io::AsyncDatagram) or
//!   [`AsyncTransport`](crate::transport::AsyncTransport) using the default
//!   method) cannot tell an exact fill from a truncated one, so
//!   [`exact_fill_outcome`] conservatively reports a buffer-filling receive as
//!   possibly truncated.
//! - Empty datagrams are skipped without extending the receive's deadline,
//!   because `Ok(0)` is reserved for stream end-of-file
//!   ([`deliverable`]).
//! - A non-complete outcome surfaces as [`Error::ResponseTooLarge`] from the
//!   length-returning receive methods ([`delivered_len`]), which owners treat
//!   as one discarded datagram, never as a dead session.

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
use crate::Error;

/// Result of one transport receive with datagram-boundary information.
///
/// A byte stream always reports [`ReceiveOutcome::Complete`]. Datagram
/// transports must not hand bytes to the protocol decoder unless the outcome
/// is complete: a valid-looking prefix is not a complete datagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "a receive outcome must be classified before its bytes are decoded"]
#[non_exhaustive]
pub enum ReceiveOutcome {
    /// The complete payload was copied into the caller's buffer.
    #[non_exhaustive]
    Complete {
        /// Number of payload bytes copied.
        bytes: usize,
    },
    /// The runtime/OS positively reported that the datagram was truncated.
    #[non_exhaustive]
    Truncated {
        /// Number of payload bytes copied before the tail was discarded.
        copied: usize,
    },
    /// A datagram receiver filled the supplied buffer but cannot report
    /// whether a tail was discarded.
    ///
    /// This is deliberately rejected just like [`Self::Truncated`].
    /// Implementations that can distinguish an exact fit from truncation
    /// should override
    #[cfg_attr(
        feature = "async",
        doc = "[`AsyncTransport::recv_into_with_outcome`](crate::transport::AsyncTransport::recv_into_with_outcome)."
    )]
    #[cfg_attr(not(feature = "async"), doc = "`recv_into_with_outcome`.")]
    #[non_exhaustive]
    PossiblyTruncated {
        /// Number of payload bytes copied.
        copied: usize,
    },
}

#[cfg_attr(
    all(not(feature = "async"), not(test)),
    expect(
        dead_code,
        reason = "the constructors serve custom async transports; the blocking facade does not export the type"
    )
)]
impl ReceiveOutcome {
    /// Reports a complete payload copied into the receive buffer.
    pub const fn complete(bytes: usize) -> Self {
        Self::Complete { bytes }
    }
    /// Reports a payload known to have been truncated.
    pub const fn truncated(copied: usize) -> Self {
        Self::Truncated { copied }
    }
    /// Reports an exact buffer fit whose completeness is unknown.
    pub const fn possibly_truncated(copied: usize) -> Self {
        Self::PossiblyTruncated { copied }
    }

    /// Number of payload bytes copied into the supplied buffer.
    pub const fn copied_len(self) -> usize {
        match self {
            Self::Complete { bytes }
            | Self::Truncated { copied: bytes }
            | Self::PossiblyTruncated { copied: bytes } => bytes,
        }
    }

    /// Whether all payload bytes are known to have been copied.
    pub const fn is_complete(self) -> bool {
        matches!(self, Self::Complete { .. })
    }
}

/// The outcome of a receive that copied `bytes` into a `capacity`-byte buffer
/// without truncation information: an exact fill may have been truncated.
#[cfg(feature = "async")]
pub(crate) const fn exact_fill_outcome(bytes: usize, capacity: usize) -> ReceiveOutcome {
    if bytes == capacity {
        ReceiveOutcome::PossiblyTruncated { copied: bytes }
    } else {
        ReceiveOutcome::Complete { bytes }
    }
}

/// Classify one datagram receive into a `capacity`-byte buffer.
///
/// Returns `None` for an empty datagram, which the caller skips (`Ok(0)` is
/// reserved for stream end-of-file), and the outcome otherwise. A receiver
/// claiming to have copied more than the buffer holds is an error.
#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
pub(crate) fn deliverable(
    outcome: ReceiveOutcome,
    capacity: usize,
) -> Result<Option<ReceiveOutcome>, Error> {
    let copied = outcome.copied_len();
    if copied > capacity {
        return Err(Error::InvalidResponse {
            expected: "datagram receive fitting the supplied buffer".into(),
            actual: copied.to_le_bytes().to_vec(),
        });
    }
    if outcome.is_complete() && copied == 0 {
        Ok(None)
    } else {
        Ok(Some(outcome))
    }
}

/// The byte count a length-returning receive reports for `outcome`.
///
/// A datagram that did not fit is reported as [`Error::ResponseTooLarge`]:
/// the socket already consumed it, and its copied prefix must be discarded.
#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
pub(crate) fn delivered_len(outcome: ReceiveOutcome, capacity: usize) -> Result<usize, Error> {
    match outcome {
        ReceiveOutcome::Complete { bytes } => Ok(bytes),
        ReceiveOutcome::Truncated { .. } | ReceiveOutcome::PossiblyTruncated { .. } => {
            Err(Error::ResponseTooLarge { max_size: capacity })
        }
    }
}

/// Receive one datagram through an OS socket, retaining its truncation flag.
///
/// The caller supplies runtime-specific readiness handling; this performs
/// exactly one receive.
#[cfg(all(
    unix,
    any(
        feature = "blocking",
        feature = "runtime-tokio",
        feature = "runtime-smol"
    )
))]
pub(crate) fn recv_datagram<S>(socket: &S, dst: &mut [u8]) -> std::io::Result<ReceiveOutcome>
where
    S: std::os::fd::AsFd,
{
    // A one-byte stack sentinel extends the destination: a datagram that
    // reaches it was larger than `dst`.
    let socket = socket2::SockRef::from(socket);
    let capacity = dst.len();
    let mut sentinel = [0_u8; 1];
    let mut buffers = [
        std::io::IoSliceMut::new(dst),
        std::io::IoSliceMut::new(&mut sentinel),
    ];
    let mut socket = &*socket;
    let received = std::io::Read::read_vectored(&mut socket, &mut buffers)?;
    Ok(if received > capacity {
        ReceiveOutcome::Truncated { copied: capacity }
    } else {
        ReceiveOutcome::Complete { bytes: received }
    })
}

/// Winsock's "message too long" error for an over-size datagram receive.
#[cfg(all(
    windows,
    any(
        feature = "blocking",
        feature = "runtime-tokio",
        feature = "runtime-smol"
    )
))]
const WSAEMSGSIZE: i32 = 10_040;

/// Windows counterpart of the Unix [`recv_datagram`].
#[cfg(all(
    windows,
    any(
        feature = "blocking",
        feature = "runtime-tokio",
        feature = "runtime-smol"
    )
))]
pub(crate) fn recv_datagram<S>(socket: &S, dst: &mut [u8]) -> std::io::Result<ReceiveOutcome>
where
    S: std::os::windows::io::AsSocket,
{
    let socket = socket2::SockRef::from(socket);
    let mut socket = &*socket;
    // `Socket2`'s plain `Read` preserves WSAEMSGSIZE (its vectored adapter
    // suppresses it). The datagram has already been consumed.
    match std::io::Read::read(&mut socket, dst) {
        Ok(bytes) => Ok(ReceiveOutcome::Complete { bytes }),
        Err(error) if error.raw_os_error() == Some(WSAEMSGSIZE) => {
            Ok(ReceiveOutcome::Truncated { copied: dst.len() })
        }
        Err(error) => Err(error),
    }
}

#[cfg(all(
    test,
    any(
        feature = "blocking",
        feature = "runtime-tokio",
        feature = "runtime-smol"
    )
))]
mod tests {
    use super::*;

    #[test]
    fn empty_complete_datagrams_are_skipped_and_others_delivered() {
        assert_eq!(deliverable(ReceiveOutcome::complete(0), 4).ok(), Some(None));
        for outcome in [
            ReceiveOutcome::complete(3),
            ReceiveOutcome::truncated(0),
            ReceiveOutcome::truncated(4),
            ReceiveOutcome::possibly_truncated(4),
        ] {
            assert_eq!(deliverable(outcome, 4).ok(), Some(Some(outcome)));
        }
        assert!(matches!(
            deliverable(ReceiveOutcome::complete(5), 4),
            Err(Error::InvalidResponse { .. })
        ));
    }

    #[test]
    fn only_complete_outcomes_deliver_bytes() {
        assert_eq!(delivered_len(ReceiveOutcome::complete(3), 4).ok(), Some(3));
        for outcome in [
            ReceiveOutcome::truncated(4),
            ReceiveOutcome::possibly_truncated(4),
        ] {
            assert!(matches!(
                delivered_len(outcome, 4),
                Err(Error::ResponseTooLarge { max_size: 4 })
            ));
        }
    }

    #[cfg(feature = "async")]
    #[test]
    fn an_exact_fill_without_truncation_information_is_possibly_truncated() {
        assert_eq!(exact_fill_outcome(3, 4), ReceiveOutcome::complete(3));
        assert_eq!(
            exact_fill_outcome(4, 4),
            ReceiveOutcome::possibly_truncated(4)
        );
    }
}
