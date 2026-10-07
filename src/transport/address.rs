//! Common address resolution utilities for transport implementations.
//!
//! This module provides a unified way to resolve network addresses across
//! different transport types, eliminating code duplication.

use std::borrow::Cow;
use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr},
    num::NonZeroU16,
};

use crate::Error;

/// Endpoint parsed from caller input before a default port is applied.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedEndpoint {
    host: Host,
    port: Option<NonZeroU16>,
}

/// Host value separated from the optional port.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Host {
    Ip(IpAddr),
    Dns(Box<str>),
}

/// Fully specified endpoint ready for DNS resolution or socket APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct NetworkEndpoint {
    host: Host,
    port: NonZeroU16,
}

impl ParsedEndpoint {
    fn parse(address: &str) -> Result<Self, Error> {
        let address = address.trim();

        if address.is_empty() {
            return Err(invalid_address("Empty address"));
        }

        if address.starts_with('[') {
            return parse_bracketed_ipv6(address);
        }

        if address.contains('[') || address.contains(']') {
            return Err(invalid_address("Malformed IPv6 brackets"));
        }

        if let Ok(ip) = address.parse::<IpAddr>() {
            return match ip {
                IpAddr::V4(ip) => Ok(Self {
                    host: Host::Ip(IpAddr::V4(ip)),
                    port: None,
                }),
                IpAddr::V6(_) => Err(invalid_address("Bare IPv6 literals must be bracketed")),
            };
        }

        if let Ok(socket_addr) = address.parse::<SocketAddr>() {
            return Ok(Self {
                host: Host::Ip(socket_addr.ip()),
                port: Some(nonzero_port(socket_addr.port())?),
            });
        }

        match address.chars().filter(|&c| c == ':').count() {
            0 => Ok(Self {
                host: parse_dns_host(address)?,
                port: None,
            }),
            1 => {
                let (host_part, port_part) = address
                    .split_once(':')
                    .ok_or_else(|| invalid_address("Missing port separator"))?;

                Ok(Self {
                    host: parse_dns_host(host_part)?,
                    port: Some(parse_port(port_part)?),
                })
            }
            _ => Err(invalid_address(
                "Multi-colon input is not a bracketed IPv6 endpoint",
            )),
        }
    }

    fn with_default_port(self, default_port: Option<u16>) -> Result<NetworkEndpoint, Error> {
        let port = match (self.port, default_port) {
            (Some(port), _) => port,
            (None, Some(port)) => nonzero_port(port)?,
            (None, None) => {
                return Err(invalid_address(
                    "Missing port and no default port is available",
                ));
            }
        };

        Ok(NetworkEndpoint {
            host: self.host,
            port,
        })
    }
}

impl NetworkEndpoint {
    fn format_socket_addr(&self) -> String {
        match &self.host {
            Host::Ip(IpAddr::V4(host)) => format!("{}:{}", host, self.port),
            Host::Ip(IpAddr::V6(host)) => format!("[{}]:{}", host, self.port),
            Host::Dns(host) => format!("{}:{}", host, self.port),
        }
    }
}

fn parse_bracketed_ipv6(address: &str) -> Result<ParsedEndpoint, Error> {
    let bracket_end = address
        .find(']')
        .ok_or_else(|| invalid_address("Unclosed IPv6 bracket"))?;

    if bracket_end == 1 {
        return Err(invalid_address("Empty IPv6 address in brackets"));
    }

    let ipv6_part = &address[1..bracket_end];
    let host = ipv6_part
        .parse::<Ipv6Addr>()
        .map_err(|_| invalid_address(format!("Invalid bracketed IPv6 address: {ipv6_part}")))?;
    let remainder = &address[bracket_end + 1..];

    let port = if remainder.is_empty() {
        None
    } else if let Some(port_str) = remainder.strip_prefix(':') {
        Some(parse_port(port_str)?)
    } else {
        return Err(invalid_address("Invalid characters after IPv6 bracket"));
    };

    Ok(ParsedEndpoint {
        host: Host::Ip(IpAddr::V6(host)),
        port,
    })
}

