//! Stack-allocated command builder for hybrid const/runtime encoding.

use crate::command::const_encoding::VISCA_TERMINATOR;

/// Stack-allocated command builder for creating VISCA commands.
///
/// This builder allows const construction with compile-time prefixes
/// while supporting runtime parameter encoding.
#[derive(Debug, Clone, Copy)]
pub struct CommandBuilder<const N: usize> {
    buffer: [u8; N],
    position: usize,
}

impl<const N: usize> CommandBuilder<N> {
    /// Create a new builder from a const prefix.
    ///
    /// # Example
    /// ```ignore
    /// // Internal API - not part of public interface
    ///
    /// const PREFIX: &[u8] = &[0x81, 0x01, 0x04, 0x47];
    /// let builder = CommandBuilder::<9>::from_prefix(PREFIX);
    /// let command = builder.build();
    ///
    /// // Verify the command starts with our prefix
    /// assert_eq!(&command[0..4], PREFIX);
    /// ```
    pub const fn from_prefix(prefix: &[u8]) -> Self {
        let mut buffer = [0u8; N];
        let mut i = 0;
        while i < prefix.len() && i < N {
            buffer[i] = prefix[i];
            i += 1;
        }
        Self {
            buffer,
            position: i,
        }
    }

    /// Create an empty builder.
    pub const fn new() -> Self {
        Self {
            buffer: [0u8; N],
            position: 0,
        }
    }

    /// Finalize with terminator and return the complete array.
    pub const fn build(mut self) -> [u8; N] {
        if self.position < N {
            self.buffer[self.position] = VISCA_TERMINATOR;
        }
        self.buffer
    }

    /// Append bytes from a slice.
    pub fn append(&mut self, bytes: &[u8]) -> &mut Self {
        for &b in bytes {
            if self.position < N {
                self.buffer[self.position] = b;
                self.position += 1;
            }
        }
        self
    }

    /// Push a single byte.
    pub fn push(&mut self, b: u8) -> &mut Self {
        if self.position < N {
            self.buffer[self.position] = b;
            self.position += 1;
        }
        self
    }

    /// Add camera ID byte at the beginning of the buffer.
    /// This replaces the default 0x81 with the provided camera ID.
    pub fn with_camera_id(&mut self, camera_id: crate::camera_id::CameraId) -> &mut Self {
        if !self.buffer.is_empty()
            && self.buffer[0] == crate::command::const_encoding::DEFAULT_ADDRESS
        {
            self.buffer[0] = camera_id.to_address_byte();
        }
        self
    }

    /// Add VISCA-encoded 16-bit value (4 bytes) using mutable reference.
    pub fn push_visca_u16(&mut self, value: u16) -> &mut Self {
        if self.position + 4 <= N {
            self.buffer[self.position] = ((value >> 12) & 0x0F) as u8;
            self.buffer[self.position + 1] = ((value >> 8) & 0x0F) as u8;
            self.buffer[self.position + 2] = ((value >> 4) & 0x0F) as u8;
            self.buffer[self.position + 3] = (value & 0x0F) as u8;
            self.position += 4;
        }
        self
    }

    /// Add VISCA-encoded 14-bit value (4 bytes) using mutable reference.
    pub fn push_visca_u14(&mut self, value: u16) -> &mut Self {
        self.push_visca_u16(value & 0x3FFF)
    }

    /// Finalize with terminator and return the number of bytes written.
    pub fn finalize(&mut self) -> usize {
        if self.position < N {
            self.buffer[self.position] = VISCA_TERMINATOR;
            self.position += 1;
        }
        self.position
    }

    /// Add a nibble pair (2 bytes) from a u16 value.
    /// The high nibble (bits 4-7) and low nibble (bits 0-3) are stored as separate bytes.
    pub fn push_nibble_pair(&mut self, value: u16) -> &mut Self {
        if self.position + 2 <= N {
            self.buffer[self.position] = ((value >> 4) & 0x0F) as u8;
            self.buffer[self.position + 1] = (value & 0x0F) as u8;
            self.position += 2;
        }
        self
    }

    /// Copy the built command into the provided buffer.
    /// Returns the number of bytes written.
    pub fn copy_to(&self, buffer: &mut [u8]) -> Result<usize, crate::Error> {
        let len = self.position;
        if buffer.len() < len {
            return Err(crate::Error::BufferTooSmall {
                required: len,
                actual: buffer.len(),
            });
        }
        buffer[..len].copy_from_slice(&self.buffer[..len]);
        Ok(len)
    }
}

impl<const N: usize> Default for CommandBuilder<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {}
