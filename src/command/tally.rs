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
            let cmd = CommandBuilder::<8>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_PREFIX)
                .append(&[0x02])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Turn red tally light off
        RedOff => {
            let cmd = CommandBuilder::<8>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_PREFIX)
                .append(&[0x03])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Set tally brightness to low
        BrightLo => {
            let cmd = CommandBuilder::<8>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_BRIGHT_PREFIX)
                .append(&[0x04])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Set tally brightness to high
        BrightHi => {
            let cmd = CommandBuilder::<8>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_BRIGHT_PREFIX)
                .append(&[0x05])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Turn green tally light on (FR7 specific)
        GreenOn => {
            let cmd = CommandBuilder::<8>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_GREEN_PREFIX)
                .append(&[0x02])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Turn green tally light off (FR7 specific)
        GreenOff => {
            let cmd = CommandBuilder::<8>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_GREEN_PREFIX)
                .append(&[0x03])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Set tally to flash mode (PTZOptics specific)
        Flash => {
            let cmd = CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_PTZO_PREFIX)
                .append(&[0x01])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Set tally to solid on (PTZOptics specific)
        On => {
            let cmd = CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_PTZO_PREFIX)
                .append(&[0x02])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Turn tally off (PTZOptics specific)
        Off => {
            let cmd = CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::tally::TALLY_PTZO_PREFIX)
                .append(&[0x03])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
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

        let command = CommandBuilder::<7>::from_prefix(constants::tally::TALLY_INQUIRY_PREFIX)
            .with_camera_id(camera_id)
            .build();

        buffer[..7].copy_from_slice(&command);
        Ok(7)
    }

    fn response_type(&self) -> Option<ResponseType> {
        Some(match self {
            Self::Red => ResponseType::TallyRed,
            Self::Green => ResponseType::TallyGreen,
        })
    }
}
