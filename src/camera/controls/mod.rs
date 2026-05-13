//! Method implementations for cameras based on their capabilities.
//!
//! Each module contains trait definitions for camera control methods.
//! Methods are implemented directly on the Camera struct with separate async and blocking implementations.
//!
//! Prefer method-call syntax (`camera.power_on()`) or noun accessors
//! (`camera.power().on()`) when calling these traits. The traits use an
//! associated `Mode` future type, so UFCS-style calls such as
//! `PowerControl::power_on(&camera)` can require extra type annotations in
//! generic code.

pub mod color;
pub mod exposure;
pub mod focus;
pub mod image_processing;
pub mod inquiry;
pub mod menu;
pub mod motion;
pub mod motion_sync;
pub mod nd_filter;
pub mod pan_tilt;
pub mod power;
pub mod presets;
pub mod streaming;
pub mod system;
pub mod tally;
pub mod variable_speed;
pub mod white_balance;
pub mod zoom;

pub use self::{
    color::ColorControl,
    exposure::{ExposureCompensationControl, ExposureControl},
    focus::{FocusControl, FocusLockControl, PushAFControl},
    image_processing::ImageProcessingControl,
    inquiry::{InquiryControl, PanTiltInquiryControl},
    menu::{DirectMenuControl, MenuControl},
    motion::MotionControl,
    motion_sync::MotionSyncControl,
    nd_filter::NdFilterControl,
    pan_tilt::PanTiltControl,
    power::PowerControl,
    presets::PresetsControl,
    streaming::StreamingControl,
    system::SystemControl,
    tally::TallyControl,
    variable_speed::VariableSpeedControl,
    white_balance::WhiteBalanceControl,
    zoom::ZoomControl,
};
