//! Common address resolution utilities for transport implementations.
//!
//! This module provides a unified way to resolve network addresses across
//! different transport types, eliminating code duplication.

use std::borrow::Cow;
use std::net::{SocketAddr, ToSocketAddrs};

use crate::Error;

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
}
