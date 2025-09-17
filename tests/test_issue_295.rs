//! Test for issue #295: TallyGreenInquiry bug fix
//! This verifies that TallyGreenInquiry is correctly classified as an inquiry
//! both by its type metadata (response_type()) and by the new command_kind() method.

use grafton_visca::command::inquiry::TallyGreenInquiry;

#[test]
fn test_tally_green_inquiry_is_detected_as_inquiry() {
    // The TallyGreenInquiry command should be correctly classified as an inquiry
    let inquiry = TallyGreenInquiry;

    // Encode the command
    let mut buffer = [0u8; 32];
    let camera_id = grafton_visca::camera_id::CameraId::default();
    let _len =
        grafton_visca::command::encode::ViscaCommand::write_into(&inquiry, camera_id, &mut buffer)
            .expect("Should encode");

    // Verify that the command has the inquiry byte pattern (0x09 at position 1)
    assert_eq!(buffer[1], 0x09, "Second byte should be 0x09 for inquiry");

    // Verify that the type metadata correctly identifies it as an inquiry (bug fixed)
    let response_type = grafton_visca::command::encode::ViscaCommand::response_kind(&inquiry);
    assert!(
        response_type.is_some(),
        "TallyGreenInquiry should return Some(response_type) for inquiries"
    );

    // Verify that command_kind() correctly returns Inquiry
    let command_kind = grafton_visca::command::encode::ViscaCommand::command_kind(&inquiry);
    assert!(
        matches!(command_kind, grafton_visca::command::CommandKind::Inquiry),
        "TallyGreenInquiry should be classified as CommandKind::Inquiry"
    );
}

// Test that other inquiry commands work correctly with command_kind()
#[test]
fn test_command_kind_for_inquiries_and_commands() {
    use grafton_visca::command::{encode::ViscaCommand, CommandKind};

    // Test an inquiry command
    use grafton_visca::command::inquiry::PowerInquiry;
    let power_inquiry = PowerInquiry;
    assert!(
        matches!(power_inquiry.command_kind(), CommandKind::Inquiry),
        "PowerInquiry should be CommandKind::Inquiry"
    );

    // Test a regular command
    use grafton_visca::command::power::PowerOn;
    let power_on = PowerOn::new();
    assert!(
        matches!(power_on.command_kind(), CommandKind::Command),
        "PowerOn should be CommandKind::Command"
    );
}
