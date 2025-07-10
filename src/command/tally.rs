//! Tally light control commands for VISCA cameras.
//!
//! This module provides commands for controlling tally lights on compatible cameras.
//! Tally lights indicate when a camera is active or being used.

use crate::{
    command::{Command, ResponseType},
    error::Error,
    timeout::CommandCategory,
};

/// Tally light control commands.
///
/// Controls the tally light indicators on compatible cameras.
/// Not all cameras support all tally light features.
#[derive(Debug, Copy, Clone)]
pub enum TallyCommand {
    /// Turn red tally light on
    RedOn,
    /// Turn red tally light off
    RedOff,
    /// Set tally brightness to low
    BrightLo,
    /// Set tally brightness to high
    BrightHi,
    /// Turn green tally light on (FR7 specific)
    GreenOn,
    /// Turn green tally light off (FR7 specific)
    GreenOff,
    /// Set tally to flash mode (PTZOptics specific)
    Flash,
    /// Set tally to solid on (PTZOptics specific)
    On,
    /// Turn tally off (PTZOptics specific)
    Off,
}

impl Command for TallyCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(match self {
            Self::RedOn => vec![0x81, 0x01, 0x7E, 0x01, 0x0A, 0x00, 0x02, 0xFF],
            Self::RedOff => vec![0x81, 0x01, 0x7E, 0x01, 0x0A, 0x00, 0x03, 0xFF],
            Self::BrightLo => vec![0x81, 0x01, 0x7E, 0x01, 0x0A, 0x01, 0x04, 0xFF],
            Self::BrightHi => vec![0x81, 0x01, 0x7E, 0x01, 0x0A, 0x01, 0x05, 0xFF],
            Self::GreenOn => vec![0x81, 0x01, 0x7E, 0x04, 0x1A, 0x00, 0x02, 0xFF],
            Self::GreenOff => vec![0x81, 0x01, 0x7E, 0x04, 0x1A, 0x00, 0x03, 0xFF],
            Self::Flash => vec![0x81, 0x0A, 0x02, 0x02, 0x01, 0xFF],
            Self::On => vec![0x81, 0x0A, 0x02, 0x02, 0x02, 0xFF],
            Self::Off => vec![0x81, 0x0A, 0x02, 0x02, 0x03, 0xFF],
        })
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Tally inquiry commands.
///
/// Queries the current state of tally lights.
#[derive(Debug, Copy, Clone)]
pub enum TallyInquiryCommand {
    /// Query red tally light state
    Red,
    /// Query green tally light state (FR7 specific)
    Green,
}

impl Command for TallyInquiryCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(match self {
            Self::Red => vec![0x81, 0x09, 0x7E, 0x01, 0x0A, 0x00, 0xFF],
            Self::Green => vec![0x81, 0x09, 0x7E, 0x04, 0x1A, 0x00, 0xFF],
        })
    }

    fn response_type(&self) -> Option<ResponseType> {
        Some(match self {
            Self::Red => ResponseType::TallyRed,
            Self::Green => ResponseType::TallyGreen,
        })
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}
