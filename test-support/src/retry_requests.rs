//! Plain requests that differ only in their retry class, for the retry
//! recovery tests.
//!
//! Both write the same three-byte frame, so a test that compares them varies
//! the retry class and nothing else.

use grafton_visca::{request, CameraId, ControlClass, Error, Request, RetryClass, TimeoutClass};

/// A plain command in the standard retry class.
///
/// The distinction these tests pin is the retry class, so the command is
/// declared here rather than borrowed from a built-in whose profile support
/// would be a second variable.
#[derive(Debug)]
pub struct StandardCommand;

impl Request for StandardCommand {
    type Class = request::Plain;
    const MAX_SIZE: usize = 3;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
    const RETRY_CLASS: RetryClass = RetryClass::Standard;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn write_into(&self, target: CameraId, out: &mut [u8]) -> Result<usize, Error> {
        out[..3].copy_from_slice(&[target.to_address_byte(), 0x01, 0xff]);
        Ok(3)
    }
}

/// A plain command in the movement retry class and quick timeout class.
///
/// `0x41` is replayed for ordinary movement. A typed STOP is deliberately not
/// this vehicle: a STOP the camera finds not executable (for example a focus
/// STOP under auto-focus) is reported at once rather than rewritten.
#[derive(Debug)]
pub struct MovementCommand;

impl Request for MovementCommand {
    type Class = request::Plain;
    const MAX_SIZE: usize = 3;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
    const RETRY_CLASS: RetryClass = RetryClass::Movement;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn write_into(&self, target: CameraId, out: &mut [u8]) -> Result<usize, Error> {
        out[..3].copy_from_slice(&[target.to_address_byte(), 0x01, 0xff]);
        Ok(3)
    }
}