fn parse_dns_host(host: &str) -> Result<Host, Error> {
    if host.is_empty() {
        return Err(invalid_address("Empty host part"));
    }

    if host.contains('[') || host.contains(']') || host.contains(':') {
        return Err(invalid_address("Invalid host syntax"));
    }

    if host.chars().any(char::is_whitespace) {
        return Err(invalid_address("Host contains whitespace"));
    }

    Ok(Host::Dns(host.into()))
}

fn parse_port(port: &str) -> Result<NonZeroU16, Error> {
    if port.is_empty() {
        return Err(invalid_address("Empty port after colon"));
    }

    let port = port
        .parse::<u16>()
        .map_err(|_| invalid_address(format!("Invalid port number: {port}")))?;
    nonzero_port(port)
}

fn nonzero_port(port: u16) -> Result<NonZeroU16, Error> {
    NonZeroU16::new(port).ok_or_else(|| invalid_address("Port must be non-zero"))
}

fn invalid_address(reason: impl Into<Cow<'static, str>>) -> Error {
    Error::InvalidAddress {
        reason: reason.into(),
    }
}

/// Canonicalize a network endpoint address for TCP/UDP connections.
///
/// This function provides a single, centralized location for address canonicalization
/// at connection boundaries. It ensures:
///
/// 1. The address is parsed and validated
/// 2. Bare IPv6 literals are rejected to avoid ambiguous endpoint grammar
/// 3. Bracketed IPv6 is required for IPv6 endpoints
/// 4. Default ports are applied when the parsed port is missing
/// 5. A missing final port without a default returns `Error::InvalidAddress`
/// 6. The output is always a canonical `host:port` string suitable for `ToSocketAddrs`
///
/// IPv6 zone identifiers are intentionally rejected because the internal endpoint
/// model uses `std::net::IpAddr`, which does not represent scope identifiers.
///
/// This should be called at all TCP/UDP connection entry points to ensure consistent
/// address handling across the entire API surface.
///
/// # Arguments
///
/// * `address` - The address string to canonicalize (IPv4, bracketed IPv6, or hostname)
/// * `default_port` - Port to use if none is specified in the address
///
/// # Returns
///
/// A canonical socket address string where:
/// - IPv6 addresses are bracketed: `[2001:db8::1]:5678`
/// - IPv4 addresses are formatted: `192.168.1.1:5678`
/// - Hostnames are formatted: `camera.local:5678`
///
/// # Errors
///
/// Returns `Error::InvalidAddress` if:
/// - The address cannot be parsed
/// - The address format is invalid
/// - The endpoint has no explicit or default port
///
/// # Invariant
///
/// After canonicalization, all TCP/UDP connectors receive a canonical `host:port` string
/// where IPv6 is always bracketed when a port is present.
///
/// # Examples
///
/// ```ignore
/// use grafton_visca::transport::address::canonicalize_endpoint;
///
/// // Already bracketed IPv6 is preserved
/// assert_eq!(
///     canonicalize_endpoint("[::1]:1234", Some(5678)).unwrap(),
///     "[::1]:1234"
/// );
///
/// // IPv4 with port is formatted correctly
/// assert_eq!(
///     canonicalize_endpoint("192.168.1.1:5678", Some(1234)).unwrap(),
///     "192.168.1.1:5678"
/// );
///
/// // IPv4 without port gets default port added
/// assert_eq!(
///     canonicalize_endpoint("192.168.1.1", Some(5678)).unwrap(),
///     "192.168.1.1:5678"
/// );
///
/// // Hostname with port is formatted correctly
/// assert_eq!(
///     canonicalize_endpoint("camera.local:5678", Some(1234)).unwrap(),
///     "camera.local:5678"
/// );
///
/// // Hostname without port gets default port added
/// assert_eq!(
///     canonicalize_endpoint("localhost", Some(5678)).unwrap(),
///     "localhost:5678"
/// );
/// ```
pub fn canonicalize_endpoint(address: &str, default_port: Option<u16>) -> Result<String, Error> {
    let parsed = ParsedEndpoint::parse(address)?;
    let endpoint = parsed.with_default_port(default_port)?;
    Ok(endpoint.format_socket_addr())
}

