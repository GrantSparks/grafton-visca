//! Method implementations for cameras based on their capabilities.
//!
//! Each module contains legacy trait definitions that are no longer used.
//! Methods are now implemented directly on the Camera struct with mode markers.
//!
//! ## Unified Control Traits
//!
//! The `unified_*` modules contain the new Mode-parametrized control traits
//! that work with both blocking and async modes through the Mode trait system.
//! These provide a single API surface that adapts to the execution mode.

pub mod color;
pub mod exposure;
pub mod focus;
pub mod image_processing;
pub mod inquiry;
pub mod menu;
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

// Unified control traits using Mode trait system
pub mod unified_focus;
pub mod unified_pan_tilt;
pub mod unified_power;
pub mod unified_presets;
pub mod unified_zoom;

// Re-export unified traits for easier access
pub use unified_focus::UnifiedFocusControl;
pub use unified_pan_tilt::UnifiedPanTiltControl;
pub use unified_power::UnifiedPowerControl;
pub use unified_presets::UnifiedPresetsControl;
pub use unified_zoom::UnifiedZoomControl;

// Note: Legacy trait exports have been removed as methods are now implemented
// directly on Camera<AsyncMode, P, T> and Camera<BlockingMode, P, T>
// The unified traits should be used for new implementations.
