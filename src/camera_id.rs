//! Camera ID type for VISCA protocol addressing.
//!
//! In the VISCA protocol:
//! - Camera IDs range from 1-7 (encoded as 0x81 through 0x87)
//! - ID 8 (0x88) is used for broadcast commands
//! - The controller always uses ID 0 (0x80)

use std::fmt;

use crate::error::Error;

/// Represents a VISCA camera ID for addressing commands.
///
/// Valid camera IDs are 1-7 for individual cameras, or 8 for broadcast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CameraId(u8);

impl CameraId {
    /// Camera 1 (default for most single-camera setups).
    pub const CAMERA_1: Self = CameraId(1);

    /// Camera 2.
    pub const CAMERA_2: Self = CameraId(2);

    /// Camera 3.
    pub const CAMERA_3: Self = CameraId(3);

    /// Camera 4.
    pub const CAMERA_4: Self = CameraId(4);

    /// Camera 5.
    pub const CAMERA_5: Self = CameraId(5);

    /// Camera 6.
    pub const CAMERA_6: Self = CameraId(6);

    /// Camera 7.
    pub const CAMERA_7: Self = CameraId(7);

    /// Broadcast address for sending commands to all cameras.
    pub const BROADCAST: Self = CameraId(8);

    /// Create a new camera ID with validation.
    ///
    /// # Arguments
    /// * `id` - The camera ID (1-7 for individual cameras, 8 for broadcast)
    ///
    /// # Errors
    /// Returns an error if the ID is not in the valid range (1-8).
    pub fn new(id: u8) -> Result<Self, Error> {
        match id {
            1..=8 => Ok(CameraId(id)),
            _ => Err(Error::InvalidCameraId { id }),
        }
    }

    /// Convert the camera ID to the VISCA address byte format.
    ///
    /// Camera IDs are encoded as 0x80 | id, resulting in:
    /// - Camera 1: 0x81
    /// - Camera 2: 0x82
    /// - ...
    /// - Camera 7: 0x87
    /// - Broadcast: 0x88
    #[must_use]
    pub fn to_address_byte(self) -> u8 {
        0x80 | self.0
    }

    /// Get the raw ID value (1-8).
    #[must_use]
    pub fn id(self) -> u8 {
        self.0
    }

    /// Decode the camera that sent a reply from the reply's address byte.
    ///
    /// A VISCA device at address *n* answers with the high nibble `8 + n`, so
    /// camera 1 replies `0x90`, camera 2 `0xA0`, and camera 7 `0xF0`. The low
    /// nibble carries the destination (the controller, address 0) and is not
    /// part of the source address.
    ///
    /// Returns `None` for any byte that is not a reply address: `0x8y` is the
    /// controller/broadcast range (`0x88` is address-set traffic, handled by the
    /// serial handshake, not by response decoding) and anything below that is
    /// not addressed to us at all.
    pub(crate) fn from_reply_address(byte: u8) -> Option<Self> {
        match byte >> 4 {
            nibble @ 0x9..=0xF => Some(CameraId(nibble - 8)),
            _ => None,
        }
    }

    /// Check if this is the broadcast address.
    #[must_use]
    pub fn is_broadcast(self) -> bool {
        self.0 == 8
    }
}

impl fmt::Display for CameraId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_broadcast() {
            write!(f, "Broadcast")
        } else {
            write!(f, "Camera {id}", id = self.0)
        }
    }
}

impl Default for CameraId {
    /// Default to Camera 1, which is the most common single-camera setup.
    fn default() -> Self {
        Self::CAMERA_1
    }
}

impl TryFrom<u8> for CameraId {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        CameraId::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_id_creation() {
        assert!(matches!(CameraId::new(1), Ok(id) if id == CameraId::CAMERA_1));
        assert!(matches!(CameraId::new(7), Ok(id) if id == CameraId::CAMERA_7));
        assert!(matches!(CameraId::new(8), Ok(id) if id == CameraId::BROADCAST));

        assert!(CameraId::new(0).is_err());
        assert!(CameraId::new(9).is_err());
        assert!(CameraId::new(255).is_err());
    }

    #[test]
    fn test_address_byte_conversion() {
        assert_eq!(CameraId::CAMERA_1.to_address_byte(), 0x81);
        assert_eq!(CameraId::CAMERA_2.to_address_byte(), 0x82);
        assert_eq!(CameraId::CAMERA_7.to_address_byte(), 0x87);
        assert_eq!(CameraId::BROADCAST.to_address_byte(), 0x88);
    }

    #[test]
    fn test_is_broadcast() {
        assert!(!CameraId::CAMERA_1.is_broadcast());
        assert!(!CameraId::CAMERA_7.is_broadcast());
        assert!(CameraId::BROADCAST.is_broadcast());
    }

    #[test]
    fn test_display() {
        assert_eq!(CameraId::CAMERA_1.to_string(), "Camera 1");
        assert_eq!(CameraId::CAMERA_7.to_string(), "Camera 7");
        assert_eq!(CameraId::BROADCAST.to_string(), "Broadcast");
    }

    #[test]
    fn test_default() {
        assert_eq!(CameraId::default(), CameraId::CAMERA_1);
    }

    #[test]
    fn test_from_reply_address() {
        // Every chain address replies with the high nibble 8 + n.
        for id in 1..=7u8 {
            let address = (8 + id) << 4;
            assert_eq!(
                CameraId::from_reply_address(address),
                Some(CameraId(id)),
                "reply address 0x{address:02X} is camera {id}"
            );
        }

        assert_eq!(CameraId::from_reply_address(0x90), Some(CameraId::CAMERA_1));
        assert_eq!(CameraId::from_reply_address(0xA0), Some(CameraId::CAMERA_2));
        assert_eq!(CameraId::from_reply_address(0xF0), Some(CameraId::CAMERA_7));

        // The low nibble is the destination address, not part of the source.
        assert_eq!(CameraId::from_reply_address(0x9F), Some(CameraId::CAMERA_1));

        // Not reply addresses: controller (0x80), broadcast/address-set (0x88),
        // and command addresses (0x81-0x87).
        for byte in [0x00, 0x0F, 0x7F, 0x80, 0x81, 0x87, 0x88, 0x8F] {
            assert_eq!(
                CameraId::from_reply_address(byte),
                None,
                "0x{byte:02X} is not a reply address"
            );
        }
    }

    #[test]
    fn test_try_from() {
        assert!(matches!(CameraId::try_from(1), Ok(id) if id == CameraId::CAMERA_1));
        assert!(matches!(CameraId::try_from(8), Ok(id) if id == CameraId::BROADCAST));
        assert!(CameraId::try_from(0).is_err());
        assert!(CameraId::try_from(9).is_err());
    }
}