/// The error for an endpoint whose name resolution failed.
///
/// Every facade (blocking, Tokio, smol) and both IP transports report a
/// resolution failure through this one constructor, so an unresolvable host
/// is the same [`Error::InvalidAddress`] with the same message everywhere.
#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
pub(crate) fn resolution_failed(endpoint: &str, error: &std::io::Error) -> Error {
    invalid_address(format!("Failed to resolve '{endpoint}': {error}"))
}

/// Collect a resolver's result for `endpoint`, rejecting failure and an empty
/// answer with the shared address errors.
#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
pub(crate) fn resolved<I>(
    endpoint: &str,
    lookup: std::io::Result<I>,
) -> Result<Vec<SocketAddr>, Error>
where
    I: IntoIterator<Item = SocketAddr>,
{
    let addresses: Vec<SocketAddr> = lookup
        .map_err(|error| resolution_failed(endpoint, &error))?
        .into_iter()
        .collect();
    if addresses.is_empty() {
        return Err(invalid_address(format!(
            "No addresses resolved for '{endpoint}'"
        )));
    }
    Ok(addresses)
}

/// A canonical endpoint that is already a socket address needs no resolver.
#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
pub(crate) fn literal_socket_addr(endpoint: &str) -> Option<SocketAddr> {
    endpoint.parse().ok()
}

/// Resolve a canonical endpoint on a blocking caller within `deadline`, with
/// the system resolver (see [`BlockingResolver`]).
#[cfg(feature = "blocking")]
pub(crate) fn resolve_blocking(
    endpoint: &str,
    deadline: crate::timeout::Deadline,
) -> Result<Vec<SocketAddr>, Error> {
    SYSTEM_RESOLVER.resolve(endpoint, deadline)
}

/// Most host-name lookups the blocking facade runs at once, process-wide.
#[cfg(feature = "blocking")]
const MAX_IN_FLIGHT_LOOKUPS: usize = 4;

/// Stack for one lookup thread; the platform resolver needs far less than a
/// default thread stack.
#[cfg(feature = "blocking")]
const LOOKUP_STACK_SIZE: usize = 256 * 1024;

#[cfg(feature = "blocking")]
static SYSTEM_RESOLVER: BlockingResolver =
    BlockingResolver::new(MAX_IN_FLIGHT_LOOKUPS, system_lookup);

#[cfg(feature = "blocking")]
fn system_lookup(endpoint: String) -> std::io::Result<Vec<SocketAddr>> {
    use std::net::ToSocketAddrs;

    endpoint.to_socket_addrs().map(Iterator::collect)
}

/// Blocking name resolution bounded by the connect budget.
///
/// An IP-literal endpoint resolves without a lookup. A host name is looked up
/// on a short-lived helper thread with a small stack, so the connect budget
/// also bounds DNS exactly as the async facades bound it with their runtime
/// timers. If the budget expires first the caller gets a connect timeout and
/// the helper finishes in the background. At most `MAX_IN_FLIGHT_LOOKUPS`
/// helpers exist at once: a connect that finds every slot taken waits for one
/// to free within its own budget, and only a budget that expires while
/// waiting is a connect timeout. No executor is involved.
#[cfg(feature = "blocking")]
#[derive(Debug)]
pub(crate) struct BlockingResolver {
    in_flight: std::sync::Mutex<usize>,
    freed: std::sync::Condvar,
    limit: usize,
    lookup: fn(String) -> std::io::Result<Vec<SocketAddr>>,
}

#[cfg(feature = "blocking")]
impl BlockingResolver {
    pub(crate) const fn new(
        limit: usize,
        lookup: fn(String) -> std::io::Result<Vec<SocketAddr>>,
    ) -> Self {
        Self {
            in_flight: std::sync::Mutex::new(0),
            freed: std::sync::Condvar::new(),
            limit,
            lookup,
        }
    }

