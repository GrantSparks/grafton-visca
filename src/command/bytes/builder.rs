//! Stack-allocated command builder for hybrid const/runtime encoding.

use core::marker::PhantomData;

use crate::command::bytes::VISCA_TERMINATOR;

/// Type state for an incomplete (unterminated) command
#[derive(Debug, Clone, Copy)]
pub struct Incomplete;

/// Type state for a terminated command
#[derive(Debug, Clone, Copy)]
pub struct Terminated;

/// Stack-allocated command builder for creating VISCA commands.
///
/// This builder allows const construction with compile-time prefixes
/// while supporting runtime parameter encoding.
///
/// The type-state pattern ensures that commands are properly terminated
/// before they can be used. Commands start in the `Incomplete` state and
/// must be explicitly terminated to move to the `Terminated` state.
///
/// # Type States
///
/// - `Incomplete`: The default state. Commands can be built but not sent.
/// - `Terminated`: The command has been properly terminated and is ready to send.
///
/// # Examples
///
/// ```ignore
/// // Commands must be terminated before use
/// let builder = ConstCommandBuilder::<9>::from_prefix(&[0x81, 0x01, 0x04, 0x47]);
/// let terminated = builder.terminate(); // Moves to Terminated state
/// let bytes = terminated.as_bytes();    // Only available on Terminated
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ConstCommandBuilder<const N: usize, State = Incomplete> {
    buffer: [u8; N],
    position: usize,
    _state: PhantomData<State>,
}

