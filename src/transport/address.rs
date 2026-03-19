//! Common address resolution utilities for transport implementations.
//!
//! This module provides a unified way to resolve network addresses across
//! different transport types, eliminating code duplication.

use std::{
    borrow::Cow,
    net::{SocketAddr, ToSocketAddrs},
};

use crate::Error;

/// Represents a parsed host:port combination that handles IPv6 addresses correctly.
///
/// This enum properly distinguishes between IPv6 address colons and port separators,
/// solving common parsing issues where `host.contains(':')` or `host.rfind(':')`
/// fail with IPv6 addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostPort {
    /// IPv4 address with optional port
    Ipv4 {
        /// IPv4 host address string
        host: String,
        /// Optional port number
        port: Option<u16>,
    },
    /// IPv6 address with optional port (brackets handled automatically)
    Ipv6 {
        /// IPv6 host address string (without brackets)
        host: String,
        /// Optional port number
        port: Option<u16>,
    },
    /// Hostname with optional port
    Hostname {
        /// Hostname string
        host: String,
        /// Optional port number
        port: Option<u16>,
    },
}

impl HostPort {
    /// Parse a host:port string, correctly handling IPv6 addresses.
    ///
    /// This function correctly handles:
    /// - IPv4: `192.168.1.1`, `192.168.1.1:5678`
    /// - IPv6: `::1`, `[::1]`, `[::1]:5678`, `2001:db8::1`, `[2001:db8::1]:5678`
    /// - IPv6 with zone: `[fe80::1%eth0]:5678`
    /// - Hostnames: `localhost`, `camera.local:5678`
    ///
    /// # Examples
    ///
    /// ```
    /// use grafton_visca::transport::address::HostPort;
    ///
    /// // IPv4 examples
    /// let parsed = HostPort::parse("192.168.1.1:5678").unwrap();
    /// assert_eq!(parsed.host(), "192.168.1.1");
    /// assert_eq!(parsed.port(), Some(5678));
    ///
    /// // IPv6 examples
    /// let parsed = HostPort::parse("[::1]:5678").unwrap();
    /// assert_eq!(parsed.host(), "::1");
    /// assert_eq!(parsed.port(), Some(5678));
    ///
    /// let parsed = HostPort::parse("::1").unwrap();
    /// assert_eq!(parsed.host(), "::1");
    /// assert_eq!(parsed.port(), None);
    /// ```
    pub fn parse(address: &str) -> Result<Self, Error> {
        let address = address.trim();

        if address.is_empty() {
            return Err(Error::InvalidAddress {
                reason: "Empty address".into(),
            });
        }

        // Check for IPv6 with brackets first: [::1]:5678 or [::1]
        if address.starts_with('[') {
            if let Some(bracket_end) = address.find(']') {
                if bracket_end == 1 {
                    return Err(Error::InvalidAddress {
                        reason: "Empty IPv6 address in brackets".into(),
                    });
                }

                let ipv6_part = &address[1..bracket_end];
                let remainder = &address[bracket_end + 1..];

                if remainder.is_empty() {
                    // Just [::1]
                    return Ok(HostPort::Ipv6 {
                        host: ipv6_part.to_string(),
                        port: None,
                    });
                } else if let Some(port_str) = remainder.strip_prefix(':') {
                    // [::1]:5678
                    if port_str.is_empty() {
                        return Err(Error::InvalidAddress {
                            reason: "Empty port after colon".into(),
                        });
                    }
                    let port = port_str.parse::<u16>().map_err(|_| Error::InvalidAddress {
                        reason: format!("Invalid port number: {}", port_str).into(),
                    })?;
                    return Ok(HostPort::Ipv6 {
                        host: ipv6_part.to_string(),
                        port: Some(port),
                    });
                } else {
                    return Err(Error::InvalidAddress {
                        reason: "Invalid characters after IPv6 bracket".into(),
                    });
                }
            } else {
                return Err(Error::InvalidAddress {
                    reason: "Unclosed IPv6 bracket".into(),
                });
            }
        }

        // Check if this looks like a bare IPv6 address (contains :: or multiple colons)
        let colon_count = address.chars().filter(|&c| c == ':').count();
        if colon_count > 1 && (address.contains("::") || colon_count >= 2) {
            // Likely IPv6 without brackets, e.g., "::1" or "2001:db8::1"
            // We need to be careful here - check if the last part could be a port
            if let Some(last_colon) = address.rfind(':') {
                let potential_port = &address[last_colon + 1..];

                // Only treat it as a port if:
                // 1. It's a valid port number (1-65535)
                // 2. The rest looks like a valid IPv6 address
                if let Ok(port) = potential_port.parse::<u16>() {
                    if port > 0 {
                        let ipv6_part = &address[..last_colon];
                        // Simple validation: IPv6 should contain :: or have exactly the right number of colons
                        if ipv6_part.contains("::")
                            || ipv6_part.chars().filter(|&c| c == ':').count() == 7
                        {
                            return Ok(HostPort::Ipv6 {
                                host: ipv6_part.to_string(),
                                port: Some(port),
                            });
                        }
                    }
                }
            }

            // If we reach here, treat the whole thing as an IPv6 address without port
            return Ok(HostPort::Ipv6 {
                host: address.to_string(),
                port: None,
            });
        }

        // Not IPv6, so check for IPv4 or hostname with port
        if let Some(last_colon) = address.rfind(':') {
            let host_part = &address[..last_colon];
            let port_part = &address[last_colon + 1..];

            if host_part.is_empty() {
                return Err(Error::InvalidAddress {
                    reason: "Empty host part".into(),
                });
            }

            if port_part.is_empty() {
                return Err(Error::InvalidAddress {
                    reason: "Empty port after colon".into(),
                });
            }

            let port = port_part
                .parse::<u16>()
                .map_err(|_| Error::InvalidAddress {
                    reason: format!("Invalid port number: {}", port_part).into(),
                })?;

            // Determine if this is IPv4 or hostname
            if is_ipv4_address(host_part) {
                Ok(HostPort::Ipv4 {
                    host: host_part.to_string(),
                    port: Some(port),
                })
            } else {
                Ok(HostPort::Hostname {
                    host: host_part.to_string(),
                    port: Some(port),
                })
            }
        } else {
            // No port specified
            if is_ipv4_address(address) {
                Ok(HostPort::Ipv4 {
                    host: address.to_string(),
                    port: None,
                })
            } else {
                Ok(HostPort::Hostname {
                    host: address.to_string(),
                    port: None,
                })
            }
        }
    }

