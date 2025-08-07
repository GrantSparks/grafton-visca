//! VISCA protocol validation utilities for testing.
//!
//! Provides validation of VISCA command and response formats to ensure
//! protocol compliance during testing.

use std::collections::HashMap;

/// VISCA command terminator byte.
const VISCA_TERMINATOR: u8 = 0xFF;

/// Validation mode for protocol checking
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValidationMode {
    /// Strict validation - all protocol rules enforced
    Strict,
    /// Lenient validation - allows some common variations
    Lenient,
    /// Minimal validation - only basic structure checked
    Minimal,
}

/// Protocol validator for VISCA commands and responses
#[derive(Debug)]
pub struct ProtocolValidator {
    mode: ValidationMode,
    /// Track socket usage for buffer management validation
    sockets_in_use: HashMap<u8, bool>,
    /// Statistics
    commands_sent: usize,
    responses_received: usize,
    errors_detected: usize,
}

/// Protocol validation error
#[derive(Debug, Clone)]
pub struct ProtocolValidationError {
    pub message: String,
    pub byte_index: Option<usize>,
    pub expected: Option<Vec<u8>>,
    pub actual: Option<Vec<u8>>,
}

impl std::fmt::Display for ProtocolValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(idx) = self.byte_index {
            write!(f, " at byte {idx}")?;
        }
        if let Some(ref expected) = self.expected {
            write!(f, ", expected {expected:02X?}")?;
        }
        if let Some(ref actual) = self.actual {
            write!(f, ", got {actual:02X?}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ProtocolValidationError {}

impl ProtocolValidator {
    /// Create a new protocol validator
    pub fn new(mode: ValidationMode) -> Self {
        let mut sockets_in_use = HashMap::new();
        sockets_in_use.insert(0, false);
        sockets_in_use.insert(1, false);

        Self {
            mode,
            sockets_in_use,
            commands_sent: 0,
            responses_received: 0,
            errors_detected: 0,
        }
    }

    /// Validate a VISCA command
    pub fn validate_command(&mut self, command: &[u8]) -> Result<(), ProtocolValidationError> {
        self.commands_sent += 1;

        // Basic structure validation
        if command.len() < 3 {
            self.errors_detected += 1;
            return Err(ProtocolValidationError {
                message: "Command too short".to_string(),
                byte_index: None,
                expected: Some(vec![0x81, 0x01, VISCA_TERMINATOR]),
                actual: Some(command.to_vec()),
            });
        }

        // Check header byte (address)
        let header = command[0];
        if header & 0xF0 != 0x80 {
            self.errors_detected += 1;
            return Err(ProtocolValidationError {
                message: "Invalid header byte".to_string(),
                byte_index: Some(0),
                expected: Some(vec![0x81]),
                actual: Some(vec![header]),
            });
        }

        // Check terminator
        if command[command.len() - 1] != VISCA_TERMINATOR {
            self.errors_detected += 1;
            return Err(ProtocolValidationError {
                message: "Missing terminator".to_string(),
                byte_index: Some(command.len() - 1),
                expected: Some(vec![VISCA_TERMINATOR]),
                actual: Some(vec![command[command.len() - 1]]),
            });
        }

        if self.mode == ValidationMode::Strict {
            // Validate command structure based on command type
            if command.len() > 1 {
                let cmd_type = command[1];
                match cmd_type {
                    0x01 => self.validate_command_packet(command)?,
                    0x09 => self.validate_inquiry_packet(command)?,
                    _ => {
                        self.errors_detected += 1;
                        return Err(ProtocolValidationError {
                            message: "Invalid command type".to_string(),
                            byte_index: Some(1),
                            expected: Some(vec![0x01, 0x09]),
                            actual: Some(vec![cmd_type]),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate a VISCA response
    pub fn validate_response(&mut self, response: &[u8]) -> Result<(), ProtocolValidationError> {
        self.responses_received += 1;

        // Basic structure validation
        if response.len() < 3 {
            self.errors_detected += 1;
            return Err(ProtocolValidationError {
                message: "Response too short".to_string(),
                byte_index: None,
                expected: None,
                actual: Some(response.to_vec()),
            });
        }

        // Check header byte
        let header = response[0];
        if header & 0xF0 != 0x90 {
            self.errors_detected += 1;
            return Err(ProtocolValidationError {
                message: "Invalid response header".to_string(),
                byte_index: Some(0),
                expected: Some(vec![0x90]),
                actual: Some(vec![header]),
            });
        }

        // Check terminator
        if response[response.len() - 1] != VISCA_TERMINATOR {
            self.errors_detected += 1;
            return Err(ProtocolValidationError {
                message: "Missing terminator in response".to_string(),
                byte_index: Some(response.len() - 1),
                expected: Some(vec![VISCA_TERMINATOR]),
                actual: Some(vec![response[response.len() - 1]]),
            });
        }

        // Validate response type
        if response.len() > 1 {
            let response_type = response[1];
            match response_type & 0xF0 {
                0x40 => {
                    // ACK - mark socket as in use
                    let socket = response_type & 0x0F;
                    if socket < 2 {
                        self.sockets_in_use.insert(socket, true);
                    }
                }
                0x50 => {
                    // Completion - mark socket as free
                    let socket = response_type & 0x0F;
                    if socket < 2 {
                        self.sockets_in_use.insert(socket, false);
                    }
                }
                0x60 => {
                    // Error response
                    if self.mode == ValidationMode::Strict && response.len() != 4 {
                        self.errors_detected += 1;
                        return Err(ProtocolValidationError {
                            message: "Invalid error response length".to_string(),
                            byte_index: None,
                            expected: Some(vec![0x90, 0x60, 0x00, VISCA_TERMINATOR]),
                            actual: Some(response.to_vec()),
                        });
                    }
                }
                _ => {
                    if self.mode == ValidationMode::Strict {
                        self.errors_detected += 1;
                        return Err(ProtocolValidationError {
                            message: "Invalid response type".to_string(),
                            byte_index: Some(1),
                            expected: None,
                            actual: Some(vec![response_type]),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a socket is available
    pub fn is_socket_available(&self, socket: u8) -> bool {
        !self.sockets_in_use.get(&socket).copied().unwrap_or(false)
    }

    /// Check if all sockets are free
    pub fn all_sockets_free(&self) -> bool {
        self.sockets_in_use.values().all(|&in_use| !in_use)
    }

    /// Reset the validator state
    pub fn reset(&mut self) {
        self.sockets_in_use.insert(0, false);
        self.sockets_in_use.insert(1, false);
        self.commands_sent = 0;
        self.responses_received = 0;
        self.errors_detected = 0;
    }

    /// Get validation summary
    pub fn get_summary(&self) -> ValidationSummary {
        ValidationSummary {
            commands_sent: self.commands_sent,
            responses_received: self.responses_received,
            errors_detected: self.errors_detected,
            sockets_in_use: self
                .sockets_in_use
                .iter()
                .filter(|(_, &in_use)| in_use)
                .map(|(&socket, _)| socket)
                .collect(),
        }
    }

    // Private helper methods

    fn validate_command_packet(&self, command: &[u8]) -> Result<(), ProtocolValidationError> {
        // Command packets should have at least 4 bytes: header, command, data, terminator
        if command.len() < 4 {
            return Err(ProtocolValidationError {
                message: "Command packet too short".to_string(),
                byte_index: None,
                expected: None,
                actual: Some(command.to_vec()),
            });
        }

        // Validate data bytes (should be 0x00-0x0F for nibbles in strict mode)
        if self.mode == ValidationMode::Strict {
            for (i, &byte) in command[2..command.len() - 1].iter().enumerate() {
                // Some commands use full bytes, so only check obvious nibble positions
                if command.len() > 6 && i > 2 && byte > 0x0F && byte < 0x80 {
                    return Err(ProtocolValidationError {
                        message: "Invalid nibble value in command".to_string(),
                        byte_index: Some(i + 2),
                        expected: Some(vec![0x00, 0x0F]),
                        actual: Some(vec![byte]),
                    });
                }
            }
        }

        Ok(())
    }

    fn validate_inquiry_packet(&self, command: &[u8]) -> Result<(), ProtocolValidationError> {
        // Inquiry packets have specific structure
        if command.len() < 5 {
            return Err(ProtocolValidationError {
                message: "Inquiry packet too short".to_string(),
                byte_index: None,
                expected: None,
                actual: Some(command.to_vec()),
            });
        }

        Ok(())
    }
}

/// Summary of validation results
#[derive(Debug, Clone)]
pub struct ValidationSummary {
    pub commands_sent: usize,
    pub responses_received: usize,
    pub errors_detected: usize,
    pub sockets_in_use: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_command() {
        let mut validator = ProtocolValidator::new(ValidationMode::Strict);

        // Valid power on command
        let command = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        assert!(validator.validate_command(&command).is_ok());

        // Valid inquiry command
        let inquiry = vec![0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR];
        assert!(validator.validate_command(&inquiry).is_ok());
    }

    #[test]
    fn test_validate_invalid_commands() {
        let mut validator = ProtocolValidator::new(ValidationMode::Strict);

        // Too short
        assert!(validator.validate_command(&[0x81, VISCA_TERMINATOR]).is_err());

        // Invalid header
        assert!(validator.validate_command(&[0x71, 0x01, VISCA_TERMINATOR]).is_err());

        // Missing terminator
        assert!(validator.validate_command(&[0x81, 0x01, 0x04]).is_err());
    }

    #[test]
    fn test_socket_tracking() {
        let mut validator = ProtocolValidator::new(ValidationMode::Strict);

        // Initially all sockets are free
        assert!(validator.all_sockets_free());

        // ACK response marks socket as in use
        validator.validate_response(&[0x90, 0x41, VISCA_TERMINATOR]).unwrap();
        assert!(!validator.is_socket_available(1));
        assert!(validator.is_socket_available(0));

        // Completion response frees the socket
        validator.validate_response(&[0x90, 0x51, VISCA_TERMINATOR]).unwrap();
        assert!(validator.is_socket_available(1));
        assert!(validator.all_sockets_free());
    }
}
