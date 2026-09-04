//! Common address resolution utilities for transport implementations.
//!
//! This module provides a unified way to resolve network addresses across
//! different transport types, eliminating code duplication.

use std::borrow::Cow;
#[cfg(any(feature = "blocking", test))]
use std::net::ToSocketAddrs;
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

/// A utility for resolving network addresses.
///
/// This struct provides common address resolution logic that can be used
/// across different transport implementations, reducing code duplication.
#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol",
    test
))]
#[derive(Debug, Clone, Copy, Default)]
pub struct AddressResolver;

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol",
    test
))]
impl AddressResolver {
    /// Create a new address resolver.
    pub fn new() -> Self {
        Self
    }

    /// Resolve an address string to a list of socket addresses.
    ///
    /// This method handles DNS resolution and returns all resolved addresses.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to resolve (can be hostname:port or IP:port)
    ///
    /// # Returns
    ///
    /// A vector of resolved socket addresses.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The address format is invalid
    /// - DNS resolution fails
    /// - No addresses could be resolved
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use grafton_visca::transport::address::AddressResolver;
    ///
    /// let resolver = AddressResolver::new();
    /// let addresses = resolver.resolve("example.com:5678")?;
    /// for addr in addresses {
    ///     println!("Resolved: {addr}");
    /// }
    /// # Ok::<(), grafton_visca::Error>(())
    /// ```
    #[cfg(any(feature = "blocking", test))]
    pub fn resolve(&self, address: &str) -> Result<Vec<SocketAddr>, Error> {
        let addrs: Vec<SocketAddr> = address
            .to_socket_addrs()
            .map_err(|e| Error::InvalidAddress {
                reason: format!("Failed to resolve '{address}': {e}").into(),
            })?
            .collect();

        if addrs.is_empty() {
            return Err(Error::InvalidAddress {
                reason: format!("No addresses resolved for '{address}'").into(),
            });
        }

        Ok(addrs)
    }

    /// Resolve an address string and return the first resolved socket address.
    ///
    /// This is a convenience method that resolves an address and returns
    /// the first available result, which is suitable for most use cases.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to resolve (can be hostname:port or IP:port)
    ///
    /// # Returns
    ///
    /// The first resolved socket address.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The address format is invalid
    /// - DNS resolution fails
    /// - No addresses could be resolved
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use grafton_visca::transport::address::AddressResolver;
    ///
    /// let resolver = AddressResolver::new();
    /// let addr = resolver.resolve_first("192.168.0.100:5678")?;
    /// println!("Using address: {addr}");
    /// # Ok::<(), grafton_visca::Error>(())
    /// ```
    #[cfg(any(feature = "blocking", test))]
    pub fn resolve_first(&self, address: &str) -> Result<SocketAddr, Error> {
        self.resolve(address)?
            .into_iter()
            .next()
            .ok_or_else(|| Error::InvalidAddress {
                reason: format!("No addresses resolved for '{address}'").into(),
            })
    }

    /// Determine an appropriate bind address for connecting to a target address.
    ///
    /// This method returns a bind address that matches the IP version of the
    /// target address (IPv4 or IPv6).
    ///
    /// # Arguments
    ///
    /// * `target` - The target socket address to connect to
    ///
    /// # Returns
    ///
    /// An appropriate bind address for the target's IP version.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use grafton_visca::transport::address::AddressResolver;
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    ///
    /// let resolver = AddressResolver::new();
    /// let target = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 100)), 5678);
    /// let bind_addr = resolver.bind_address_for(&target);
    /// assert!(bind_addr.is_ipv4());
    /// ```
    pub fn bind_address_for(&self, target: &SocketAddr) -> SocketAddr {
        use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

        if target.is_ipv4() {
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)
        } else {
            SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0)
        }
    }

    /// Resolve an address with a preference for a specific IP version.
    ///
    /// This method resolves an address and filters the results based on
    /// the preferred IP version.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to resolve
    /// * `prefer_ipv4` - If true, prefer IPv4 addresses; otherwise prefer IPv6
    ///
    /// # Returns
    ///
    /// A socket address matching the preferred IP version, or the first
    /// available address if no match is found.
    ///
    /// # Errors
    ///
    /// Returns an error if no addresses could be resolved.
    #[cfg(test)]
    pub fn resolve_with_preference(
        &self,
        address: &str,
        prefer_ipv4: bool,
    ) -> Result<SocketAddr, Error> {
        let addrs = self.resolve(address)?;

        // Try to find an address matching the preference
        let preferred = addrs
            .iter()
            .find(|addr| addr.is_ipv4() == prefer_ipv4)
            .copied();

        // Fall back to the first address if no preference match
        preferred
            .or(addrs.into_iter().next())
            .ok_or(Error::InvalidAddress {
                reason: Cow::Borrowed("No addresses resolved"),
            })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    #[cfg_attr(miri, ignore = "requires OS hostname resolution")]
    fn test_resolve_localhost() {
        let resolver = AddressResolver::new();
        let result = resolver.resolve("localhost:5678");
        assert!(result.is_ok());
        let addrs = result.unwrap();
        assert!(!addrs.is_empty());
    }

    #[test]
    fn test_resolve_first() {
        let resolver = AddressResolver::new();
        let result = resolver.resolve_first("127.0.0.1:5678");
        assert!(result.is_ok());
        let addr = result.unwrap();
        assert_eq!(addr.port(), 5678);
        assert!(addr.is_ipv4());
    }

    #[test]
    fn test_bind_address_for_ipv4() {
        let resolver = AddressResolver::new();
        let target: SocketAddr = "192.168.0.100:5678".parse().unwrap();
        let bind_addr = resolver.bind_address_for(&target);
        assert!(bind_addr.is_ipv4());
        assert_eq!(bind_addr.port(), 0);
    }

    #[test]
    fn test_bind_address_for_ipv6() {
        let resolver = AddressResolver::new();
        let target: SocketAddr = "[::1]:5678".parse().unwrap();
        let bind_addr = resolver.bind_address_for(&target);
        assert!(bind_addr.is_ipv6());
        assert_eq!(bind_addr.port(), 0);
    }

    #[test]
    fn test_invalid_address() {
        let resolver = AddressResolver::new();
        let result = resolver.resolve("not-a-valid-address");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_with_preference_ipv4() {
        let resolver = AddressResolver::new();
        let result = resolver.resolve_with_preference("127.0.0.1:5678", true);
        assert!(result.is_ok());
        let addr = result.unwrap();
        assert!(addr.is_ipv4());
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