    /// Resolve `endpoint` before `deadline`.
    ///
    /// # Errors
    ///
    /// The connect error contract of [`crate::transport::connect`]: a spent
    /// budget (including one spent waiting for a free lookup slot) is a
    /// connect timeout, a failed lookup is
    /// [`Error::InvalidAddress`], and a helper thread that cannot be started
    /// is [`Error::ConnectionFailed`] naming the endpoint.
    pub(crate) fn resolve(
        &'static self,
        endpoint: &str,
        deadline: crate::timeout::Deadline,
    ) -> Result<Vec<SocketAddr>, Error> {
        use std::{sync::mpsc, time::Instant};

        if let Some(address) = literal_socket_addr(endpoint) {
            return Ok(vec![address]);
        }
        let slot = self.claim(deadline)?;
        let remaining = deadline.remaining_or(Instant::now(), Error::connect_timeout)?;

        let (sender, receiver) = mpsc::sync_channel(1);
        let lookup_endpoint = endpoint.to_owned();
        let lookup = self.lookup;
        std::thread::Builder::new()
            .name("grafton-visca-resolve".into())
            .stack_size(LOOKUP_STACK_SIZE)
            .spawn(move || {
                let _slot = slot;
                // The connector may already have given up; nothing to report.
                let _ = sender.send(lookup(lookup_endpoint));
            })
            .map_err(|error| crate::transport::connect::connection_failed(endpoint, error))?;

        match receiver.recv_timeout(remaining) {
            Ok(lookup) => resolved(endpoint, lookup),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(Error::connect_timeout()),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(resolution_failed(
                endpoint,
                &std::io::Error::other("resolver thread ended without an answer"),
            )),
        }
    }