    /// Get the host part of the address (without brackets for IPv6).
    pub fn host(&self) -> &str {
        match self {
            HostPort::Ipv4 { host, .. } => host,
            HostPort::Ipv6 { host, .. } => host,
            HostPort::Hostname { host, .. } => host,
        }
    }

    /// Get the port if specified.
    pub fn port(&self) -> Option<u16> {
        match self {
            HostPort::Ipv4 { port, .. } => *port,
            HostPort::Ipv6 { port, .. } => *port,
            HostPort::Hostname { port, .. } => *port,
        }
    }

    /// Check if this is an IPv6 address.
    pub fn is_ipv6(&self) -> bool {
        matches!(self, HostPort::Ipv6 { .. })
    }

    /// Format as a proper socket address string suitable for `ToSocketAddrs`.
    ///
    /// This ensures IPv6 addresses are properly bracketed when a port is present.
    pub fn format_socket_addr(&self, default_port: Option<u16>) -> String {
        match self {
            HostPort::Ipv4 { host, port } => match (port, default_port) {
                (Some(p), _) => format!("{}:{}", host, p),
                (None, Some(dp)) => format!("{}:{}", host, dp),
                (None, None) => host.clone(),
            },
            HostPort::Ipv6 { host, port } => match (port, default_port) {
                (Some(p), _) => format!("[{}]:{}", host, p),
                (None, Some(dp)) => format!("[{}]:{}", host, dp),
                (None, None) => host.clone(),
            },
            HostPort::Hostname { host, port } => match (port, default_port) {
                (Some(p), _) => format!("{}:{}", host, p),
                (None, Some(dp)) => format!("{}:{}", host, dp),
                (None, None) => host.clone(),
            },
        }
    }
}

