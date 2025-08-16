//! Tally light control commands for VISCA cameras.
//!
//! This module provides commands for controlling tally lights on compatible cameras.
//! Tally lights indicate when a camera is active or being used.
//!
//! # VISCA Compliance
//! Basic red tally light control is part of baseline VISCA.
//!
//! ## Vendor-Specific Features
//! - Green tally light (`GreenOn`, `GreenOff`) - Sony FR7 specific
//! - Flash/solid modes (`Flash`, `On`, `Off`) - PTZOptics specific

use crate::macros::internal::*;

use crate::{
    command::{const_encoding::builder::CommandBuilder, encode_visca::EncodeVisca, ResponseType},
    error::Error,
    timeout::CommandCategory,
};

visca_command! {
    /// Tally light control commands.
    ///
    /// Controls the tally light indicators on compatible cameras.
    /// Not all cameras support all tally light features.
    category = "Quick",
    enum Tally {
        /// Turn red tally light on
        RedOn => {
            Ok(CommandBuilder::<16>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_PREFIX)
                .append(&[0x02]))
        },
        /// Turn red tally light off
        RedOff => {
            Ok(CommandBuilder::<16>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_PREFIX)
                .append(&[0x03]))
        },
        /// Set tally brightness to low
        BrightLo => {
            Ok(CommandBuilder::<16>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_BRIGHT_PREFIX)
                .append(&[0x04]))
        },
        /// Set tally brightness to high
        BrightHi => {
            Ok(CommandBuilder::<16>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_BRIGHT_PREFIX)
                .append(&[0x05]))
        },
        /// Turn green tally light on (FR7 specific)
        GreenOn => {
            Ok(CommandBuilder::<16>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_GREEN_PREFIX)
                .append(&[0x02]))
        },
        /// Turn green tally light off (FR7 specific)
        GreenOff => {
            Ok(CommandBuilder::<16>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_GREEN_PREFIX)
                .append(&[0x03]))
        },
        /// Set tally to flash mode (PTZOptics specific)
        Flash => {
            Ok(CommandBuilder::<16>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_PTZO_PREFIX)
                .append(&[0x01]))
        },
        /// Set tally to solid on (PTZOptics specific)
        On => {
            Ok(CommandBuilder::<16>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_PTZO_PREFIX)
                .append(&[0x02]))
        },
        /// Turn tally off (PTZOptics specific)
        Off => {
            Ok(CommandBuilder::<16>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_PTZO_PREFIX)
                .append(&[0x03]))
        },
    }
}

/// Tally inquiry commands.
///
/// Queries the current state of tally lights.
#[derive(Debug, Copy, Clone)]
pub enum TallyInquiry {
    /// Query red tally light state
    Red,
    /// Query green tally light state (FR7 specific)
    Green,
}

impl EncodeVisca for TallyInquiry {
    type Response = ();
    const MAX_SIZE: usize = 7;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::const_encoding::constants;

        match self {
            Self::Red => CommandBuilder::<7>::from_prefix(constants::inquiry::TALLY_STATUS)
                .with_camera_id(camera_id)
                .build_into(buffer),
            Self::Green => CommandBuilder::<7>::from_prefix(constants::inquiry::TALLY_GREEN)
                .with_camera_id(camera_id)
                .build_into(buffer),
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        Some(match self {
            Self::Red => ResponseType::TallyRed,
            Self::Green => ResponseType::TallyGreen,
        })
    }
}
