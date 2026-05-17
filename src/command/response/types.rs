//! Core response types for VISCA commands.

use std::borrow::Cow;

use smallvec::SmallVec;

use crate::{
    command::inquiry_structs::{InquiryData, InquiryKind},
    error::Error,
    ViscaSocket,
};

const INLINE_RAW_INQUIRY_PAYLOAD_SIZE: usize = 24;

/// Raw VISCA data payload returned by a custom inquiry.
///
/// The payload is the bytes after the VISCA data-reply header (`90 50`) and
/// before the frame terminator (`FF`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawInquiryPayload {
    bytes: SmallVec<[u8; INLINE_RAW_INQUIRY_PAYLOAD_SIZE]>,
}

impl RawInquiryPayload {
    /// Create a raw inquiry payload by copying bytes into inline-backed storage.
    #[inline]
    pub(crate) fn from_slice(payload: &[u8]) -> Self {
        Self {
            bytes: SmallVec::from_slice(payload),
        }
    }

    /// Return the raw VISCA data payload bytes.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

/// Response from a VISCA command.
///
/// Represents all possible responses from the camera including acknowledgments,
/// completions, errors, and inquiry data.
#[derive(Debug)]
#[non_exhaustive]
pub enum Response {
    /// Acknowledgment that the command was received and is being processed
    CmdAck {
        /// Socket that acknowledged, if available
        socket: Option<ViscaSocket>,
    },
    /// Command completed successfully (no data returned)
    Completion {
        /// Socket that completed, if available
        socket: Option<ViscaSocket>,
    },
    /// Command failed with an error
    Error(Error),
    /// Inquiry command response containing requested data
    Inquiry(InquiryData),
    /// Raw custom inquiry response containing the VISCA data payload.
    RawInquiry(RawInquiryPayload),
    /// Unknown response format with type information and raw data
    Unknown {
        /// The response type that could not be parsed
        response_type: Option<InquiryKind>,
        /// Raw response data for debugging
        data: Vec<u8>,
    },
}

impl Response {
    /// Convert response to a Result, treating Completion as Ok and Error as Err.
    ///
    /// Note: ACK responses are treated as an error because they only indicate
    /// the command was queued, not completed. Callers should wait for the
    /// subsequent Completion response.
    pub fn into_result(self) -> Result<(), Error> {
        match self {
            Response::Completion { .. } => Ok(()),
            Response::CmdAck { .. } => Err(Error::CommandPending), // ACK means command is queued, not completed
            Response::Error(e) => Err(e),
            Response::Inquiry(_) => Ok(()), // Inquiry responses are success
            Response::RawInquiry(_) => Ok(()), // Raw inquiry responses are success
            Response::Unknown { data, .. } => Err(Error::InvalidResponse {
                expected: Cow::Borrowed("Known response type"),
                actual: data,
            }),
        }
    }
}
