//! The one IP connect pipeline, shared by the blocking, Tokio and smol
//! connectors.
//!
//! Every TCP and UDP connector runs the same steps; only the I/O primitives
//! differ per facade:
//!
//! 1. [`preflight`] validates the [`TransportConfig`] and canonicalizes the
//!    endpoint before any I/O.
//! 2. One `connect_timeout` budget ([`connect_deadline`]) bounds name
//!    resolution *and* connection setup on every facade.
//! 3. Name resolution reports failure through
//!    [`resolved`](crate::transport::address::resolved), so an unresolvable
//!    host is the same [`Error::InvalidAddress`] on every facade.
//! 4. TCP tries each resolved address in order ([`TcpAttempts`]), giving each
//!    an equal share of the remaining budget (at least
//!    [`MIN_ADDRESS_ATTEMPT`]) so an unroutable early address cannot starve
//!    later ones. UDP uses a numeric address as given. For a name resolving
//!    to both address families it selects the first IPv4 address; otherwise
//!    it selects the first address. It binds the unspecified local address
//!    of the selected family. UDP has no handshake, so setup never probes
//!    reachability or falls back after a successful socket connect.
//! 5. Shared socket options ([`socket_options`](crate::transport::socket_options))
//!    are applied before TCP connect, so TTL also governs the handshake. Socket
//!    setup failure advances to the next address in the same attempt plan. UDP
//!    options are set before the first datagram.
//!
//! # Errors
//!
//! - Invalid configuration: [`Error::InvalidRequest`]; malformed endpoint or
//!   failed resolution: [`Error::InvalidAddress`].
//! - The budget expired: [`Error::Timeout`] (stage `Session`, certainty
//!   `NotAccepted`).
//! - Socket creation, binding, connection or socket-option setup failed:
//!   [`Error::ConnectionFailed`] naming the endpoint and carrying the socket
//!   error (the last attempt's error for TCP).

use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

use crate::{
    timeout::Deadline,
    transport::{address::canonicalize_endpoint, builder::TransportConfig},
    Error, Result,
};

/// Validate `config` and canonicalize `address` before any connector I/O.
pub(crate) fn preflight(address: &str, config: &TransportConfig) -> Result<String> {
    config.validate()?;
    canonicalize_endpoint(address, None)
}

/// The single budget for resolving and connecting one endpoint.
pub(crate) fn connect_deadline(now: Instant, connect_timeout: Duration) -> Result<Deadline> {
    Deadline::after(now, connect_timeout, "connect_timeout")
}

/// The error for a connector that could not reach `endpoint`.
pub(crate) fn connection_failed(endpoint: &str, error: std::io::Error) -> Error {
    Error::connection_failed(endpoint.to_owned(), std::sync::Arc::new(error))
}

/// Select one UDP destination from a nonempty resolution result.
///
/// Literal endpoints resolve to one address. A mixed-family name consistently
/// selects its first IPv4 address, independently of resolver family ordering.
pub(crate) fn udp_target(addresses: &[SocketAddr]) -> Result<SocketAddr> {
    addresses
        .iter()
        .copied()
        .find(SocketAddr::is_ipv4)
        .or_else(|| addresses.first().copied())
        .ok_or_else(|| Error::invalid_address("No UDP destination addresses resolved"))
}

/// Attribute a socket-setup failure to the caller's endpoint.
pub(crate) fn socket_setup<T>(endpoint: &str, result: std::io::Result<T>) -> Result<T> {
    result.map_err(|error| connection_failed(endpoint, error))
}

/// The smallest share of the connect budget one resolved address receives
/// while later addresses are still waiting (capped by the budget left).
pub(crate) const MIN_ADDRESS_ATTEMPT: Duration = Duration::from_millis(250);

/// One failed TCP address attempt, independent of the connector facade.
#[derive(Debug)]
pub(crate) enum AttemptFailure {
    ShareExpired,
    Socket(std::io::Error),
}

impl AttemptFailure {
    pub(crate) fn from_socket(error: std::io::Error) -> Self {
        if error.kind() == std::io::ErrorKind::TimedOut {
            Self::ShareExpired
        } else {
            Self::Socket(error)
        }
    }
}