/// Check if a string looks like an IPv4 address (simple heuristic).
fn is_ipv4_address(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return false;
    }

    parts.iter().all(|part| {
        if let Ok(_num) = part.parse::<u8>() {
            // Valid if it parses as u8 and doesn't have leading zeros (except for "0")
            *part == "0" || !part.starts_with('0')
        } else {
            false
        }
    })
}

/// Normalize a host with an optional default port, handling IPv6 correctly.
///
/// This function takes a host string (which may or may not include a port)
/// and ensures it's formatted correctly for use with `ToSocketAddrs`.
/// IPv6 addresses are properly bracketed when needed.
///
/// # Arguments
///
/// * `host` - The host string to normalize (e.g., `"::1"`, `"[::1]:5678"`, `"192.168.1.1:5678"`)
/// * `default_port` - Port to use if none is specified in the host string
///
/// # Examples
///
/// ```
/// use grafton_visca::transport::address::normalize_host_with_default_port;
///
/// // IPv6 examples
/// assert_eq!(
///     normalize_host_with_default_port("::1", Some(5678)).unwrap(),
///     "[::1]:5678"
/// );
/// assert_eq!(
///     normalize_host_with_default_port("[::1]:1234", Some(5678)).unwrap(),
///     "[::1]:1234"
/// );
///
/// // IPv4 examples
/// assert_eq!(
///     normalize_host_with_default_port("192.168.1.1", Some(5678)).unwrap(),
///     "192.168.1.1:5678"
/// );
/// assert_eq!(
///     normalize_host_with_default_port("192.168.1.1:1234", Some(5678)).unwrap(),
///     "192.168.1.1:1234"
/// );
/// ```
pub fn normalize_host_with_default_port(
    host: &str,
    default_port: Option<u16>,
) -> Result<String, Error> {
    let parsed = HostPort::parse(host)?;
    Ok(parsed.format_socket_addr(default_port))
}

/// Canonicalize a network endpoint address for TCP/UDP connections.
///
/// This function provides a single, centralized location for address canonicalization
/// at connection boundaries. It ensures:
///
/// 1. The address is parsed and validated
/// 2. IPv6 addresses are properly bracketed when a port is present
/// 3. Default ports are applied when the parsed port is missing
/// 4. The output is always a canonical `host:port` string suitable for `ToSocketAddrs`
///
/// This should be called at all TCP/UDP connection entry points to ensure consistent
/// address handling across the entire API surface.
///
/// # Arguments
///
/// * `address` - The address string to canonicalize (may be IPv4, IPv6, or hostname)
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
///
/// # Invariant
///
/// After canonicalization, all TCP/UDP connectors receive a canonical `host:port` string
/// where IPv6 is always bracketed when a port is present.
///
/// # Examples
///
/// ```
/// use grafton_visca::transport::address::canonicalize_endpoint;
///
/// // Unbracketed IPv6 with port is canonicalized to bracketed form
/// assert_eq!(
///     canonicalize_endpoint("2001:db8::1:5678", Some(1234)).unwrap(),
///     "[2001:db8::1]:5678"
/// );
///
/// // IPv6 without port gets default port added
/// assert_eq!(
///     canonicalize_endpoint("::1", Some(5678)).unwrap(),
///     "[::1]:5678"
/// );
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
    let parsed = HostPort::parse(address)?;
    Ok(parsed.format_socket_addr(default_port))
}

