//! State cache for write-only VISCA properties.
//!
//! This module provides optional library-level state caching for write-only VISCA properties,
//! enabling consumers to track and query values that have no corresponding VISCA inquiry command.
//!
//! # Problem
//!
//! Several VISCA properties have setter commands but no corresponding inquiry:
//!
//! | Property | Set Command | Inquiry |
//! |----------|-------------|---------|
//! | Auto Slow Shutter | `AutoSlowShutterOn`/`Off` | None |
//! | Spotlight | `SpotlightOn`/`Off` | None |
//! | Pan/Tilt Limits | `pan_tilt_limit_set`/`clear` | None |
//!
//! This breaks the "resource model" pattern where properties should be round-trippable.
//!
//! # Solution
//!
//! The `StateCache` provides transparent tracking of write-only property values. When a setter
//! command succeeds, the cache is automatically updated. The cached values can then be queried
//! via accessor methods.
//!
//! # Thread Safety
//!
//! The cache uses interior mutability with appropriate synchronization primitives:
//! - Async mode: `Arc<std::sync::Mutex<StateCacheInner>>` for Send+Sync
//! - Blocking mode: `RefCell<StateCacheInner>` for single-threaded access
//!
//! # Example
//!
//! ```ignore
//! let camera = Connect::open_tcp_async::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
//!
//! // Set write-only property
//! camera.enable_auto_slow_shutter().await?;
//!
//! // Query cached value
//! assert_eq!(camera.state_cache().auto_slow_shutter(), Some(true));
//! ```

use crate::{
    command::pan_tilt::PanTiltLimitCorner,
    types::{PanPosition, TiltPosition},
};

#[cfg(feature = "mode-async")]
use std::sync::{Arc, Mutex};

#[cfg(not(feature = "mode-async"))]
use std::cell::RefCell;

/// Limits for pan/tilt movement.
///
/// Represents a rectangular bounding box for allowed pan/tilt positions,
/// defined by two diagonal corners: upper-right and lower-left.
///
/// These two corners fully specify the movement rectangle. Other corners
/// (upper-left and lower-right) are implicitly derived from these two:
/// - Upper-left: (down_left.pan, up_right.tilt)
/// - Lower-right: (up_right.pan, down_left.tilt)
///
/// This design mirrors the VISCA protocol, which only supports setting
/// two diagonal corners, not all four independently.
///
/// # Variants
///
/// The enum-based design makes invalid states unrepresentable:
/// - `Unset`: No limits configured
/// - `PartialUpRight`: Only upper-right corner is set
/// - `PartialDownLeft`: Only lower-left corner is set
/// - `Complete`: Both corners are set, forming a valid bounding box
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum PanTiltLimits {
    /// No limits configured.
    #[default]
    Unset,
    /// Only upper-right corner is set (maximum pan, maximum tilt).
    PartialUpRight {
        /// Upper-right corner limit.
        up_right: (PanPosition, TiltPosition),
    },
    /// Only lower-left corner is set (minimum pan, minimum tilt).
    PartialDownLeft {
        /// Lower-left corner limit.
        down_left: (PanPosition, TiltPosition),
    },
    /// Complete bounding box with both corners defined.
    Complete {
        /// Upper-right corner limit (maximum pan, maximum tilt).
        up_right: (PanPosition, TiltPosition),
        /// Lower-left corner limit (minimum pan, minimum tilt).
        down_left: (PanPosition, TiltPosition),
    },
}

impl PanTiltLimits {
    /// Create a new empty `PanTiltLimits` with no corners set.
    #[must_use]
    pub const fn new() -> Self {
        Self::Unset
    }

    /// Check if any limits are set.
    #[must_use]
    pub const fn is_set(&self) -> bool {
        !matches!(self, Self::Unset)
    }

