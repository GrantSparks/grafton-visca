//! Tests for high-level control API extension traits.

use grafton_visca::{
    PanTiltDirection, ViscaCommand, ViscaDevice, ViscaError, ViscaFocusExt, ViscaPanTiltExt,
    ViscaPresetExt, ViscaResponse, ViscaZoomExt,
};

/// Mock device for testing control commands
struct MockDevice {
    sent_commands: Vec<Vec<u8>>,
}

impl MockDevice {
    const fn new() -> Self {
        Self {
            sent_commands: Vec::new(),
        }
    }

    const fn with_completion() -> Self {
        Self::new()
    }

    fn last_command(&self) -> &[u8] {
        self.sent_commands.last().expect("No commands sent")
    }
}

impl ViscaDevice for MockDevice {
    fn execute_command(&mut self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError> {
        let data = command.to_bytes()?;
        self.sent_commands.push(data);
        Ok(ViscaResponse::Completion)
    }
}

#[cfg(test)]
mod pan_tilt_tests {
    use super::*;

    #[test]
    fn test_move_to_position_default_speed() {
        let mut device = MockDevice::with_completion();

        device.move_to_position(1000, -500, None).unwrap();

        // Expected: 81 01 06 02 12 0E 03 E8 FE 0C FF (with default speeds 18, 14)
        let cmd = device.last_command();
        assert_eq!(cmd[0..5], [0x81, 0x01, 0x06, 0x02, 18]);
        assert_eq!(cmd[5], 14);
    }

    #[test]
    fn test_move_to_position_custom_speed() {
        let mut device = MockDevice::with_completion();

        device.move_to_position(0, 0, Some((24, 18))).unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd[0..5], [0x81, 0x01, 0x06, 0x02, 24]);
        assert_eq!(cmd[5], 18);
    }

    #[test]
    fn test_move_relative() {
        let mut device = MockDevice::with_completion();

        device.move_relative(100, -50, None).unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd[0..4], [0x81, 0x01, 0x06, 0x03]); // Relative position command
    }

    #[test]
    fn test_start_moving() {
        let mut device = MockDevice::with_completion();

        device
            .start_moving(PanTiltDirection::UpRight, 10, 8)
            .unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd[0..4], [0x81, 0x01, 0x06, 0x01]); // Direction command
        assert_eq!(cmd[4], 10); // Pan speed
        assert_eq!(cmd[5], 8); // Tilt speed
    }

    #[test]
    fn test_stop_movement() {
        let mut device = MockDevice::with_completion();

        device.stop_movement().unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd[0..6], [0x81, 0x01, 0x06, 0x01, 0x00, 0x00]); // Stop with speeds 0
        assert_eq!(cmd[6..8], [0x03, 0x03]); // Stop direction
    }

    #[test]
    fn test_go_home() {
        let mut device = MockDevice::with_completion();

        device.go_home().unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd, [0x81, 0x01, 0x06, 0x04, 0xFF]); // Home command
    }
}

#[cfg(test)]
mod zoom_tests {
    use super::*;

    #[test]
    fn test_zoom_to() {
        let mut device = MockDevice::with_completion();

        device.zoom_to(0x2000).unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd[0..4], [0x81, 0x01, 0x04, 0x47]); // Direct zoom command
                                                         // Position 0x2000 encoded as p=2, q=0, r=0, s=0
        assert_eq!(cmd[4..8], [0x02, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_zoom_in_standard_speed() {
        let mut device = MockDevice::with_completion();

        device.zoom_in(None).unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd, [0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]); // Tele standard
    }

    #[test]
    fn test_zoom_in_variable_speed() {
        let mut device = MockDevice::with_completion();

        device.zoom_in(Some(5)).unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd, [0x81, 0x01, 0x04, 0x07, 0x25, 0xFF]); // Tele variable speed 5
    }

    #[test]
    fn test_zoom_out_standard_speed() {
        let mut device = MockDevice::with_completion();

        device.zoom_out(None).unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd, [0x81, 0x01, 0x04, 0x07, 0x03, 0xFF]); // Wide standard
    }

    #[test]
    fn test_stop_zoom() {
        let mut device = MockDevice::with_completion();

        device.stop_zoom().unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd, [0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]); // Zoom stop
    }
}

#[cfg(test)]
mod focus_tests {
    use super::*;

    #[test]
    fn test_set_auto_focus() {
        let mut device = MockDevice::with_completion();

        device.set_auto_focus(true).unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd, [0x81, 0x01, 0x04, 0x38, 0x02, 0xFF]); // Auto focus on
    }

    #[test]
    fn test_set_manual_focus() {
        let mut device = MockDevice::with_completion();

        device.set_auto_focus(false).unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd, [0x81, 0x01, 0x04, 0x38, 0x03, 0xFF]); // Manual focus
    }

    #[test]
    fn test_focus_to() {
        let mut device = MockDevice::with_completion();

        device.focus_to(0x8000).unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd[0..4], [0x81, 0x01, 0x04, 0x48]); // Direct focus command
                                                         // Position 0x8000 encoded
                                                         // Position 0x8000 encoded as nibbles: 8, 0, 0, 0
        assert_eq!(cmd[4..8], [0x08, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_focus_near() {
        let mut device = MockDevice::with_completion();

        device.focus_near(None).unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd, [0x81, 0x01, 0x04, 0x08, 0x03, 0xFF]); // Near standard
    }

    #[test]
    fn test_focus_far_variable_speed() {
        let mut device = MockDevice::with_completion();

        device.focus_far(Some(6)).unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd, [0x81, 0x01, 0x04, 0x08, 0x26, 0xFF]); // Far variable speed 6
    }

    #[test]
    fn test_trigger_one_push_focus() {
        let mut device = MockDevice::with_completion();

        device.trigger_one_push_focus().unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd, [0x81, 0x01, 0x04, 0x18, 0x01, 0xFF]); // One push trigger
    }
}

#[cfg(test)]
mod preset_tests {
    use super::*;

    #[test]
    fn test_save_preset() {
        let mut device = MockDevice::with_completion();

        device.save_preset(1).unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd, [0x81, 0x01, 0x04, 0x3F, 0x01, 0x01, 0xFF]); // Set preset 1
    }

    #[test]
    fn test_recall_preset() {
        let mut device = MockDevice::with_completion();

        device.recall_preset(5).unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd, [0x81, 0x01, 0x04, 0x3F, 0x02, 0x05, 0xFF]); // Recall preset 5
    }

    #[test]
    fn test_reset_preset() {
        let mut device = MockDevice::with_completion();

        device.reset_preset(89).unwrap();

        let cmd = device.last_command();
        assert_eq!(cmd, [0x81, 0x01, 0x04, 0x3F, 0x00, 0x59, 0xFF]); // Reset preset 89 (max allowed)
    }
}
