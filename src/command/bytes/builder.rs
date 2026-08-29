//! Stack-allocated command builder for hybrid const/runtime encoding.

use core::marker::PhantomData;

#[cfg(test)]
#[cfg(test)]
use crate::command::bytes::FixedCommandBytes;
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
/// # Overflow Protection
///
/// The builder tracks the total required bytes and detects buffer overflow.
/// If operations would exceed the buffer capacity, they are skipped and an
/// error is returned at finalization time.
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
    required: usize,  // Total bytes required (including those that couldn't fit)
    overflowed: bool, // True if any write was skipped due to lack of space
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
        // Track if prefix was truncated
        let overflowed = prefix.len() > N;
        let required = prefix.len();

        Self {
            buffer,
            position: i,
            required,
            overflowed,
            _state: PhantomData,
        }
    }

    /// Create an empty builder.
    pub const fn new() -> Self {
        Self {
            buffer: [0u8; N],
            position: 0,
            required: 0,
            overflowed: false,
            _state: PhantomData,
        }
    }

    /// Append bytes from a slice.
    pub fn append(mut self, bytes: &[u8]) -> Self {
        self.required += bytes.len();
        for &b in bytes {
            if self.position < N {
                self.buffer[self.position] = b;
                self.position += 1;
            } else {
                self.overflowed = true;
            }
        }
        self
    }

    /// Push a single byte.
    pub fn push(mut self, b: u8) -> Self {
        self.required += 1;
        if self.position < N {
            self.buffer[self.position] = b;
            self.position += 1;
        } else {
            self.overflowed = true;
        }
        self
    }

    /// Add camera ID byte at the beginning of the buffer.
    /// This replaces the default 0x81 with the provided camera ID.
    pub fn with_camera_id(mut self, camera_id: crate::CameraId) -> Self {
        if !self.buffer.is_empty() && self.buffer[0] == crate::command::bytes::DEFAULT_CAMERA_ID {
            self.buffer[0] = camera_id.to_address_byte();
        }
        self
    }

    /// Add VISCA-encoded 16-bit value (4 bytes).
    pub fn push_visca_u16(mut self, value: u16) -> Self {
        self.required += 4;
        if self.position + 4 <= N {
            self.buffer[self.position] = ((value >> 12) & 0x0F) as u8;
            self.buffer[self.position + 1] = ((value >> 8) & 0x0F) as u8;
            self.buffer[self.position + 2] = ((value >> 4) & 0x0F) as u8;
            self.buffer[self.position + 3] = (value & 0x0F) as u8;
            self.position += 4;
        } else {
            self.overflowed = true;
        }
        self
    }

    /// Add a nibble pair (2 bytes) from a u16 value.
    /// The high nibble (bits 4-7) and low nibble (bits 0-3) are stored as separate bytes.
    pub fn push_nibble_pair(mut self, value: u16) -> Self {
        self.required += 2;
        if self.position + 2 <= N {
            self.buffer[self.position] = ((value >> 4) & 0x0F) as u8;
            self.buffer[self.position + 1] = (value & 0x0F) as u8;
            self.position += 2;
        } else {
            self.overflowed = true;
        }
        self
    }

    /// Mutable append bytes from a slice.
    pub fn append_mut(&mut self, bytes: &[u8]) -> &mut Self {
        self.required += bytes.len();
        for &b in bytes {
            if self.position < N {
                self.buffer[self.position] = b;
                self.position += 1;
            } else {
                self.overflowed = true;
            }
        }
        self
    }

    /// Mutable push a single byte.
    pub fn push_mut(&mut self, b: u8) -> &mut Self {
        self.required += 1;
        if self.position < N {
            self.buffer[self.position] = b;
            self.position += 1;
        } else {
            self.overflowed = true;
        }
        self
    }

    /// Mutable camera ID method.
    pub fn with_camera_id_mut(&mut self, camera_id: crate::CameraId) -> &mut Self {
        if !self.buffer.is_empty() && self.buffer[0] == crate::command::bytes::DEFAULT_CAMERA_ID {
            self.buffer[0] = camera_id.to_address_byte();
        }
        self
    }

    /// Mutable VISCA-encoded 16-bit value.
    pub fn push_visca_u16_mut(&mut self, value: u16) -> &mut Self {
        self.required += 4;
        if self.position + 4 <= N {
            self.buffer[self.position] = ((value >> 12) & 0x0F) as u8;
            self.buffer[self.position + 1] = ((value >> 8) & 0x0F) as u8;
            self.buffer[self.position + 2] = ((value >> 4) & 0x0F) as u8;
            self.buffer[self.position + 3] = (value & 0x0F) as u8;
            self.position += 4;
        } else {
            self.overflowed = true;
        }
        self
    }

    /// Mutable nibble pair.
    pub fn push_nibble_pair_mut(&mut self, value: u16) -> &mut Self {
        self.required += 2;
        if self.position + 2 <= N {
            self.buffer[self.position] = ((value >> 4) & 0x0F) as u8;
            self.buffer[self.position + 1] = (value & 0x0F) as u8;
            self.position += 2;
        } else {
            self.overflowed = true;
        }
        self
    }

    /// Terminate the command by adding the VISCA terminator byte.
    /// This consumes the builder and returns a terminated version.
    pub fn terminate(mut self) -> ConstCommandBuilder<N, Terminated> {
        // Check if we need to add terminator
        let needs_terminator =
            self.position == 0 || self.buffer[self.position - 1] != VISCA_TERMINATOR;

        if needs_terminator {
            self.required += 1;
            if self.position < N {
                self.buffer[self.position] = VISCA_TERMINATOR;
                self.position += 1;
            } else {
                self.overflowed = true;
            }
        }

        // Note: We don't validate here since this is the type-safe path.
        // The terminator is guaranteed by the builder logic.

        ConstCommandBuilder {
            buffer: self.buffer,
            position: self.position,
            required: self.required,
            overflowed: self.overflowed,
            _state: PhantomData,
        }
    }

    // Note: build_into() is removed from Incomplete state.
    // Use terminate().build_into() instead for the type-safe pattern.
}