    /// Check if both limits are set, forming a complete bounding box.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete { .. })
    }

    /// Get the upper-right corner if set.
    #[must_use]
    pub const fn up_right(&self) -> Option<(PanPosition, TiltPosition)> {
        match self {
            Self::Unset | Self::PartialDownLeft { .. } => None,
            Self::PartialUpRight { up_right } | Self::Complete { up_right, .. } => Some(*up_right),
        }
    }

    /// Get the lower-left corner if set.
    #[must_use]
    pub const fn down_left(&self) -> Option<(PanPosition, TiltPosition)> {
        match self {
            Self::Unset | Self::PartialUpRight { .. } => None,
            Self::PartialDownLeft { down_left } | Self::Complete { down_left, .. } => {
                Some(*down_left)
            }
        }
    }

    /// Set the upper-right corner, preserving any existing lower-left corner.
    #[must_use]
    pub const fn with_up_right(self, up_right: (PanPosition, TiltPosition)) -> Self {
        match self {
            Self::Unset | Self::PartialUpRight { .. } => Self::PartialUpRight { up_right },
            Self::PartialDownLeft { down_left } | Self::Complete { down_left, .. } => {
                Self::Complete {
                    up_right,
                    down_left,
                }
            }
        }
    }

    /// Set the lower-left corner, preserving any existing upper-right corner.
    #[must_use]
    pub const fn with_down_left(self, down_left: (PanPosition, TiltPosition)) -> Self {
        match self {
            Self::Unset | Self::PartialDownLeft { .. } => Self::PartialDownLeft { down_left },
            Self::PartialUpRight { up_right } | Self::Complete { up_right, .. } => Self::Complete {
                up_right,
                down_left,
            },
        }
    }

    /// Clear the upper-right corner, preserving any existing lower-left corner.
    #[must_use]
    pub const fn without_up_right(self) -> Self {
        match self {
            Self::Unset | Self::PartialDownLeft { .. } => self,
            Self::PartialUpRight { .. } => Self::Unset,
            Self::Complete { down_left, .. } => Self::PartialDownLeft { down_left },
        }
    }

    /// Clear the lower-left corner, preserving any existing upper-right corner.
    #[must_use]
    pub const fn without_down_left(self) -> Self {
        match self {
            Self::Unset | Self::PartialUpRight { .. } => self,
            Self::PartialDownLeft { .. } => Self::Unset,
            Self::Complete { up_right, .. } => Self::PartialUpRight { up_right },
        }
    }
}

/// Cached flip state for image orientation.
///
/// Tracks both horizontal (mirror) and vertical (upside-down) flip settings.
/// Used for PTZOptics cameras that require the combined flip command (0xA4).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CachedFlipState {
    /// Whether horizontal flip (mirror) is enabled.
    pub horizontal: bool,
    /// Whether vertical flip (upside-down) is enabled.
    pub vertical: bool,
}

/// Inner state storage for the cache.
///
/// This struct holds the actual cached values and is wrapped in appropriate
/// synchronization primitives based on the build mode.
#[derive(Debug, Default)]
struct StateCacheInner {
    /// Cached auto slow shutter state.
    /// `None` if never set through this camera instance.
    auto_slow_shutter: Option<bool>,

    /// Cached spotlight state (Sony models).
    /// `None` if never set through this camera instance.
    spotlight: Option<bool>,

    /// Cached pan/tilt limits.
    pan_tilt_limits: PanTiltLimits,

    /// Cached flip state (for PTZOptics combined flip command).
    /// `None` if never queried or set through this camera instance.
    flip_state: Option<CachedFlipState>,
}