    /// Reserve one helper slot, waiting for one to free until `deadline`.
    fn claim(&'static self, deadline: crate::timeout::Deadline) -> Result<LookupSlot, Error> {
        use std::time::Instant;

        let mut in_flight = self.lock();
        while *in_flight >= self.limit {
            let remaining = deadline.remaining_or(Instant::now(), Error::connect_timeout)?;
            in_flight = self
                .freed
                .wait_timeout(in_flight, remaining)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
        *in_flight += 1;
        Ok(LookupSlot { resolver: self })
    }

    /// The slot count; a panicking lookup thread cannot leave it unusable.
    fn lock(&self) -> std::sync::MutexGuard<'_, usize> {
        self.in_flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// One reserved helper slot, released when its lookup thread ends (or when
/// the thread could not be started).
#[cfg(feature = "blocking")]
struct LookupSlot {
    resolver: &'static BlockingResolver,
}

#[cfg(feature = "blocking")]
impl Drop for LookupSlot {
    fn drop(&mut self) {
        *self.resolver.lock() -= 1;
        self.resolver.freed.notify_one();
    }
}

/// The unspecified local address of the same family as `target`, used to bind
/// a UDP socket before connecting it.
#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
pub(crate) fn bind_address_for(target: SocketAddr) -> SocketAddr {
    use std::net::Ipv4Addr;

    match target {
        SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[cfg(any(
        feature = "blocking",
        feature = "runtime-tokio",
        feature = "runtime-smol"
    ))]
    #[test]
    fn resolution_errors_name_the_endpoint() {
        let error = resolved::<Vec<SocketAddr>>(
            "camera.invalid:5678",
            Err(std::io::Error::other("lookup failed")),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            Error::InvalidAddress { ref reason }
                if reason == "Failed to resolve 'camera.invalid:5678': lookup failed"
        ));

        let error = resolved("camera.invalid:5678", Ok(Vec::new())).unwrap_err();
        assert!(matches!(
            error,
            Error::InvalidAddress { ref reason }
                if reason == "No addresses resolved for 'camera.invalid:5678'"
        ));
    }

    #[cfg(any(
        feature = "blocking",
        feature = "runtime-tokio",
        feature = "runtime-smol"
    ))]
    #[test]
    fn bind_address_matches_the_target_family() {
        let v4 = bind_address_for("192.168.0.100:5678".parse().unwrap());
        assert!(v4.is_ipv4() && v4.ip().is_unspecified() && v4.port() == 0);
        let v6 = bind_address_for("[::1]:5678".parse().unwrap());
        assert!(v6.is_ipv6() && v6.ip().is_unspecified() && v6.port() == 0);
    }

    #[cfg(feature = "blocking")]
    mod blocking_resolver {
        use std::{
            sync::{Condvar, Mutex},
            time::{Duration, Instant},
        };

        use super::*;
        use crate::timeout::Deadline;

        fn budget(timeout: Duration) -> Deadline {
            Deadline::after(Instant::now(), timeout, "connect_timeout").unwrap()
        }

        fn answer() -> SocketAddr {
            "192.0.2.7:5678".parse().unwrap()
        }

        /// A lookup that waits until its test opens the gate.
        struct Gate {
            open: Mutex<bool>,
            changed: Condvar,
        }

        impl Gate {
            const fn new() -> Self {
                Self {
                    open: Mutex::new(false),
                    changed: Condvar::new(),
                }
            }

            fn wait(&self) {
                let mut open = self.open.lock().unwrap();
                while !*open {
                    open = self.changed.wait(open).unwrap();
                }
            }

            fn release(&self) {
                *self.open.lock().unwrap() = true;
                self.changed.notify_all();
            }
        }

        fn wait_for_idle(resolver: &BlockingResolver) {
            let deadline = Instant::now() + Duration::from_secs(5);
            while *resolver.lock() != 0 {
                assert!(Instant::now() < deadline, "lookup thread never finished");
                std::thread::yield_now();
            }
        }

        #[test]
        fn ip_literals_resolve_without_a_lookup_even_with_a_spent_budget() {
            assert_eq!(
                resolve_blocking("[::1]:5678", budget(Duration::ZERO)).unwrap(),
                vec!["[::1]:5678".parse::<SocketAddr>().unwrap()]
            );
        }

        #[test]
        fn a_host_name_resolves_on_the_helper() {
            fn instant(_: String) -> std::io::Result<Vec<SocketAddr>> {
                Ok(vec![answer()])
            }
            static RESOLVER: BlockingResolver = BlockingResolver::new(1, instant);

            assert_eq!(
                RESOLVER
                    .resolve("camera.test:5678", budget(Duration::from_secs(5)))
                    .unwrap(),
                vec![answer()]
            );
            wait_for_idle(&RESOLVER);
        }

        #[test]
        fn a_failed_lookup_is_an_address_error() {
            fn failing(_: String) -> std::io::Result<Vec<SocketAddr>> {
                Err(std::io::Error::other("no such host"))
            }
            static RESOLVER: BlockingResolver = BlockingResolver::new(1, failing);

            assert!(matches!(
                RESOLVER.resolve("camera.test:5678", budget(Duration::from_secs(5))),
                Err(Error::InvalidAddress { .. })
            ));
        }

        /// The budget bounds a stalled lookup, and the stalled helper keeps
        /// its slot: a full pool makes the next connect wait for its own
        /// budget (never a 0 ms failure), and a slot freed in time lets it
        /// proceed.
        #[test]
        fn a_full_pool_waits_for_a_slot_within_the_budget() {
            static GATE: Gate = Gate::new();
            fn gated(_: String) -> std::io::Result<Vec<SocketAddr>> {
                GATE.wait();
                Ok(vec![answer()])
            }
            static RESOLVER: BlockingResolver = BlockingResolver::new(1, gated);

            let started = Instant::now();
            assert!(matches!(
                RESOLVER.resolve("camera.test:5678", budget(Duration::from_millis(20))),
                Err(Error::Timeout { .. })
            ));
            assert!(started.elapsed() >= Duration::from_millis(20));
            assert_eq!(*RESOLVER.lock(), 1);

            // Every slot stays taken: the next connect waits its whole budget.
            let started = Instant::now();
            assert!(matches!(
                RESOLVER.resolve("camera.test:5678", budget(Duration::from_millis(40))),
                Err(Error::Timeout { .. })
            ));
            assert!(started.elapsed() >= Duration::from_millis(40));

            // The stalled lookup finishes while this connect is waiting.
            let release = std::thread::spawn(|| {
                std::thread::sleep(Duration::from_millis(30));
                GATE.release();
            });
            assert_eq!(
                RESOLVER
                    .resolve("camera.test:5678", budget(Duration::from_secs(5)))
                    .unwrap(),
                vec![answer()]
            );
            release.join().unwrap();
            wait_for_idle(&RESOLVER);
        }

        /// More parallel host-name connects than slots all succeed against a
        /// slow but working resolver.
        #[test]
        fn parallel_connects_beyond_the_cap_all_resolve() {
            fn slow(_: String) -> std::io::Result<Vec<SocketAddr>> {
                std::thread::sleep(Duration::from_millis(30));
                Ok(vec![answer()])
            }
            static RESOLVER: BlockingResolver = BlockingResolver::new(4, slow);

            let connects: Vec<_> = (0..8)
                .map(|_| {
                    std::thread::spawn(|| {
                        RESOLVER.resolve("camera.test:5678", budget(Duration::from_secs(5)))
                    })
                })
                .collect();
            for connect in connects {
                assert_eq!(connect.join().unwrap().unwrap(), vec![answer()]);
            }
            wait_for_idle(&RESOLVER);
        }

        #[test]
        fn localhost_resolves_through_the_system_resolver() {
            let addresses =
                resolve_blocking("localhost:5678", budget(Duration::from_secs(5))).unwrap();
            assert!(addresses.iter().all(|address| address.port() == 5678));
        }
    }

    fn assert_invalid(result: Result<String, Error>) {
        assert!(
            matches!(result, Err(Error::InvalidAddress { .. })),
            "expected Error::InvalidAddress, got {result:?}"
        );
    }

    #[test]
    fn test_bare_ipv6_with_numeric_final_hextet_is_rejected() {
        assert_invalid(canonicalize_endpoint("2001:db8::1:5678", Some(1234)));
        assert_invalid(canonicalize_endpoint("2001:db8::1:5678", None));
    }

    #[test]
    fn test_bracketed_ipv6_explicit_and_default_ports() {
        assert_eq!(
            canonicalize_endpoint("[2001:db8::1]:5678", Some(1234)).unwrap(),
            "[2001:db8::1]:5678"
        );
        assert_eq!(
            canonicalize_endpoint("[::1]", Some(5678)).unwrap(),
            "[::1]:5678"
        );

        assert_invalid(canonicalize_endpoint("::1", None));
        assert_invalid(canonicalize_endpoint("[::1]", None));
    }

    #[test]
    fn test_ipv4_explicit_and_default_ports() {
        assert_eq!(
            canonicalize_endpoint("192.168.0.110:5678", Some(1234)).unwrap(),
            "192.168.0.110:5678"
        );
        assert_eq!(
            canonicalize_endpoint("192.168.0.110", Some(5678)).unwrap(),
            "192.168.0.110:5678"
        );

        assert_invalid(canonicalize_endpoint("192.168.0.110", None));
    }

    #[test]
    fn test_dns_explicit_and_default_ports() {
        assert_eq!(
            canonicalize_endpoint("camera.local:5678", Some(1234)).unwrap(),
            "camera.local:5678"
        );
        assert_eq!(
            canonicalize_endpoint("camera.local", Some(5678)).unwrap(),
            "camera.local:5678"
        );

        assert_invalid(canonicalize_endpoint("camera.local", None));
    }

    #[test]
    fn test_port_rejections_are_consistent() {
        for address in [
            "host:",
            "[::1]:",
            "host:0",
            "[::1]:0",
            "host:99999",
            "[::1]:99999",
        ] {
            assert_invalid(canonicalize_endpoint(address, Some(5678)));
        }

        assert_invalid(canonicalize_endpoint("host", Some(0)));
    }

    #[test]
    fn test_malformed_address_rejections() {
        for address in [
            "",
            "   ",
            ":5678",
            "[]:5678",
            "[::1",
            "[::1]junk",
            "2001:db8:::1",
            "host:name:5678",
            "[fe80::1%eth0]:5678",
            "fe80::1%eth0",
        ] {
            assert_invalid(canonicalize_endpoint(address, Some(5678)));
        }
    }

    #[test]
    fn test_bare_ipv6_literals_are_rejected() {
        for literal in [
            "::",
            "::1",
            "2001:db8::1",
            "2001:db8::1:80",
            "2001:db8::1:5678",
            "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
            "::ffff:192.0.2.128",
        ] {
            assert_invalid(canonicalize_endpoint(literal, Some(1234)));
            assert_invalid(canonicalize_endpoint(literal, None));
        }
    }
}
