//! Method implementations for cameras based on their capabilities.
//!
//! Each module provides extension traits that add methods to
//! CameraCore, CameraAsync, and CameraBlocking based on the camera's capabilities.

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

// Re-export extension traits
pub use color::{ColorAsyncExt, ColorBlockingExt, ColorCoreExt};
pub use exposure::{ExposureAsyncExt, ExposureBlockingExt, ExposureCoreExt};
pub use focus::{FocusAsyncExt, FocusBlockingExt, FocusCoreExt};
pub use image_processing::{
    ImageProcessingAsyncExt, ImageProcessingBlockingExt, ImageProcessingCoreExt,
};
pub use inquiry::{
    InquiryAsyncExt, InquiryBlockingExt, InquiryCoreExt, PanTiltInquiryAsyncExt,
    PanTiltInquiryBlockingExt, PanTiltInquiryCoreExt,
};
pub use nd_filter::{NDFilterAsyncExt, NDFilterBlockingExt, NDFilterCoreExt};
pub use pan_tilt::{PanTiltAsyncExt, PanTiltBlockingExt, PanTiltCoreExt};
pub use power::{PowerAsyncExt, PowerBlockingExt, PowerCoreExt};
pub use presets::{PresetsAsyncExt, PresetsBlockingExt, PresetsCoreExt};
pub use system::{SystemAsyncExt, SystemBlockingExt, SystemCoreExt};
pub use tally::{TallyAsyncExt, TallyBlockingExt, TallyCoreExt};
pub use white_balance::{WhiteBalanceAsyncExt, WhiteBalanceBlockingExt, WhiteBalanceCoreExt};
pub use zoom::{ZoomAsyncExt, ZoomBlockingExt, ZoomCoreExt};
