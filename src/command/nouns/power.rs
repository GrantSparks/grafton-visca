//! Power requests and power-status readback.

use crate::command::nouns::{forward_inquiry, forward_request};

/// Reads the documented power status, including a reported circuit error.
#[derive(Debug, Clone, Copy)]
pub struct StatusInquiry;
forward_inquiry!(StatusInquiry, crate::command::PowerInquiry; crate::command::PowerInquiry);

/// Powers the camera on and waits for protocol completion.
#[derive(Debug, Clone, Copy)]
pub struct On;
forward_request!(On, crate::command::PowerOn, Plain, rows = [PowerOn]; crate::command::PowerOn::new());

/// Places the camera in standby and waits for protocol completion.
#[derive(Debug, Clone, Copy)]
pub struct Standby;
forward_request!(Standby, crate::command::PowerStandby, Plain, rows = [PowerStandby]; crate::command::PowerStandby::new());