impl StateCacheInner {
    fn new() -> Self {
        Self::default()
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Cached state for write-only properties.
///
/// This cache tracks values set via commands that have no VISCA inquiry.
/// Values are updated automatically when commands succeed, and can be
/// queried via accessor methods.
///
/// # Thread Safety
///
/// In async mode, the cache is `Send + Sync` using `Arc<Mutex<>>`.
/// In blocking mode, the cache uses `RefCell` for interior mutability.
///
/// # Persistence
///
/// The cache is in-memory only and resets on camera disconnect or explicit clear.
///
/// # Initialization
///
/// Values start as `None` and become `Some` after the first set command succeeds.
#[cfg(feature = "mode-async")]
#[derive(Debug, Clone)]
pub struct StateCache {
    inner: Arc<Mutex<StateCacheInner>>,
}

/// Cached state for write-only properties (blocking mode).
#[cfg(not(feature = "mode-async"))]
#[derive(Debug)]
pub struct StateCache {
    inner: RefCell<StateCacheInner>,
}

#[cfg(feature = "mode-async")]
impl Default for StateCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "mode-async"))]
impl Default for StateCache {
    fn default() -> Self {
        Self::new()
    }
}

impl StateCache {
    /// Create a new empty state cache.
    #[must_use]
    pub fn new() -> Self {
        #[cfg(feature = "mode-async")]
        {
            Self {
                inner: Arc::new(Mutex::new(StateCacheInner::new())),
            }
        }
        #[cfg(not(feature = "mode-async"))]
        {
            Self {
                inner: RefCell::new(StateCacheInner::new()),
            }
        }
    }

    /// Query cached auto slow shutter state.
    ///
    /// Returns `None` if never set through this camera instance.
    #[must_use]
    pub fn auto_slow_shutter(&self) -> Option<bool> {
        #[cfg(feature = "mode-async")]
        {
            self.inner
                .lock()
                .map(|guard| guard.auto_slow_shutter)
                .unwrap_or(None)
        }
        #[cfg(not(feature = "mode-async"))]
        {
            self.inner.borrow().auto_slow_shutter
        }
    }

    /// Query cached spotlight state.
    ///
    /// Returns `None` if never set through this camera instance.
    #[must_use]
    pub fn spotlight(&self) -> Option<bool> {
        #[cfg(feature = "mode-async")]
        {
            self.inner
                .lock()
                .map(|guard| guard.spotlight)
                .unwrap_or(None)
        }
        #[cfg(not(feature = "mode-async"))]
        {
            self.inner.borrow().spotlight
        }
    }

    /// Query cached pan/tilt limits.
    ///
    /// Returns the current limit settings. Individual corners may be `None`
    /// if they haven't been set.
    #[must_use]
    pub fn pan_tilt_limits(&self) -> PanTiltLimits {
        #[cfg(feature = "mode-async")]
        {
            self.inner
                .lock()
                .map(|guard| guard.pan_tilt_limits)
                .unwrap_or_default()
        }
        #[cfg(not(feature = "mode-async"))]
        {
            self.inner.borrow().pan_tilt_limits
        }
    }

    /// Query cached flip state.
    ///
    /// Returns `None` if never queried or set through this camera instance.
    /// Used by PTZOptics cameras that need to track flip state for the
    /// combined flip command (0xA4).
    #[must_use]
    pub fn flip_state(&self) -> Option<CachedFlipState> {
        #[cfg(feature = "mode-async")]
        {
            self.inner
                .lock()
                .map(|guard| guard.flip_state)
                .unwrap_or(None)
        }
        #[cfg(not(feature = "mode-async"))]
        {
            self.inner.borrow().flip_state
        }
    }

    /// Clear all cached state.
    ///
    /// Call this after a camera reset or reconnection to ensure
    /// the cache reflects the actual camera state (unknown).
    pub fn clear(&self) {
        #[cfg(feature = "mode-async")]
        {
            if let Ok(mut guard) = self.inner.lock() {
                guard.clear();
            }
        }
        #[cfg(not(feature = "mode-async"))]
        {
            self.inner.borrow_mut().clear();
        }
    }

    // Internal mutation methods - called by control traits after successful commands

