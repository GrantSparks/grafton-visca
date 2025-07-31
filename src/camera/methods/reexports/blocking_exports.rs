//! Blocking trait exports.

pub use crate::camera::methods::{
    color::ColorOpsBlocking,
    exposure::{ExposureCompensationOpsBlocking, ExposureOpsBlocking},
    focus::FocusOpsBlocking,
    image_processing::ImageProcessingOpsBlocking,
    inquiry::{InquiryOpsBlocking, PanTiltInquiryOpsBlocking},
    menu::MenuControlOpsBlocking,
    motion_sync::MotionSyncControlBlocking,
    nd_filter::NDFilterOpsBlocking,
    pan_tilt::PanTiltOpsBlocking,
    power::PowerOpsBlocking,
    presets::PresetsOpsBlocking,
    streaming::StreamingOpsBlocking,
    system::SystemOpsBlocking,
    tally::TallyOpsBlocking,
    variable_speed::VariableSpeedOpsBlocking,
    white_balance::WhiteBalanceOpsBlocking,
    zoom::ZoomOpsBlocking,
};
