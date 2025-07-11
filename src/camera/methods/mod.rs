//! Method implementations for cameras based on their capabilities.
//!
//! Each module provides extension traits that add methods to
//! CameraCore, CameraAsync, and CameraBlocking based on the camera's capabilities.

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