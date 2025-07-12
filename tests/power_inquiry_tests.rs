#![allow(missing_docs)]
#[cfg(test)]
mod tests {
    use grafton_visca::command::response::{parse_response, Response, ResponseType};
    use grafton_visca::command::{Command, PowerInquiry};
    use grafton_visca::InquiryResponse;

    #[test]
    fn test_power_inquiry_command_bytes() {
        let cmd = PowerInquiry;
        let bytes = cmd.try_into_vec().unwrap();
        assert_eq!(bytes, vec![0x81, 0x09, 0x04, 0x00, 0xFF]);
    }

    #[test]
    fn test_power_inquiry_response_type() {
        let cmd = PowerInquiry;
        assert_eq!(cmd.response_type(), Some(ResponseType::Power));
    }

    #[test]
    fn test_power_response_parsing_on() {
        let response = vec![0x90, 0x50, 0x02, 0xFF];
        let parsed = parse_response(&response, &ResponseType::Power).unwrap();

        match parsed {
            Response::InquiryResponse(InquiryResponse::Power { on }) => {
                assert!(on);
            }
            _ => panic!("Expected Power inquiry response"),
        }
    }

    #[test]
    fn test_power_response_parsing_off() {
        let response = vec![0x90, 0x50, 0x03, 0xFF];
        let parsed = parse_response(&response, &ResponseType::Power).unwrap();

        match parsed {
            Response::InquiryResponse(InquiryResponse::Power { on }) => {
                assert!(!on);
            }
            _ => panic!("Expected Power inquiry response"),
        }
    }

    #[test]
    fn test_power_response_invalid_length() {
        let response = vec![0x90, 0x50, 0x02, 0x03, 0xFF]; // Too long
        let result = parse_response(&response, &ResponseType::Power);
        assert!(result.is_err());
    }

    #[test]
    fn test_power_response_ack() {
        let response = vec![0x90, 0x41, 0xFF];
        let parsed = parse_response(&response, &ResponseType::Power).unwrap();

        match parsed {
            Response::CmdAck => (),
            _ => panic!("Expected ACK response"),
        }
    }

    #[test]
    fn test_power_response_completion() {
        let response = vec![0x90, 0x51, 0xFF];
        let parsed = parse_response(&response, &ResponseType::Power).unwrap();

        match parsed {
            Response::Completion => (),
            _ => panic!("Expected Completion response"),
        }
    }
}
