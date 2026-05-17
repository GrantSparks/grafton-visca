//! Test for issue #295: TallyGreenInquiry bug fix
//! This verifies that TallyGreenInquiry is correctly classified as an inquiry
//! by its complete command behavior metadata.

use grafton_visca::command::{
    CommandBehavior, CommandKind, InquiryKind, InquiryResponseSpec, PowerInquiry, PowerOn,
    TallyGreenInquiry, ViscaCommand,
};

#[test]
fn test_tally_green_inquiry_is_detected_as_inquiry() {
    // The TallyGreenInquiry command should be correctly classified as an inquiry
    let inquiry = TallyGreenInquiry;

    // Encode the command
    let mut buffer = [0u8; 32];
    let camera_id = grafton_visca::CameraId::default();
    let _len = grafton_visca::command::ViscaCommand::write_into(&inquiry, camera_id, &mut buffer)
        .expect("Should encode");

    // Verify that the command has the inquiry byte pattern (0x09 at position 1)
    assert_eq!(buffer[1], 0x09, "Second byte should be 0x09 for inquiry");

    assert_eq!(
        inquiry.behavior(),
        CommandBehavior::Inquiry(InquiryResponseSpec::Builtin(InquiryKind::TallyGreen))
    );
}

// Test that other inquiry commands work correctly with behavior-derived CommandKind.
#[test]
fn test_command_kind_for_inquiries_and_commands() {
    // Test an inquiry command
    let power_inquiry = PowerInquiry;
    assert!(
        matches!(
            power_inquiry.behavior().command_kind(),
            CommandKind::Inquiry
        ),
        "PowerInquiry should be CommandKind::Inquiry"
    );

    // Test a regular command
    let power_on = PowerOn::new();
    assert!(
        matches!(power_on.behavior().command_kind(), CommandKind::Command),
        "PowerOn should be CommandKind::Command"
    );
}