    /// Update the cached auto slow shutter state.
    ///
    /// This is called internally after a successful `enable_auto_slow_shutter()`
    /// or `disable_auto_slow_shutter()` command.
    pub(crate) fn set_auto_slow_shutter(&self, enabled: bool) {
        #[cfg(feature = "mode-async")]
        {
            if let Ok(mut guard) = self.inner.lock() {
                guard.auto_slow_shutter = Some(enabled);
            }
        }
        #[cfg(not(feature = "mode-async"))]
        {
            self.inner.borrow_mut().auto_slow_shutter = Some(enabled);
        }
    }

    /// Update the cached spotlight state.
    ///
    /// This is called internally after a successful `enable_spotlight()`
    /// or `disable_spotlight()` command.
    pub(crate) fn set_spotlight(&self, enabled: bool) {
        #[cfg(feature = "mode-async")]
        {
            if let Ok(mut guard) = self.inner.lock() {
                guard.spotlight = Some(enabled);
            }
        }
        #[cfg(not(feature = "mode-async"))]
        {
            self.inner.borrow_mut().spotlight = Some(enabled);
        }
    }

    /// Update the cached flip state.
    ///
    /// This is called internally after a successful flip command or inquiry.
    /// PTZOptics cameras use this to track state for the combined flip command.
    pub(crate) fn set_flip_state(&self, horizontal: bool, vertical: bool) {
        #[cfg(feature = "mode-async")]
        {
            if let Ok(mut guard) = self.inner.lock() {
                guard.flip_state = Some(CachedFlipState {
                    horizontal,
                    vertical,
                });
            }
        }
        #[cfg(not(feature = "mode-async"))]
        {
            self.inner.borrow_mut().flip_state = Some(CachedFlipState {
                horizontal,
                vertical,
            });
        }
    }

    /// Update a cached pan/tilt limit corner.
    ///
    /// This is called internally after a successful `pan_tilt_limit_set()` command.
    /// Both corner variants (`UpRight` and `DownLeft`) update their corresponding
    /// fields in the cache, ensuring complete coverage of all representable states.
    pub(crate) fn set_pan_tilt_limit(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) {
        #[cfg(feature = "mode-async")]
        {
            if let Ok(mut guard) = self.inner.lock() {
                guard.pan_tilt_limits = match corner {
                    PanTiltLimitCorner::UpRight => guard.pan_tilt_limits.with_up_right((pan, tilt)),
                    PanTiltLimitCorner::DownLeft => {
                        guard.pan_tilt_limits.with_down_left((pan, tilt))
                    }
                };
            }
        }
        #[cfg(not(feature = "mode-async"))]
        {
            let mut inner = self.inner.borrow_mut();
            inner.pan_tilt_limits = match corner {
                PanTiltLimitCorner::UpRight => inner.pan_tilt_limits.with_up_right((pan, tilt)),
                PanTiltLimitCorner::DownLeft => inner.pan_tilt_limits.with_down_left((pan, tilt)),
            };
        }
    }