// Methods available only in Terminated state
impl<const N: usize> ConstCommandBuilder<N, Terminated> {
    /// Get the command bytes as a slice.
    /// This is only available after the command has been terminated.
    ///
    /// Returns only the meaningful bytes (up to and including the terminator),
    /// not the full buffer capacity.
    #[cfg(test)]
    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer[..self.position]
    }

    /// Get the number of bytes in the terminated command.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.position
    }

    /// Build the command, returning a length-aware buffer.
    ///
    /// This method returns a [`FixedCommandBytes`] that carries both the bytes
    /// and the actual encoded length, preventing accidental transmission of
    /// trailing bytes.
    ///
    /// # Errors
    ///
    /// Returns `Error::BufferTooSmall` if the command would overflow the buffer.
    #[cfg(test)]
    pub fn try_build(self) -> Result<FixedCommandBytes<N>, crate::Error> {
        if self.overflowed {
            return Err(crate::Error::BufferTooSmall {
                required: self.required,
                actual: N,
            });
        }
        Ok(FixedCommandBytes::new(self.buffer, self.position))
    }

    /// Build the terminated command into the provided buffer.
    /// Returns the number of bytes written.
    pub fn build_into(&self, buffer: &mut [u8]) -> Result<usize, crate::Error> {
        // Check for internal buffer overflow
        if self.overflowed {
            return Err(crate::Error::BufferTooSmall {
                required: self.required,
                actual: N,
            });
        }

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

    /// Blanket fallback used to assert, at compile time, that the `Incomplete`
    /// state carries no inherent `as_bytes`.
    ///
    /// Rust resolves inherent methods before trait methods, so a call to
    /// `as_bytes` on a builder falls through to this fallback *only* while no
    /// inherent method of that name applies. Binding each call's result to an
    /// explicit type therefore turns "the method does not exist" into a type
    /// error rather than a comment: give `ConstCommandBuilder<N, Incomplete>`
    /// an inherent `as_bytes` and this module stops compiling.
    ///
    /// This cannot be a public compile-contract fixture: `command::bytes` is `pub(crate)`,
    /// so no external fixture crate can name the builder at all, and a fixture
    /// that tried would fail on the module privacy rather than on the type
    /// state — a green check for the wrong reason.
    trait AsBytesFallback {
        fn as_bytes(&self) -> &'static str {
            "fallback"
        }
    }

    impl<T> AsBytesFallback for T {}

    #[test]
    fn the_incomplete_state_has_no_inherent_as_bytes() {
        let builder = ConstCommandBuilder::<10>::new()
            .push(0x81)
            .push(0x01)
            .push(0x04)
            .push(0x47);

        // Resolves to the fallback, which is only possible while the
        // `Incomplete` state has no `as_bytes` of its own.
        let unterminated: &'static str = builder.as_bytes();
        assert_eq!(unterminated, "fallback");

        // The same call on the terminated state resolves to the inherent
        // method instead, which is what makes the assertion above meaningful
        // rather than a property of the fallback trait.
        let terminated = builder.terminate();
        let bytes: &[u8] = terminated.as_bytes();
        assert_eq!(bytes, &[0x81, 0x01, 0x04, 0x47, VISCA_TERMINATOR]);
    }

    #[test]
    fn test_terminated_builder_provides_access() {
        let builder = ConstCommandBuilder::<10>::from_prefix(&[0x81, 0x01, 0x04, 0x47]);
        let terminated = builder.terminate();

        // Can access bytes via as_bytes() - returns only meaningful bytes
        let bytes = terminated.as_bytes();
        assert_eq!(bytes[0..4], [0x81, 0x01, 0x04, 0x47]);
        assert_eq!(bytes[4], VISCA_TERMINATOR);
        assert_eq!(terminated.len(), 5);

        // Verify as_bytes() returns exactly len() bytes
        assert_eq!(bytes.len(), terminated.len());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_try_build_returns_fixed_command_bytes() {
        // Test try_build() method
        let builder = ConstCommandBuilder::<10>::from_prefix(&[0x81, 0x01, 0x04, 0x47]);
        let result = builder.terminate().try_build();
        assert!(result.is_ok());
        let fixed = result.unwrap();
        assert_eq!(fixed.len(), 5);
        assert_eq!(
            fixed.as_slice(),
            &[0x81, 0x01, 0x04, 0x47, VISCA_TERMINATOR]
        );

        // Test try_build() method with auto-termination
        let result = ConstCommandBuilder::<10>::new()
            .push(0x81)
            .push(0x01)
            .push(0x04)
            .push(0x47)
            .terminate()
            .try_build();
        assert!(result.is_ok());
        let fixed = result.unwrap();
        assert_eq!(fixed.len(), 5);
        assert_eq!(fixed.as_slice().last(), Some(&VISCA_TERMINATOR));
    }

    #[test]
    fn test_overflow_from_prefix() {
        // Create a prefix that's larger than the buffer
        let large_prefix = [0x81, 0x01, 0x04, 0x47, 0x00, 0x01];
        let builder = ConstCommandBuilder::<4>::from_prefix(&large_prefix);

        // Should detect overflow when trying to build
        let mut buffer = [0u8; 10];
        let result = builder.terminate().build_into(&mut buffer);

        // 6 bytes for prefix + 1 for terminator = 7 required, but only 4 available
        assert!(matches!(
            result,
            Err(crate::Error::BufferTooSmall {
                required: 7,
                actual: 4
            })
        ));
    }

    #[test]
    fn test_overflow_append() {
        let builder = ConstCommandBuilder::<5>::new();
        let builder = builder.append(&[0x81, 0x01, 0x04, 0x47, 0x00, 0x01]);

        let mut buffer = [0u8; 10];
        let result = builder.terminate().build_into(&mut buffer);

        // 6 bytes + 1 for terminator = 7 required, but only 5 available
        assert!(matches!(
            result,
            Err(crate::Error::BufferTooSmall {
                required: 7,
                actual: 5
            })
        ));
    }

    #[test]
    fn test_overflow_push() {
        let builder = ConstCommandBuilder::<3>::new()
            .push(0x81)
            .push(0x01)
            .push(0x04)
            .push(0x47); // This one won't fit

        let mut buffer = [0u8; 10];
        let result = builder.terminate().build_into(&mut buffer);

        // 4 bytes + 1 for terminator = 5 required, but only 3 available
        assert!(matches!(
            result,
            Err(crate::Error::BufferTooSmall {
                required: 5,
                actual: 3
            })
        ));
    }

    #[test]
    fn test_overflow_push_visca_u16() {
        let builder = ConstCommandBuilder::<5>::new()
            .push(0x81)
            .push(0x01)
            .push_visca_u16(0x1234); // Needs 4 bytes, won't fit

        let mut buffer = [0u8; 10];
        let result = builder.terminate().build_into(&mut buffer);

        // 2 bytes + 4 for u16 + 1 for terminator = 7 required, but only 5 available
        assert!(matches!(
            result,
            Err(crate::Error::BufferTooSmall {
                required: 7,
                actual: 5
            })
        ));
    }

    #[test]
    fn test_overflow_push_nibble_pair() {
        let builder = ConstCommandBuilder::<3>::new()
            .push(0x81)
            .push(0x01)
            .push_nibble_pair(0x12); // Needs 2 bytes, won't fit

        let mut buffer = [0u8; 10];
        let result = builder.terminate().build_into(&mut buffer);

        // 2 bytes + 2 for nibble pair + 1 for terminator = 5 required, but only 3 available
        assert!(matches!(
            result,
            Err(crate::Error::BufferTooSmall {
                required: 5,
                actual: 3
            })
        ));
    }

    #[test]
    fn test_overflow_mutable_methods() {
        let mut builder = ConstCommandBuilder::<4>::new();
        builder.append_mut(&[0x81, 0x01]);
        builder.push_mut(0x04);
        builder.push_mut(0x47);
        builder.push_mut(0x00); // This one won't fit

        let mut buffer = [0u8; 10];
        let result = builder.terminate().build_into(&mut buffer);

        // 5 bytes + 1 for terminator = 6 required, but only 4 available
        assert!(matches!(
            result,
            Err(crate::Error::BufferTooSmall {
                required: 6,
                actual: 4
            })
        ));
    }

    #[test]
    fn test_overflow_no_room_for_terminator() {
        // Fill the buffer exactly, leaving no room for terminator
        let builder = ConstCommandBuilder::<4>::new()
            .push(0x81)
            .push(0x01)
            .push(0x04)
            .push(0x47);

        let mut buffer = [0u8; 10];
        let result = builder.terminate().build_into(&mut buffer);

        // 4 bytes + 1 for terminator = 5 required, but only 4 available
        assert!(matches!(
            result,
            Err(crate::Error::BufferTooSmall {
                required: 5,
                actual: 4
            })
        ));
    }

    #[test]
    fn test_terminated_overflow_detection() {
        // Create a builder that will overflow
        let builder = ConstCommandBuilder::<3>::new()
            .push(0x81)
            .push(0x01)
            .push(0x04)
            .push(0x47); // Won't fit

        let terminated = builder.terminate();

        let mut buffer = [0u8; 10];
        let result = terminated.build_into(&mut buffer);

        // Should still detect overflow in terminated state
        assert!(matches!(
            result,
            Err(crate::Error::BufferTooSmall {
                required: 5,
                actual: 3
            })
        ));
    }

    #[test]
    fn test_try_build_returns_error_on_overflow() {
        // try_build() method should return error on overflow instead of panicking
        let result = ConstCommandBuilder::<3>::new()
            .push(0x81)
            .push(0x01)
            .push(0x04)
            .push(0x47) // Won't fit
            .terminate()
            .try_build();

        assert!(matches!(
            result,
            Err(crate::Error::BufferTooSmall {
                required: 5,
                actual: 3
            })
        ));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_exact_fit_with_terminator() {
        // Exactly fits including terminator
        let builder = ConstCommandBuilder::<5>::new()
            .push(0x81)
            .push(0x01)
            .push(0x04)
            .push(0x47);

        // Test with try_build()
        let result = builder.terminate().try_build();
        assert!(result.is_ok(), "try_build should succeed with exact fit");
        let fixed = result.unwrap();
        assert_eq!(fixed.len(), 5);
        assert_eq!(fixed.as_slice().last(), Some(&VISCA_TERMINATOR));

        // Also test with build_into()
        let builder2 = ConstCommandBuilder::<5>::new()
            .push(0x81)
            .push(0x01)
            .push(0x04)
            .push(0x47);
        let mut buffer = [0u8; 10];
        let result = builder2.terminate().build_into(&mut buffer);
        assert!(result.is_ok(), "build_into should succeed with exact fit");
        assert_eq!(result.unwrap(), 5);
        assert_eq!(buffer[4], VISCA_TERMINATOR);
    }

    #[test]
    fn test_push_visca_u16_mut_overflow() {
        let mut builder = ConstCommandBuilder::<5>::new();
        builder.push_mut(0x81);
        builder.push_mut(0x01);
        builder.push_visca_u16_mut(0x1234); // Needs 4 bytes, won't fit

        let mut buffer = [0u8; 10];
        let result = builder.terminate().build_into(&mut buffer);

        assert!(matches!(
            result,
            Err(crate::Error::BufferTooSmall {
                required: 7,
                actual: 5
            })
        ));
    }

    #[test]
    fn test_push_nibble_pair_mut_overflow() {
        let mut builder = ConstCommandBuilder::<3>::new();
        builder.push_mut(0x81);
        builder.push_mut(0x01);
        builder.push_nibble_pair_mut(0x12); // Needs 2 bytes, won't fit

        let mut buffer = [0u8; 10];
        let result = builder.terminate().build_into(&mut buffer);

        assert!(matches!(
            result,
            Err(crate::Error::BufferTooSmall {
                required: 5,
                actual: 3
            })
        ));
    }
}
