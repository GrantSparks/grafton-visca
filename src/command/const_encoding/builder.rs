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
    /// ```
    /// const PREFIX: &[u8] = &[0x81, 0x01, 0x04, 0x47];
    /// let builder = CommandBuilder::<9>::from_prefix(PREFIX);
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
    
    /// Add a single byte.
    pub const fn byte(mut self, b: u8) -> Self {
        if self.position < N {
            self.buffer[self.position] = b;
            self.position += 1;
        }
        self
    }
    
    /// Add multiple bytes.
    pub const fn bytes(mut self, bytes: &[u8]) -> Self {
        let mut i = 0;
        while i < bytes.len() && self.position < N {
            self.buffer[self.position] = bytes[i];
            self.position += 1;
            i += 1;
        }
        self
    }
    
    /// Add VISCA-encoded 16-bit value (4 bytes).
    ///
    /// VISCA encoding splits a 16-bit value into 4 nibbles.
    pub const fn visca_u16(mut self, value: u16) -> Self {
        if self.position + 4 <= N {
            self.buffer[self.position] = ((value >> 12) & 0x0F) as u8;
            self.buffer[self.position + 1] = ((value >> 8) & 0x0F) as u8;
            self.buffer[self.position + 2] = ((value >> 4) & 0x0F) as u8;
            self.buffer[self.position + 3] = (value & 0x0F) as u8;
            self.position += 4;
        }
        self
    }
    
    /// Add VISCA-encoded signed 16-bit value (4 bytes).
    ///
    /// For negative values, the sign is encoded in the high nibble.
    pub const fn visca_i16(mut self, value: i16) -> Self {
        if self.position + 4 <= N {
            // Manual abs implementation for const context
            let abs_val = if value < 0 { 
                (-(value as i32)) as u16 
            } else { 
                value as u16 
            };
            let sign = if value < 0 { 0x0F } else { 0x00 };
            
            self.buffer[self.position] = sign | ((abs_val >> 12) & 0x0F) as u8;
            self.buffer[self.position + 1] = ((abs_val >> 8) & 0x0F) as u8;
            self.buffer[self.position + 2] = ((abs_val >> 4) & 0x0F) as u8;
            self.buffer[self.position + 3] = (abs_val & 0x0F) as u8;
            self.position += 4;
        }
        self
    }
    
    /// Add VISCA-encoded 14-bit value (4 bytes).
    ///
    /// Used for zoom and focus positions.
    pub const fn visca_u14(self, value: u16) -> Self {
        self.visca_u16(value & 0x3FFF)
    }
    
    /// Finalize with terminator and return the complete array.
    pub const fn build(mut self) -> [u8; N] {
        if self.position < N {
            self.buffer[self.position] = VISCA_TERMINATOR;
        }
        self.buffer
    }
    
    /// Get slice of valid bytes (for dynamic sizing).
    pub fn as_bytes(&self) -> &[u8] {
        let end = if self.position < N { self.position + 1 } else { N };
        &self.buffer[..end]
    }
    
    /// Get the current position in the buffer.
    pub const fn position(&self) -> usize {
        self.position
    }
    
    /// Check if the builder has room for more bytes.
    pub const fn has_capacity(&self, bytes: usize) -> bool {
        self.position + bytes <= N
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
}

impl<const N: usize> Default for CommandBuilder<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_builder_from_prefix() {
        const PREFIX: &[u8] = &[0x81, 0x01, 0x04, 0x47];
        let builder = CommandBuilder::<9>::from_prefix(PREFIX);
        let cmd = builder.visca_u16(0x4000).build();
        
        assert_eq!(
            cmd,
            [0x81, 0x01, 0x04, 0x47, 0x04, 0x00, 0x00, 0x00, 0xFF]
        );
    }
    
    #[test]
    fn test_builder_visca_i16() {
        let builder = CommandBuilder::<6>::new();
        let cmd = builder.byte(0x81).visca_i16(-100).build();
        
        // -100 = 0x64 absolute value
        // High nibble has sign bit (0x0F)
        assert_eq!(
            cmd[1..5],
            [0x0F, 0x00, 0x06, 0x04]
        );
    }
    
    #[test]
    fn test_const_construction() {
        const CMD: [u8; 6] = CommandBuilder::<6>::new()
            .byte(0x81)
            .byte(0x01)
            .byte(0x04)
            .byte(0x00)
            .byte(0x02)
            .build();
        
        assert_eq!(CMD, [0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);
    }
}