    /// Clear a cached pan/tilt limit corner.
    ///
    /// This is called internally after a successful `pan_tilt_limit_clear()` command.
    /// Both corner variants (`UpRight` and `DownLeft`) clear their corresponding
    /// fields in the cache, ensuring complete coverage of all representable states.
    pub(crate) fn clear_pan_tilt_limit(&self, corner: PanTiltLimitCorner) {
        #[cfg(feature = "mode-async")]
        {
            if let Ok(mut guard) = self.inner.lock() {
                guard.pan_tilt_limits = match corner {
                    PanTiltLimitCorner::UpRight => guard.pan_tilt_limits.without_up_right(),
                    PanTiltLimitCorner::DownLeft => guard.pan_tilt_limits.without_down_left(),
                };
            }
        }
        #[cfg(not(feature = "mode-async"))]
        {
            let mut inner = self.inner.borrow_mut();
            inner.pan_tilt_limits = match corner {
                PanTiltLimitCorner::UpRight => inner.pan_tilt_limits.without_up_right(),
                PanTiltLimitCorner::DownLeft => inner.pan_tilt_limits.without_down_left(),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_cache_new_returns_none_values() {
        let cache = StateCache::new();
        assert_eq!(cache.auto_slow_shutter(), None);
        assert_eq!(cache.spotlight(), None);
        assert!(!cache.pan_tilt_limits().is_set());
        assert_eq!(cache.flip_state(), None);
    }

    #[test]
    fn test_auto_slow_shutter_caching() {
        let cache = StateCache::new();

        cache.set_auto_slow_shutter(true);
        assert_eq!(cache.auto_slow_shutter(), Some(true));

        cache.set_auto_slow_shutter(false);
        assert_eq!(cache.auto_slow_shutter(), Some(false));
    }

    #[test]
    fn test_spotlight_caching() {
        let cache = StateCache::new();

        cache.set_spotlight(true);
        assert_eq!(cache.spotlight(), Some(true));

        cache.set_spotlight(false);
        assert_eq!(cache.spotlight(), Some(false));
    }

    #[test]
    fn test_flip_state_caching() {
        let cache = StateCache::new();

        // Initially None
        assert_eq!(cache.flip_state(), None);

        // Set both flips off
        cache.set_flip_state(false, false);
        assert_eq!(
            cache.flip_state(),
            Some(CachedFlipState {
                horizontal: false,
                vertical: false
            })
        );

        // Set horizontal flip on
        cache.set_flip_state(true, false);
        assert_eq!(
            cache.flip_state(),
            Some(CachedFlipState {
                horizontal: true,
                vertical: false
            })
        );

        // Set vertical flip on
        cache.set_flip_state(false, true);
        assert_eq!(
            cache.flip_state(),
            Some(CachedFlipState {
                horizontal: false,
                vertical: true
            })
        );

        // Set both flips on
        cache.set_flip_state(true, true);
        assert_eq!(
            cache.flip_state(),
            Some(CachedFlipState {
                horizontal: true,
                vertical: true
            })
        );
    }

    #[test]
    fn test_pan_tilt_limit_caching() {
        let cache = StateCache::new();

        let pan = PanPosition::CENTER;
        let tilt = TiltPosition::CENTER;

        cache.set_pan_tilt_limit(PanTiltLimitCorner::UpRight, pan, tilt);
        let limits = cache.pan_tilt_limits();
        assert_eq!(limits.up_right(), Some((pan, tilt)));
        assert_eq!(limits.down_left(), None);

        cache.set_pan_tilt_limit(PanTiltLimitCorner::DownLeft, pan, tilt);
        let limits = cache.pan_tilt_limits();
        assert!(limits.is_complete());
    }

    #[test]
    fn test_clear_resets_all_values() {
        let cache = StateCache::new();

        cache.set_auto_slow_shutter(true);
        cache.set_spotlight(true);
        cache.set_pan_tilt_limit(
            PanTiltLimitCorner::UpRight,
            PanPosition::CENTER,
            TiltPosition::CENTER,
        );
        cache.set_flip_state(true, true);

        cache.clear();

        assert_eq!(cache.auto_slow_shutter(), None);
        assert_eq!(cache.spotlight(), None);
        assert!(!cache.pan_tilt_limits().is_set());
        assert_eq!(cache.flip_state(), None);
    }

    #[test]
    fn test_pan_tilt_limits_is_complete() {
        let limits = PanTiltLimits::new();
        assert!(!limits.is_complete());
        assert!(!limits.is_set());

        let limits = PanTiltLimits::PartialUpRight {
            up_right: (PanPosition::CENTER, TiltPosition::CENTER),
        };
        assert!(!limits.is_complete());
        assert!(limits.is_set());

        let limits = PanTiltLimits::Complete {
            up_right: (PanPosition::CENTER, TiltPosition::CENTER),
            down_left: (PanPosition::CENTER, TiltPosition::CENTER),
        };
        assert!(limits.is_complete());
        assert!(limits.is_set());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_pan_tilt_limit_caching_both_corners() {
        let cache = StateCache::new();

        // Initially no limits are set
        let limits = cache.pan_tilt_limits();
        assert!(!limits.is_set());
        assert!(!limits.is_complete());

        // Set UpRight corner (upper-right has max pan/max tilt)
        // PanPosition valid range: -2448 to 2448
        // TiltPosition valid range: -432 to 1296
        let ur_pan = PanPosition::new(1000).unwrap();
        let ur_tilt = TiltPosition::new(500).unwrap();
        cache.set_pan_tilt_limit(PanTiltLimitCorner::UpRight, ur_pan, ur_tilt);

        let limits = cache.pan_tilt_limits();
        assert!(limits.is_set());
        assert!(!limits.is_complete());
        assert_eq!(limits.up_right(), Some((ur_pan, ur_tilt)));
        assert_eq!(limits.down_left(), None);

        // Set DownLeft corner - should complete the bounding box
        // (lower-left has min pan/min tilt)
        let dl_pan = PanPosition::new(-1000).unwrap();
        let dl_tilt = TiltPosition::new(-200).unwrap();
        cache.set_pan_tilt_limit(PanTiltLimitCorner::DownLeft, dl_pan, dl_tilt);

        let limits = cache.pan_tilt_limits();
        assert!(limits.is_set());
        assert!(limits.is_complete());
        assert_eq!(limits.up_right(), Some((ur_pan, ur_tilt)));
        assert_eq!(limits.down_left(), Some((dl_pan, dl_tilt)));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_pan_tilt_limit_clear_individual_corners() {
        let cache = StateCache::new();

        // Set both corners
        // PanPosition valid range: -2448 to 2448
        // TiltPosition valid range: -432 to 1296
        let ur_pan = PanPosition::new(1000).unwrap();
        let ur_tilt = TiltPosition::new(500).unwrap();
        let dl_pan = PanPosition::new(-1000).unwrap();
        let dl_tilt = TiltPosition::new(-200).unwrap();
        cache.set_pan_tilt_limit(PanTiltLimitCorner::UpRight, ur_pan, ur_tilt);
        cache.set_pan_tilt_limit(PanTiltLimitCorner::DownLeft, dl_pan, dl_tilt);

        let limits = cache.pan_tilt_limits();
        assert!(limits.is_complete());

        // Clear UpRight corner
        cache.clear_pan_tilt_limit(PanTiltLimitCorner::UpRight);

        let limits = cache.pan_tilt_limits();
        assert!(limits.is_set()); // DownLeft still set
        assert!(!limits.is_complete()); // Not complete without UpRight
        assert_eq!(limits.up_right(), None);
        assert_eq!(limits.down_left(), Some((dl_pan, dl_tilt)));

        // Clear DownLeft corner
        cache.clear_pan_tilt_limit(PanTiltLimitCorner::DownLeft);

        let limits = cache.pan_tilt_limits();
        assert!(!limits.is_set());
        assert!(!limits.is_complete());
        assert_eq!(limits.up_right(), None);
        assert_eq!(limits.down_left(), None);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_pan_tilt_limit_overwrite() {
        let cache = StateCache::new();

        // Set UpRight with initial values
        let pan1 = PanPosition::new(100).unwrap();
        let tilt1 = TiltPosition::new(50).unwrap();
        cache.set_pan_tilt_limit(PanTiltLimitCorner::UpRight, pan1, tilt1);

        assert_eq!(cache.pan_tilt_limits().up_right(), Some((pan1, tilt1)));

        // Overwrite with new values
        let pan2 = PanPosition::new(200).unwrap();
        let tilt2 = TiltPosition::new(100).unwrap();
        cache.set_pan_tilt_limit(PanTiltLimitCorner::UpRight, pan2, tilt2);

        // Should have the new values
        assert_eq!(cache.pan_tilt_limits().up_right(), Some((pan2, tilt2)));
    }
}
