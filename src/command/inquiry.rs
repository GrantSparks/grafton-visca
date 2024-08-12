use crate::command::ViscaCommand;
use crate::error::ViscaError;

use super::ViscaResponseType;

#[derive(Debug)]
pub enum InquiryCommand {
    PanTiltPosition,
    ZoomPosition,
    FocusPosition,
    ExposureMode,
    WhiteBalanceMode,
    Luminance,
    Contrast,
    // Add other inquiry commands as needed
}

impl ViscaCommand for InquiryCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        let bytes = match self {
            InquiryCommand::PanTiltPosition => vec![0x81, 0x09, 0x06, 0x12, 0xFF],
            InquiryCommand::ZoomPosition => vec![0x81, 0x09, 0x04, 0x47, 0xFF],
            InquiryCommand::FocusPosition => vec![0x81, 0x09, 0x04, 0x48, 0xFF],
            InquiryCommand::ExposureMode => vec![0x81, 0x09, 0x04, 0x39, 0xFF],
            InquiryCommand::WhiteBalanceMode => vec![0x81, 0x09, 0x04, 0x35, 0xFF],
            InquiryCommand::Luminance => vec![0x81, 0x09, 0x04, 0xA1, 0xFF],
            InquiryCommand::Contrast => vec![0x81, 0x09, 0x04, 0xA2, 0xFF],
        };
        Ok(bytes)
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        match self {
            InquiryCommand::PanTiltPosition => Some(ViscaResponseType::PanTiltPosition),
            InquiryCommand::ZoomPosition => Some(ViscaResponseType::ZoomPosition),
            InquiryCommand::FocusPosition => Some(ViscaResponseType::FocusPosition),
            InquiryCommand::ExposureMode => Some(ViscaResponseType::ExposureMode),
            InquiryCommand::WhiteBalanceMode => Some(ViscaResponseType::WhiteBalanceMode),
            InquiryCommand::Luminance => Some(ViscaResponseType::Luminance),
            InquiryCommand::Contrast => Some(ViscaResponseType::Contrast),
        }
    }
}