/// Sans-I/O plan for connecting a TCP stream to the first reachable resolved
/// address before one deadline.
#[derive(Debug)]
pub(crate) struct TcpAttempts {
    addresses: std::vec::IntoIter<SocketAddr>,
    deadline: Deadline,
    last_error: Option<std::io::Error>,
}

impl TcpAttempts {
    pub(crate) fn new(addresses: Vec<SocketAddr>, deadline: Deadline) -> Self {
        Self {
            addresses: addresses.into_iter(),
            deadline,
            last_error: None,
        }
    }

    /// The next address to try and its share of the remaining budget, or
    /// `None` once the addresses or the budget are exhausted.
    pub(crate) fn next_attempt(&mut self, now: Instant) -> Option<(SocketAddr, Duration)> {
        let remaining = self.deadline.remaining_at(now);
        if remaining.is_zero() {
            return None;
        }
        let attempts_left = u32::try_from(self.addresses.len()).unwrap_or(u32::MAX);
        let address = self.addresses.next()?;
        let share = if attempts_left > 1 {
            (remaining / attempts_left)
                .max(MIN_ADDRESS_ATTEMPT)
                .min(remaining)
        } else {
            remaining
        };
        Some((address, share))
    }

    /// Record one attempt's verdict without letting an expired share hide a
    /// later address's conclusive socket failure.
    pub(crate) fn failed(&mut self, failure: AttemptFailure) {
        if let AttemptFailure::Socket(error) = failure {
            self.last_error = Some(error);
        }
    }

