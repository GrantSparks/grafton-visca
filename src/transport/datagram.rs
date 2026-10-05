//! Datagram receive outcomes and the one truncation-detection policy.
//!
//! # Policy
//!
//! A valid-looking prefix of a datagram is not a valid VISCA response, so a
//! datagram transport never hands bytes to the protocol decoder unless the
//! whole datagram fitted in the caller's buffer:
//!
//! - Every transport receive reports a [`ReceiveOutcome`]; nothing infers
//!   completeness from a byte count.
//! - Built-in UDP transports learn truncation from the operating system
//!   ([`recv_datagram`]): on Unix a one-byte sentinel extends the destination,
//!   on Windows Winsock reports `WSAEMSGSIZE`. An exact-size datagram is
//!   therefore accepted.
//! - A custom receiver that cannot observe truncation reports a
//!   buffer-filling receive as [`ReceiveOutcome::PossiblyTruncated`].
//! - Empty datagrams are skipped without extending the receive's deadline,
//!   because zero bytes is reserved for stream end-of-file ([`deliverable`]).
//! - The owner discards a datagram whose outcome is not complete as one
//!   malformed input, never as a dead session, on both facades.

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
///
/// # Datagram receive policy
///
/// - Report [`Self::Complete`] only when the whole datagram fitted, and never
///   for zero bytes from a datagram: zero bytes means end of stream. Skip an
///   empty datagram and keep reading within the same deadline.
/// - Report [`Self::Truncated`] when the runtime or OS reported a discarded
///   tail, and [`Self::PossiblyTruncated`] when the datagram filled the buffer
///   and truncation cannot be observed. The owner discards such a datagram as
///   one malformed input and never decodes its prefix.
/// - A transport that reports [`Self::PossiblyTruncated`] for every exact fill
///   needs a [`recv_buffer_size`](crate::transport::BufferConfig::recv_buffer_size)
///   of at least
///   [`MIN_RECV_BUFFER_SIZE`](crate::transport::BufferConfig::MIN_RECV_BUFFER_SIZE)
///   ` + 1`, so that the largest valid reply never fills the buffer and is
///   never discarded.
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
    /// This is deliberately rejected just like [`Self::Truncated`]. A receiver
    /// that can distinguish an exact fit from truncation reports
    /// [`Self::Complete`] or [`Self::Truncated`] instead.
    #[non_exhaustive]
    PossiblyTruncated {
        /// Number of payload bytes copied.
        copied: usize,
    },
}

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

    /// Copies one whole received `message` into `dst` and reports what fitted.
    ///
    /// This is the receive outcome of a transport whose source yields whole
    /// messages (a datagram, or a channel of replies): the message is
    /// [`Self::Complete`] when it fitted and [`Self::Truncated`] when its tail
    /// did not, in which case only the prefix that fitted was copied.
    ///
    /// An empty `message` reports `complete(0)`, which the owner reads as end
    /// of stream: a datagram source must skip an empty datagram rather than
    /// copy it.
    pub fn copy_message(message: &[u8], dst: &mut [u8]) -> Self {
        let copied = message.len().min(dst.len());
        dst[..copied].copy_from_slice(&message[..copied]);
        if copied < message.len() {
            Self::Truncated { copied }
        } else {
            Self::Complete { bytes: copied }
        }
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
    fn a_copied_message_is_complete_only_when_it_fits() {
        let mut dst = [0_u8; 3];
        assert_eq!(
            ReceiveOutcome::copy_message(&[1, 2], &mut dst),
            ReceiveOutcome::complete(2)
        );
        assert_eq!(
            ReceiveOutcome::copy_message(&[1, 2, 3], &mut dst),
            ReceiveOutcome::complete(3)
        );
        assert_eq!(
            ReceiveOutcome::copy_message(&[4, 5, 6, 7], &mut dst),
            ReceiveOutcome::truncated(3)
        );
        assert_eq!(dst, [4, 5, 6]);
    }

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
}
