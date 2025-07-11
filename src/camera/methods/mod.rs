//! Method implementations for cameras based on their capabilities.
//!
//! Each module provides blanket trait implementations that add methods
//! to Camera<P, T> when P implements the corresponding capability trait.

pub mod color;
pub mod exposure;
pub mod focus;
pub mod image_processing;
pub mod inquiry;
pub mod nd_filter;
pub mod pan_tilt;
pub mod power;
pub mod presets;
pub mod system;
pub mod tally;
pub mod white_balance;
pub mod zoom;

// GAT-based methods
pub mod color_gat;
pub mod exposure_gat;
pub mod focus_gat;
pub mod image_processing_gat;
pub mod inquiry_gat;
pub mod nd_filter_gat;
pub mod pan_tilt_gat;
pub mod power_gat;
pub mod presets_gat;
pub mod system_gat;
pub mod tally_gat;
pub mod white_balance_gat;
pub mod zoom_gat;

// Re-export all extension traits
pub use color::ColorMethodsExt;
pub use exposure::ExposureMethodsExt;
pub use focus::FocusMethodsExt;
pub use image_processing::ImageProcessingMethodsExt;
pub use inquiry::{InquiryMethodsExt, PanTiltInquiryMethodsExt};
pub use nd_filter::NDFilterMethodsExt;
pub use pan_tilt::PanTiltMethodsExt;
pub use power::PowerMethodsExt;
pub use presets::PresetMethodsExt;
pub use system::SystemMethodsExt;
pub use tally::TallyMethodsExt;
pub use white_balance::WhiteBalanceMethodsExt;
pub use zoom::ZoomMethodsExt;

// Re-export GAT extension traits
pub use color_gat::{ColorCoreExt, ColorAsyncExt, ColorBlockingExt};
pub use exposure_gat::{ExposureCoreExt, ExposureAsyncExt, ExposureBlockingExt};
pub use focus_gat::{FocusCoreExt, FocusAsyncExt, FocusBlockingExt};
pub use image_processing_gat::{ImageProcessingCoreExt, ImageProcessingAsyncExt, ImageProcessingBlockingExt};
pub use inquiry_gat::{InquiryCoreExt, InquiryAsyncExt, InquiryBlockingExt, PanTiltInquiryCoreExt, PanTiltInquiryAsyncExt, PanTiltInquiryBlockingExt};
pub use nd_filter_gat::{NDFilterCoreExt, NDFilterAsyncExt, NDFilterBlockingExt};
pub use pan_tilt_gat::{PanTiltCoreExt, PanTiltAsyncExt, PanTiltBlockingExt};
pub use power_gat::{PowerCoreExt, PowerAsyncExt, PowerBlockingExt};
pub use presets_gat::{PresetsCoreExt, PresetsAsyncExt, PresetsBlockingExt};
pub use system_gat::{SystemCoreExt, SystemAsyncExt, SystemBlockingExt};
pub use tally_gat::{TallyCoreExt, TallyAsyncExt, TallyBlockingExt};
pub use white_balance_gat::{WhiteBalanceCoreExt, WhiteBalanceAsyncExt, WhiteBalanceBlockingExt};
pub use zoom_gat::{ZoomCoreExt, ZoomAsyncExt, ZoomBlockingExt};