    /// The connector's error once no attempt succeeded.
    ///
    /// A spent budget is a connect timeout even when the last attempt also
    /// reported a socket-level `TimedOut`, so expiry has one spelling on every
    /// facade.
    pub(crate) fn into_error(self, endpoint: &str, now: Instant) -> Error {
        if self.deadline.remaining_at(now).is_zero() {
            return Error::connect_timeout();
        }
        match self.last_error {
            Some(error) => connection_failed(endpoint, error),
            None => Error::connect_timeout(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::io::ErrorKind;

    use super::*;

    fn addresses() -> Vec<SocketAddr> {
        vec![
            "192.0.2.1:5678".parse().expect("address"),
            "192.0.2.2:5678".parse().expect("address"),
        ]
    }

    #[test]
    fn udp_target_preserves_single_family_order_and_prefers_ipv4_for_mixed_names() {
        let v4a: SocketAddr = "127.0.0.1:1259".parse().expect("test address");
        let v4b: SocketAddr = "127.0.0.2:1259".parse().expect("test address");
        let v6a: SocketAddr = "[::1]:1259".parse().expect("test address");
        let v6b: SocketAddr = "[::2]:1259".parse().expect("test address");
        for (addresses, expected) in [
            (vec![v6a, v4a, v6b, v4b], v4a),
            (vec![v4b, v6a, v4a], v4b),
            (vec![v4a, v4b], v4a),
            (vec![v6a, v6b], v6a),
            (vec![v6a], v6a),
        ] {
            assert_eq!(udp_target(&addresses).expect("UDP target"), expected);
        }
        assert!(matches!(udp_target(&[]), Err(Error::InvalidAddress { .. })));
    }

    #[test]
    fn socket_setup_keeps_the_endpoint_and_original_socket_error() {
        let result = socket_setup::<()>(
            "camera.local:1259",
            Err(std::io::Error::new(
                ErrorKind::PermissionDenied,
                "setup refused",
            )),
        );
        assert!(
            matches!(result, Err(Error::ConnectionFailed { addr, source })
            if addr == "camera.local:1259"
                && source.kind() == ErrorKind::PermissionDenied
                && source.to_string() == "setup refused")
        );
    }

    #[test]
    fn every_refused_address_is_tried_and_the_last_error_is_kept() {
        let now = Instant::now();
        let deadline = connect_deadline(now, Duration::from_secs(1)).expect("budget");
        let mut attempts = TcpAttempts::new(addresses(), deadline);

        let mut tried = Vec::new();
        let mut budgets = Vec::new();
        while let Some((address, budget)) = attempts.next_attempt(now) {
            budgets.push(budget);
            tried.push(address);
            attempts.failed(AttemptFailure::Socket(ErrorKind::ConnectionRefused.into()));
        }
        assert_eq!(tried, addresses());
        // The first address gets an equal share; the last gets what is left.
        assert_eq!(
            budgets,
            [Duration::from_millis(500), Duration::from_secs(1)]
        );
        assert!(matches!(
            attempts.into_error("camera.local:5678", now),
            Error::ConnectionFailed { ref addr, ref source }
                if addr == "camera.local:5678" && source.kind() == ErrorKind::ConnectionRefused
        ));
    }

    #[test]
    fn a_spent_budget_is_a_connect_timeout_on_every_facade() {
        let now = Instant::now();
        let deadline = connect_deadline(now, Duration::from_millis(5)).expect("budget");
        let mut attempts = TcpAttempts::new(vec![addresses()[0]], deadline);
        let (_, remaining) = attempts.next_attempt(now).expect("first attempt");
        // A blocking connect reports its own TimedOut; an async timer drops
        // the attempt. Both spend the budget and end the same way.
        attempts.failed(AttemptFailure::from_socket(ErrorKind::TimedOut.into()));
        assert!(attempts.next_attempt(now + remaining).is_none());
        assert!(matches!(
            attempts.into_error("camera.local:5678", now + remaining),
            Error::Timeout { .. }
        ));

        let mut attempts = TcpAttempts::new(vec![addresses()[0]], deadline);
        attempts.next_attempt(now).expect("first attempt");
        attempts.failed(AttemptFailure::ShareExpired);
        assert!(matches!(
            attempts.into_error("camera.local:5678", now),
            Error::Timeout { .. }
        ));
    }

    #[test]
    fn address_share_expiry_never_overwrites_a_conclusive_socket_failure() {
        let now = Instant::now();
        let deadline = connect_deadline(now, Duration::from_secs(2)).expect("budget");
        for failures in [
            [
                AttemptFailure::ShareExpired,
                AttemptFailure::Socket(ErrorKind::ConnectionRefused.into()),
            ],
            [
                AttemptFailure::Socket(ErrorKind::ConnectionRefused.into()),
                AttemptFailure::ShareExpired,
            ],
        ] {
            let mut attempts = TcpAttempts::new(addresses(), deadline);
            for failure in failures {
                attempts.next_attempt(now).expect("address remains");
                attempts.failed(failure);
            }
            assert!(attempts.next_attempt(now).is_none());
            assert!(matches!(attempts.into_error("camera.test:5678", now),
                Error::ConnectionFailed { source, .. } if source.kind() == ErrorKind::ConnectionRefused));
        }
    }

    #[test]
    fn only_expired_address_shares_are_a_timeout_even_before_the_total_deadline() {
        let now = Instant::now();
        let deadline = connect_deadline(now, Duration::from_secs(2)).expect("budget");
        let mut attempts = TcpAttempts::new(addresses(), deadline);
        while attempts.next_attempt(now).is_some() {
            attempts.failed(AttemptFailure::ShareExpired);
        }
        assert!(matches!(
            attempts.into_error("camera.test:5678", now),
            Error::Timeout { .. }
        ));
        assert!(matches!(
            AttemptFailure::from_socket(ErrorKind::TimedOut.into()),
            AttemptFailure::ShareExpired
        ));
    }

    #[test]
    fn a_short_budget_still_gives_each_waiting_address_the_minimum_share() {
        let now = Instant::now();
        let deadline = connect_deadline(now, Duration::from_millis(400)).expect("budget");
        let mut attempts = TcpAttempts::new(
            vec![addresses()[0], addresses()[1], addresses()[0]],
            deadline,
        );
        assert_eq!(
            attempts.next_attempt(now).map(|(_, budget)| budget),
            Some(MIN_ADDRESS_ATTEMPT)
        );
    }

    #[test]
    fn an_unrepresentable_connect_timeout_is_rejected() {
        assert!(matches!(
            connect_deadline(Instant::now(), Duration::MAX),
            Err(Error::InvalidParameter {
                parameter: "connect_timeout",
                ..
            })
        ));
    }
}