/// A utility for resolving network addresses.
///
/// This struct provides common address resolution logic that can be used
/// across different transport implementations, reducing code duplication.
#[derive(Debug, Clone, Copy, Default)]
pub struct AddressResolver;

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
    /// ```no_run
    /// use grafton_visca::transport::address::AddressResolver;
    ///
    /// let resolver = AddressResolver::new();
    /// let addresses = resolver.resolve("example.com:5678")?;
    /// for addr in addresses {
    ///     println!("Resolved: {addr}");
    /// }
    /// # Ok::<(), grafton_visca::Error>(())
    /// ```
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
    /// ```no_run
    /// use grafton_visca::transport::address::AddressResolver;
    ///
    /// let resolver = AddressResolver::new();
    /// let addr = resolver.resolve_first("192.168.0.100:5678")?;
    /// println!("Using address: {addr}");
    /// # Ok::<(), grafton_visca::Error>(())
    /// ```
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
    /// ```
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

/// A trait for types that can resolve addresses.
///
/// This trait can be implemented by transport types to provide
/// consistent address resolution behavior.
pub trait ResolveAddress {
    /// Resolve an address string to a socket address.
    fn resolve_address(&self, address: &str) -> Result<SocketAddr, Error>;
}

impl ResolveAddress for AddressResolver {
    fn resolve_address(&self, address: &str) -> Result<SocketAddr, Error> {
        self.resolve_first(address)
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

    #[test]
    fn test_hostport_parse_ipv4() {
        let parsed = HostPort::parse("192.168.1.1:5678").unwrap();
        assert_eq!(parsed.host(), "192.168.1.1");
        assert_eq!(parsed.port(), Some(5678));
        assert!(!parsed.is_ipv6());

        let parsed = HostPort::parse("192.168.1.1").unwrap();
        assert_eq!(parsed.host(), "192.168.1.1");
        assert_eq!(parsed.port(), None);
        assert!(!parsed.is_ipv6());
    }

    #[test]
    fn test_hostport_parse_ipv6_with_brackets() {
        let parsed = HostPort::parse("[::1]:5678").unwrap();
        assert_eq!(parsed.host(), "::1");
        assert_eq!(parsed.port(), Some(5678));
        assert!(parsed.is_ipv6());

        let parsed = HostPort::parse("[::1]").unwrap();
        assert_eq!(parsed.host(), "::1");
        assert_eq!(parsed.port(), None);
        assert!(parsed.is_ipv6());

        let parsed = HostPort::parse("[2001:db8::1]:8080").unwrap();
        assert_eq!(parsed.host(), "2001:db8::1");
        assert_eq!(parsed.port(), Some(8080));
        assert!(parsed.is_ipv6());
    }

    #[test]
    fn test_hostport_parse_ipv6_without_brackets() {
        let parsed = HostPort::parse("::1").unwrap();
        assert_eq!(parsed.host(), "::1");
        assert_eq!(parsed.port(), None);
        assert!(parsed.is_ipv6());

        let parsed = HostPort::parse("2001:db8::1").unwrap();
        assert_eq!(parsed.host(), "2001:db8::1");
        assert_eq!(parsed.port(), None);
        assert!(parsed.is_ipv6());

        // IPv6 with port (tricky case)
        let parsed = HostPort::parse("2001:db8::1:5678").unwrap();
        assert_eq!(parsed.host(), "2001:db8::1");
        assert_eq!(parsed.port(), Some(5678));
        assert!(parsed.is_ipv6());
    }

    #[test]
    fn test_hostport_parse_hostname() {
        let parsed = HostPort::parse("localhost:8080").unwrap();
        assert_eq!(parsed.host(), "localhost");
        assert_eq!(parsed.port(), Some(8080));
        assert!(!parsed.is_ipv6());

        let parsed = HostPort::parse("camera.local").unwrap();
        assert_eq!(parsed.host(), "camera.local");
        assert_eq!(parsed.port(), None);
        assert!(!parsed.is_ipv6());
    }

    #[test]
    fn test_hostport_parse_errors() {
        assert!(HostPort::parse("").is_err());
        assert!(HostPort::parse("   ").is_err());
        assert!(HostPort::parse("[]:5678").is_err());
        assert!(HostPort::parse("[::1").is_err());
        assert!(HostPort::parse("[::1]invalid").is_err());
        assert!(HostPort::parse("host:").is_err());
        assert!(HostPort::parse(":5678").is_err());
        assert!(HostPort::parse("host:99999").is_err());
    }

    #[test]
    fn test_hostport_format_socket_addr() {
        // IPv4
        let hp = HostPort::Ipv4 {
            host: "192.168.1.1".to_string(),
            port: Some(5678),
        };
        assert_eq!(hp.format_socket_addr(None), "192.168.1.1:5678");
        assert_eq!(hp.format_socket_addr(Some(8080)), "192.168.1.1:5678");

        let hp = HostPort::Ipv4 {
            host: "192.168.1.1".to_string(),
            port: None,
        };
        assert_eq!(hp.format_socket_addr(Some(8080)), "192.168.1.1:8080");
        assert_eq!(hp.format_socket_addr(None), "192.168.1.1");

        // IPv6
        let hp = HostPort::Ipv6 {
            host: "::1".to_string(),
            port: Some(5678),
        };
        assert_eq!(hp.format_socket_addr(None), "[::1]:5678");
        assert_eq!(hp.format_socket_addr(Some(8080)), "[::1]:5678");

        let hp = HostPort::Ipv6 {
            host: "::1".to_string(),
            port: None,
        };
        assert_eq!(hp.format_socket_addr(Some(8080)), "[::1]:8080");
        assert_eq!(hp.format_socket_addr(None), "::1");

        // Hostname
        let hp = HostPort::Hostname {
            host: "localhost".to_string(),
            port: Some(5678),
        };
        assert_eq!(hp.format_socket_addr(None), "localhost:5678");
        assert_eq!(hp.format_socket_addr(Some(8080)), "localhost:5678");

        let hp = HostPort::Hostname {
            host: "localhost".to_string(),
            port: None,
        };
        assert_eq!(hp.format_socket_addr(Some(8080)), "localhost:8080");
        assert_eq!(hp.format_socket_addr(None), "localhost");
    }

    #[test]
    fn test_normalize_host_with_default_port() {
        // IPv4 cases
        assert_eq!(
            normalize_host_with_default_port("192.168.1.1", Some(5678)).unwrap(),
            "192.168.1.1:5678"
        );
        assert_eq!(
            normalize_host_with_default_port("192.168.1.1:1234", Some(5678)).unwrap(),
            "192.168.1.1:1234"
        );

        // IPv6 cases
        assert_eq!(
            normalize_host_with_default_port("::1", Some(5678)).unwrap(),
            "[::1]:5678"
        );
        assert_eq!(
            normalize_host_with_default_port("[::1]:1234", Some(5678)).unwrap(),
            "[::1]:1234"
        );
        assert_eq!(
            normalize_host_with_default_port("2001:db8::1", Some(5678)).unwrap(),
            "[2001:db8::1]:5678"
        );

        // Hostname cases
        assert_eq!(
            normalize_host_with_default_port("localhost", Some(5678)).unwrap(),
            "localhost:5678"
        );
        assert_eq!(
            normalize_host_with_default_port("camera.local:1234", Some(5678)).unwrap(),
            "camera.local:1234"
        );
    }

    #[test]
    fn test_is_ipv4_address() {
        assert!(is_ipv4_address("192.168.1.1"));
        assert!(is_ipv4_address("0.0.0.0"));
        assert!(is_ipv4_address("255.255.255.255"));
        assert!(!is_ipv4_address("192.168.1"));
        assert!(!is_ipv4_address("192.168.1.1.1"));
        assert!(!is_ipv4_address("256.1.1.1"));
        assert!(!is_ipv4_address("192.168.01.1")); // leading zero
        assert!(!is_ipv4_address("localhost"));
        assert!(!is_ipv4_address("::1"));
    }

    #[test]
    fn test_ipv6_edge_cases() {
        // Zone identifiers (link-local)
        let parsed = HostPort::parse("[fe80::1%eth0]:5678").unwrap();
        assert_eq!(parsed.host(), "fe80::1%eth0");
        assert_eq!(parsed.port(), Some(5678));
        assert!(parsed.is_ipv6());

        // Full IPv6 address
        let parsed = HostPort::parse("[2001:0db8:85a3:0000:0000:8a2e:0370:7334]:443").unwrap();
        assert_eq!(parsed.host(), "2001:0db8:85a3:0000:0000:8a2e:0370:7334");
        assert_eq!(parsed.port(), Some(443));
        assert!(parsed.is_ipv6());

        // Compressed IPv6
        let parsed = HostPort::parse("[2001:db8:85a3::8a2e:370:7334]:80").unwrap();
        assert_eq!(parsed.host(), "2001:db8:85a3::8a2e:370:7334");
        assert_eq!(parsed.port(), Some(80));
        assert!(parsed.is_ipv6());
    }

    #[test]
    fn test_canonicalize_endpoint_ipv6_unbracketed_with_port() {
        // This is the key test case from the issue: unbracketed IPv6 with port
        // should be canonicalized to bracketed form
        assert_eq!(
            canonicalize_endpoint("2001:db8::1:5678", Some(1234)).unwrap(),
            "[2001:db8::1]:5678"
        );
    }

    #[test]
    fn test_canonicalize_endpoint_ipv6() {
        // IPv6 without port gets default port added
        assert_eq!(
            canonicalize_endpoint("::1", Some(5678)).unwrap(),
            "[::1]:5678"
        );

        // Already bracketed IPv6 is preserved
        assert_eq!(
            canonicalize_endpoint("[::1]:1234", Some(5678)).unwrap(),
            "[::1]:1234"
        );

        // IPv6 without brackets and no port
        assert_eq!(
            canonicalize_endpoint("2001:db8::1", Some(5678)).unwrap(),
            "[2001:db8::1]:5678"
        );

        // Already bracketed without port
        assert_eq!(
            canonicalize_endpoint("[::1]", Some(5678)).unwrap(),
            "[::1]:5678"
        );
    }

    #[test]
    fn test_canonicalize_endpoint_ipv4() {
        // IPv4 with port is formatted correctly
        assert_eq!(
            canonicalize_endpoint("192.168.1.1:5678", Some(1234)).unwrap(),
            "192.168.1.1:5678"
        );

        // IPv4 without port gets default port added
        assert_eq!(
            canonicalize_endpoint("192.168.1.1", Some(5678)).unwrap(),
            "192.168.1.1:5678"
        );
    }

    #[test]
    fn test_canonicalize_endpoint_hostname() {
        // Hostname with port is formatted correctly
        assert_eq!(
            canonicalize_endpoint("camera.local:5678", Some(1234)).unwrap(),
            "camera.local:5678"
        );

        // Hostname without port gets default port added
        assert_eq!(
            canonicalize_endpoint("localhost", Some(5678)).unwrap(),
            "localhost:5678"
        );
    }

    #[test]
    fn test_canonicalize_endpoint_errors() {
        // Empty address
        assert!(canonicalize_endpoint("", Some(5678)).is_err());

        // Invalid port
        assert!(canonicalize_endpoint("localhost:", Some(5678)).is_err());

        // Empty host
        assert!(canonicalize_endpoint(":5678", Some(5678)).is_err());
    }

    #[test]
    fn test_canonicalize_endpoint_no_default_port() {
        // When no default port is provided and address has no port,
        // the result should be just the host (for later error at resolution time)
        assert_eq!(
            canonicalize_endpoint("192.168.1.1", None).unwrap(),
            "192.168.1.1"
        );

        // IPv6 without port and no default should return just the host
        assert_eq!(canonicalize_endpoint("::1", None).unwrap(), "::1");
    }
}
