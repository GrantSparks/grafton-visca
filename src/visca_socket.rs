//! Unified VISCA socket type.
//!
//! This module provides a single socket type that unifies the various socket
//! representations used throughout the codebase.

/// VISCA socket identifier.
///
/// VISCA cameras maintain two concurrent command execution slots (sockets) to allow
/// up to two commands to be processed simultaneously. This unified type replaces
/// the various socket representations that were previously scattered throughout
/// the codebase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ViscaSocket {
    /// First command socket
    #[default]
    S1,
    /// Second command socket
    S2,
}

impl ViscaSocket {
    /// Convert to zero-based index (0 or 1).
    ///
    /// This is used for array indexing and similar operations.
    #[must_use]
    pub fn as_index(self) -> usize {
        match self {
            ViscaSocket::S1 => 0,
            ViscaSocket::S2 => 1,
        }
    }

    /// Convert to one-based index (1 or 2).
    ///
    /// This matches the VISCA protocol socket numbering.
    #[must_use]
    pub fn as_socket_number(self) -> u8 {
        match self {
            ViscaSocket::S1 => 1,
            ViscaSocket::S2 => 2,
        }
    }

    /// Convert to VISCA protocol socket byte (0x01 or 0x02).
    ///
    /// This is used in VISCA command addressing.
    #[must_use]
    pub fn as_protocol_byte(self) -> u8 {
        match self {
            ViscaSocket::S1 => 0x01,
            ViscaSocket::S2 => 0x02,
        }
    }

    /// Convert to VISCA cancel command byte (0x21 or 0x22).
    ///
    /// This is used specifically for command cancellation.
    #[must_use]
    pub fn as_cancel_byte(self) -> u8 {
        match self {
            ViscaSocket::S1 => 0x21,
            ViscaSocket::S2 => 0x22,
        }
    }

    /// Create from zero-based index (0 or 1).
    ///
    /// # Errors
    /// Returns `None` if the index is not 0 or 1.
    #[must_use]
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(ViscaSocket::S1),
            1 => Some(ViscaSocket::S2),
            _ => None,
        }
    }

    /// Create from one-based socket number (1 or 2).
    ///
    /// # Errors
    /// Returns `None` if the socket number is not 1 or 2.
    #[must_use]
    pub fn from_socket_number(socket: u8) -> Option<Self> {
        match socket {
            1 => Some(ViscaSocket::S1),
            2 => Some(ViscaSocket::S2),
            _ => None,
        }
    }

    /// Create from VISCA protocol byte.
    ///
    /// Accepts both socket addressing bytes (0x01, 0x02) and cancel bytes (0x21, 0x22).
    ///
    /// # Errors
    /// Returns `None` if the byte doesn't represent a valid socket.
    #[must_use]
    pub fn from_protocol_byte(byte: u8) -> Option<Self> {
        match byte & 0x0F {
            1 => Some(ViscaSocket::S1),
            2 => Some(ViscaSocket::S2),
            _ => None,
        }
    }
}

impl std::fmt::Display for ViscaSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViscaSocket::S1 => write!(f, "Socket1"),
            ViscaSocket::S2 => write!(f, "Socket2"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_index() {
        assert_eq!(ViscaSocket::S1.as_index(), 0);
        assert_eq!(ViscaSocket::S2.as_index(), 1);
    }

    #[test]
    fn test_as_socket_number() {
        assert_eq!(ViscaSocket::S1.as_socket_number(), 1);
        assert_eq!(ViscaSocket::S2.as_socket_number(), 2);
    }

    #[test]
    fn test_as_protocol_byte() {
        assert_eq!(ViscaSocket::S1.as_protocol_byte(), 0x01);
        assert_eq!(ViscaSocket::S2.as_protocol_byte(), 0x02);
    }

    #[test]
    fn test_as_cancel_byte() {
        assert_eq!(ViscaSocket::S1.as_cancel_byte(), 0x21);
        assert_eq!(ViscaSocket::S2.as_cancel_byte(), 0x22);
    }

    #[test]
    fn test_from_index() {
        assert_eq!(ViscaSocket::from_index(0), Some(ViscaSocket::S1));
        assert_eq!(ViscaSocket::from_index(1), Some(ViscaSocket::S2));
        assert_eq!(ViscaSocket::from_index(2), None);
    }

    #[test]
    fn test_from_socket_number() {
        assert_eq!(ViscaSocket::from_socket_number(1), Some(ViscaSocket::S1));
        assert_eq!(ViscaSocket::from_socket_number(2), Some(ViscaSocket::S2));
        assert_eq!(ViscaSocket::from_socket_number(0), None);
        assert_eq!(ViscaSocket::from_socket_number(3), None);
    }

    #[test]
    fn test_from_protocol_byte() {
        assert_eq!(ViscaSocket::from_protocol_byte(0x01), Some(ViscaSocket::S1));
        assert_eq!(ViscaSocket::from_protocol_byte(0x02), Some(ViscaSocket::S2));
        assert_eq!(ViscaSocket::from_protocol_byte(0x21), Some(ViscaSocket::S1));
        assert_eq!(ViscaSocket::from_protocol_byte(0x22), Some(ViscaSocket::S2));
        assert_eq!(ViscaSocket::from_protocol_byte(0x00), None);
        assert_eq!(ViscaSocket::from_protocol_byte(0x03), None);
    }

    #[test]
    fn test_default() {
        assert_eq!(ViscaSocket::default(), ViscaSocket::S1);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", ViscaSocket::S1), "Socket1");
        assert_eq!(format!("{}", ViscaSocket::S2), "Socket2");
    }
}
