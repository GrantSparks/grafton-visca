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
//!    later ones. UDP binds the unspecified address of the first
//!    resolved address's family and connects to it.
//! 5. The shared socket options are applied
//!    ([`socket_options`](crate::transport::socket_options)).
//!
//! # Errors
//!
//! - Invalid configuration: [`Error::InvalidRequest`]; malformed endpoint or
//!   failed resolution: [`Error::InvalidAddress`].
//! - The budget expired: [`Error::Timeout`] (stage `Session`, certainty
//!   `NotAccepted`).
//! - Every address refused: [`Error::ConnectionFailed`] naming the endpoint
//!   and carrying the last socket error.

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

/// The smallest share of the connect budget one resolved address receives
/// while later addresses are still waiting (capped by the budget left).
pub(crate) const MIN_ADDRESS_ATTEMPT: Duration = Duration::from_millis(250);

/// Sans-I/O plan for connecting a TCP stream to the first reachable resolved
/// address before one deadline.
#[derive(Debug)]
pub(crate) struct TcpAttempts {
    addresses: std::vec::IntoIter<SocketAddr>,
    deadline: Deadline,
    last_error: Option<std::io::Error>,
    expired: bool,
}

impl TcpAttempts {
    pub(crate) fn new(addresses: Vec<SocketAddr>, deadline: Deadline) -> Self {
        Self {
            addresses: addresses.into_iter(),
            deadline,
            last_error: None,
            expired: false,
        }
    }

    /// The next address to try and its share of the remaining budget, or
    /// `None` once the addresses or the budget are exhausted.
    pub(crate) fn next_attempt(&mut self, now: Instant) -> Option<(SocketAddr, Duration)> {
        let remaining = self.deadline.remaining_at(now);
        if remaining.is_zero() {
            self.expired = true;
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

    /// Record a failed attempt. `None` means the attempt ran out of budget.
    pub(crate) fn failed(&mut self, error: Option<std::io::Error>) {
        match error {
            Some(error) => self.last_error = Some(error),
            None => self.expired = true,
        }
    }

    /// The connector's error once no attempt succeeded.
    ///
    /// A spent budget is a connect timeout even when the last attempt also
    /// reported a socket-level `TimedOut`, so expiry has one spelling on every
    /// facade.
    pub(crate) fn into_error(self, endpoint: &str, now: Instant) -> Error {
        if self.expired || self.deadline.remaining_at(now).is_zero() {
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
    fn every_refused_address_is_tried_and_the_last_error_is_kept() {
        let now = Instant::now();
        let deadline = connect_deadline(now, Duration::from_secs(1)).expect("budget");
        let mut attempts = TcpAttempts::new(addresses(), deadline);

        let mut tried = Vec::new();
        let mut budgets = Vec::new();
        while let Some((address, budget)) = attempts.next_attempt(now) {
            budgets.push(budget);
            tried.push(address);
            attempts.failed(Some(ErrorKind::ConnectionRefused.into()));
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
        attempts.failed(Some(ErrorKind::TimedOut.into()));
        assert!(attempts.next_attempt(now + remaining).is_none());
        assert!(matches!(
            attempts.into_error("camera.local:5678", now + remaining),
            Error::Timeout { .. }
        ));

        let mut attempts = TcpAttempts::new(vec![addresses()[0]], deadline);
        attempts.next_attempt(now).expect("first attempt");
        attempts.failed(None);
        assert!(matches!(
            attempts.into_error("camera.local:5678", now),
            Error::Timeout { .. }
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
