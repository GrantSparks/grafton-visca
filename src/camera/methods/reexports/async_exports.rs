//! Async trait exports.

pub use crate::camera::methods::{
    color::ColorOps,
    exposure::{ExposureCompensationOps, ExposureOps},
    focus::FocusOps,
    image_processing::ImageProcessingOps,
    inquiry::{InquiryOps, PanTiltInquiryOps},
    menu::MenuControlOps,
    motion_sync::MotionSyncControl,
    nd_filter::NDFilterOps,
    pan_tilt::PanTiltOps,
    power::PowerOps,
    presets::PresetsOps,
    streaming::StreamingOps,
    system::SystemOps,
    tally::TallyOps,
    variable_speed::VariableSpeedOps,
    white_balance::WhiteBalanceOps,
    zoom::ZoomOps,
};
