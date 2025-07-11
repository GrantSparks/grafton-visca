//! VISCA response building utilities for testing.
//!
//! Provides a fluent API for constructing VISCA response messages,
//! making tests more readable and maintainable.

/// Builder for creating VISCA response messages
#[derive(Debug, Clone)]
pub struct ResponseBuilder {
    bytes: Vec<u8>,
}

impl ResponseBuilder {
    /// Create a new response builder starting with header byte
    pub fn new() -> Self {
        Self { bytes: vec![0x90] }
    }

    /// Create an ACK response for the given socket
    pub fn ack(socket: u8) -> Vec<u8> {
        vec![0x90, 0x40 | (socket & 0x0F), 0xFF]
    }

    /// Create a completion response for the given socket
    pub fn completion(socket: u8) -> Vec<u8> {
        vec![0x90, 0x50 | (socket & 0x0F), 0xFF]
    }

    /// Create an error response with the given error code
    pub fn error(error_code: u8) -> Vec<u8> {
        vec![0x90, 0x60, error_code, 0xFF]
    }

    /// Start building an inquiry response
    pub fn inquiry() -> Self {
        Self {
            bytes: vec![0x90, 0x50],
        }
    }

    /// Add a single byte to the response
    pub fn add_byte(mut self, byte: u8) -> Self {
        self.bytes.push(byte);
        self
    }

    /// Add multiple bytes to the response
    pub fn add_bytes(mut self, bytes: &[u8]) -> Self {
        self.bytes.extend_from_slice(bytes);
        self
    }

    /// Add a u16 value as 4 nibbles (VISCA format)
    pub fn add_u16_nibbles(mut self, value: u16) -> Self {
        self.bytes.push(((value >> 12) & 0x0F) as u8);
        self.bytes.push(((value >> 8) & 0x0F) as u8);
        self.bytes.push(((value >> 4) & 0x0F) as u8);
        self.bytes.push((value & 0x0F) as u8);
        self
    }

    /// Add a u32 value as 7 nibbles (VISCA format for some values)
    pub fn add_u32_nibbles(mut self, value: u32) -> Self {
        self.bytes.push(((value >> 24) & 0x0F) as u8);
        self.bytes.push(((value >> 20) & 0x0F) as u8);
        self.bytes.push(((value >> 16) & 0x0F) as u8);
        self.bytes.push(((value >> 12) & 0x0F) as u8);
        self.bytes.push(((value >> 8) & 0x0F) as u8);
        self.bytes.push(((value >> 4) & 0x0F) as u8);
        self.bytes.push((value & 0x0F) as u8);
        self
    }

    /// Add a boolean as 0x02 (on) or 0x03 (off)
    pub fn add_on_off(mut self, is_on: bool) -> Self {
        self.bytes.push(if is_on { 0x02 } else { 0x03 });
        self
    }

    /// Build the response with terminator
    pub fn build(mut self) -> Vec<u8> {
        self.bytes.push(0xFF);
        self.bytes
    }
}

impl Default for ResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Common response patterns
pub mod patterns {
    use super::ResponseBuilder;

    /// Create a pan/tilt position response
    pub fn pan_tilt_position_response(pan: u16, tilt: u16) -> Vec<u8> {
        ResponseBuilder::inquiry()
            .add_u16_nibbles(pan)
            .add_u16_nibbles(tilt)
            .build()
    }

    /// Create a zoom position response
    pub fn zoom_position_response(position: u16) -> Vec<u8> {
        ResponseBuilder::inquiry().add_u16_nibbles(position).build()
    }

    /// Create a focus position response
    pub fn focus_position_response(position: u16) -> Vec<u8> {
        ResponseBuilder::inquiry().add_u16_nibbles(position).build()
    }

    /// Create an exposure mode response
    pub fn exposure_mode_response(mode: u8) -> Vec<u8> {
        ResponseBuilder::inquiry().add_byte(mode).build()
    }

    /// Create a white balance mode response
    pub fn white_balance_mode_response(mode: u8) -> Vec<u8> {
        ResponseBuilder::inquiry().add_byte(mode).build()
    }

    /// Create a power status response
    pub fn power_status_response(is_on: bool) -> Vec<u8> {
        ResponseBuilder::inquiry().add_on_off(is_on).build()
    }

    /// Create an auto focus sensitivity response
    pub fn auto_focus_sensitivity_response(sensitivity: u8) -> Vec<u8> {
        ResponseBuilder::inquiry().add_byte(sensitivity).build()
    }

    /// Create a picture effect response
    pub fn picture_effect_response(effect: u8) -> Vec<u8> {
        ResponseBuilder::inquiry().add_byte(effect).build()
    }

    /// Create a system information response
    pub fn system_info_response(vendor_id: u16, model_id: u16, rom_version: u16) -> Vec<u8> {
        ResponseBuilder::inquiry()
            .add_u16_nibbles(vendor_id)
            .add_u16_nibbles(model_id)
            .add_u16_nibbles(rom_version)
            .add_byte(0x00) // Socket number (unused)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_responses() {
        // Test ACK
        assert_eq!(ResponseBuilder::ack(1), vec![0x90, 0x41, 0xFF]);

        // Test completion
        assert_eq!(ResponseBuilder::completion(0), vec![0x90, 0x50, 0xFF]);

        // Test error
        assert_eq!(ResponseBuilder::error(0x02), vec![0x90, 0x60, 0x02, 0xFF]);
    }

    #[test]
    fn test_inquiry_builder() {
        // Test u16 nibbles
        let response = ResponseBuilder::inquiry().add_u16_nibbles(0x1234).build();
        assert_eq!(response, vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xFF]);

        // Test mixed content
        let response = ResponseBuilder::inquiry()
            .add_byte(0x05)
            .add_on_off(true)
            .add_u16_nibbles(0xABCD)
            .build();
        assert_eq!(
            response,
            vec![0x90, 0x50, 0x05, 0x02, 0x0A, 0x0B, 0x0C, 0x0D, 0xFF]
        );
    }

    #[test]
    fn test_pattern_helpers() {
        // Test pan/tilt position
        let response = patterns::pan_tilt_position_response(0x1234, 0x5678);
        assert_eq!(
            response,
            vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0xFF]
        );

        // Test power status
        let response = patterns::power_status_response(false);
        assert_eq!(response, vec![0x90, 0x50, 0x03, 0xFF]);
    }
}