// Methods available only in Incomplete state
impl<const N: usize> ConstCommandBuilder<N, Incomplete> {
    /// Create a new builder from a const prefix.
    ///
    /// # Example
    /// ```ignore
    /// // Internal API - not part of public interface
    ///
    /// const PREFIX: &[u8] = &[0x81, 0x01, 0x04, 0x47];
    /// let builder = ConstCommandBuilder::<9>::from_prefix(PREFIX);
    /// let terminated = builder.terminate();
    /// let bytes = terminated.as_bytes();
    ///
    /// // Verify the command starts with our prefix
    /// assert_eq!(&bytes[0..4], PREFIX);
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
            _state: PhantomData,
        }
    }

    /// Create an empty builder.
    pub const fn new() -> Self {
        Self {
            buffer: [0u8; N],
            position: 0,
            _state: PhantomData,
        }
    }

    /// Append bytes from a slice.
    pub fn append(mut self, bytes: &[u8]) -> Self {
        for &b in bytes {
            if self.position < N {
                self.buffer[self.position] = b;
                self.position += 1;
            }
        }
        self
    }

    /// Push a single byte.
    pub fn push(mut self, b: u8) -> Self {
        if self.position < N {
            self.buffer[self.position] = b;
            self.position += 1;
        }
        self
    }

    /// Add camera ID byte at the beginning of the buffer.
    /// This replaces the default 0x81 with the provided camera ID.
    pub fn with_camera_id(mut self, camera_id: crate::camera_id::CameraId) -> Self {
        if !self.buffer.is_empty() && self.buffer[0] == crate::command::bytes::DEFAULT_ADDRESS {
            self.buffer[0] = camera_id.to_address_byte();
        }
        self
    }

    /// Add VISCA-encoded 16-bit value (4 bytes).
    pub fn push_visca_u16(mut self, value: u16) -> Self {
        if self.position + 4 <= N {
            self.buffer[self.position] = ((value >> 12) & 0x0F) as u8;
            self.buffer[self.position + 1] = ((value >> 8) & 0x0F) as u8;
            self.buffer[self.position + 2] = ((value >> 4) & 0x0F) as u8;
            self.buffer[self.position + 3] = (value & 0x0F) as u8;
            self.position += 4;
        }
        self
    }

    /// Add VISCA-encoded 14-bit value (4 bytes).
    pub fn push_visca_u14(self, value: u16) -> Self {
        self.push_visca_u16(value & 0x3FFF)
    }

    /// Add a nibble pair (2 bytes) from a u16 value.
    /// The high nibble (bits 4-7) and low nibble (bits 0-3) are stored as separate bytes.
    pub fn push_nibble_pair(mut self, value: u16) -> Self {
        if self.position + 2 <= N {
            self.buffer[self.position] = ((value >> 4) & 0x0F) as u8;
            self.buffer[self.position + 1] = (value & 0x0F) as u8;
            self.position += 2;
        }
        self
    }

    /// Mutable append bytes from a slice (for backward compatibility).
    pub fn append_mut(&mut self, bytes: &[u8]) -> &mut Self {
        for &b in bytes {
            if self.position < N {
                self.buffer[self.position] = b;
                self.position += 1;
            }
        }
        self
    }

    /// Mutable push a single byte (for backward compatibility).
    pub fn push_mut(&mut self, b: u8) -> &mut Self {
        if self.position < N {
            self.buffer[self.position] = b;
            self.position += 1;
        }
        self
    }

    /// Mutable camera ID method (for backward compatibility).
    pub fn with_camera_id_mut(&mut self, camera_id: crate::camera_id::CameraId) -> &mut Self {
        if !self.buffer.is_empty() && self.buffer[0] == crate::command::bytes::DEFAULT_ADDRESS {
            self.buffer[0] = camera_id.to_address_byte();
        }
        self
    }

    /// Mutable VISCA-encoded 16-bit value (for backward compatibility).
    pub fn push_visca_u16_mut(&mut self, value: u16) -> &mut Self {
        if self.position + 4 <= N {
            self.buffer[self.position] = ((value >> 12) & 0x0F) as u8;
            self.buffer[self.position + 1] = ((value >> 8) & 0x0F) as u8;
            self.buffer[self.position + 2] = ((value >> 4) & 0x0F) as u8;
            self.buffer[self.position + 3] = (value & 0x0F) as u8;
            self.position += 4;
        }
        self
    }

    /// Mutable nibble pair (for backward compatibility).
    pub fn push_nibble_pair_mut(&mut self, value: u16) -> &mut Self {
        if self.position + 2 <= N {
            self.buffer[self.position] = ((value >> 4) & 0x0F) as u8;
            self.buffer[self.position + 1] = (value & 0x0F) as u8;
            self.position += 2;
        }
        self
    }

    /// Terminate the command by adding the VISCA terminator byte.
    /// This consumes the builder and returns a terminated version.
    pub fn terminate(mut self) -> ConstCommandBuilder<N, Terminated> {
        if self.position < N {
            self.buffer[self.position] = VISCA_TERMINATOR;
            self.position += 1;
        }

        // Validate terminator in debug builds
        crate::command::encode_visca::validate_terminator(&self.buffer, self.position);

        ConstCommandBuilder {
            buffer: self.buffer,
            position: self.position,
            _state: PhantomData,
        }
    }

    /// Build the command, automatically adding terminator if needed.
    /// This is the standard builder pattern termination method.
    pub fn build(mut self) -> [u8; N] {
        // Automatically add terminator if not already present
        if self.position < N
            && (self.position == 0 || self.buffer[self.position - 1] != VISCA_TERMINATOR)
        {
            self.buffer[self.position] = VISCA_TERMINATOR;
            self.position += 1;
        }

        // Validate terminator (critical safety invariant)
        crate::command::encode_visca::validate_terminator(&self.buffer, self.position);

        self.buffer
    }

    /// Build the command and copy it into the provided buffer.
    /// Returns the number of bytes written.
    pub fn build_into(mut self, buffer: &mut [u8]) -> Result<usize, crate::Error> {
        // Automatically add terminator if not already present
        if self.position < N
            && (self.position == 0 || self.buffer[self.position - 1] != VISCA_TERMINATOR)
        {
            self.buffer[self.position] = VISCA_TERMINATOR;
            self.position += 1;
        }

        // Validate terminator (critical safety invariant)
        crate::command::encode_visca::validate_terminator(&self.buffer, self.position);

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

// Methods available only in Terminated state
impl<const N: usize> ConstCommandBuilder<N, Terminated> {
    /// Get the command bytes as a slice.
    /// This is only available after the command has been terminated.
    ///
    /// This method is part of the type-state API but not currently used.
    /// It's retained for API completeness and will be used when migrating
    /// commands to the type-safe pattern.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer[..self.position]
    }

    /// Get the complete buffer as an array.
    /// This is only available after the command has been terminated.
    pub fn as_array(&self) -> [u8; N] {
        self.buffer
    }

    /// Get the number of bytes in the terminated command.
    pub fn len(&self) -> usize {
        self.position
    }

    /// Check if the terminated command is empty.
    pub fn is_empty(&self) -> bool {
        self.position == 0
    }

    /// Build the terminated command into the provided buffer.
    /// Returns the number of bytes written.
    pub fn build_into(&self, buffer: &mut [u8]) -> Result<usize, crate::Error> {
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

impl<const N: usize> Default for ConstCommandBuilder<N, Incomplete> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_state_enforces_termination() {
        // Create an incomplete builder
        let builder = ConstCommandBuilder::<10>::new()
            .push(0x81)
            .push(0x01)
            .push(0x04)
            .push(0x47);

        // Can't get bytes without terminating (this won't compile if uncommented)
        // let bytes = builder.as_bytes(); // ERROR: method not found

        // Must terminate first
        let terminated = builder.terminate();

        // Now we can get the bytes
        let bytes = terminated.as_bytes();
        assert_eq!(bytes[bytes.len() - 1], VISCA_TERMINATOR);
    }

    #[test]
    fn test_terminated_builder_provides_access() {
        let builder = ConstCommandBuilder::<10>::from_prefix(&[0x81, 0x01, 0x04, 0x47]);
        let terminated = builder.terminate();

        // Can access bytes
        let bytes = terminated.as_bytes();
        assert_eq!(bytes[0..4], [0x81, 0x01, 0x04, 0x47]);
        assert_eq!(bytes[4], VISCA_TERMINATOR);
        assert_eq!(terminated.len(), 5);

        // Can get full array
        let array = terminated.as_array();
        assert_eq!(array[0..4], [0x81, 0x01, 0x04, 0x47]);
    }

    #[test]
    fn test_legacy_methods_still_work() {
        // Test build() method
        let builder = ConstCommandBuilder::<10>::from_prefix(&[0x81, 0x01, 0x04, 0x47]);
        let array = builder.build();
        assert_eq!(array[4], VISCA_TERMINATOR);

        // Test build() method with auto-termination
        let command = ConstCommandBuilder::<10>::new()
            .push(0x81)
            .push(0x01)
            .push(0x04)
            .push(0x47)
            .build();
        assert_eq!(command[4], VISCA_TERMINATOR);
    }
}
