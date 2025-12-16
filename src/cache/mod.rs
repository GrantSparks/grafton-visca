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
/// Represents a rectangular bounding box for allowed pan/tilt positions.
/// The limits are defined by two corners: upper-right and down-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct PanTiltLimits {
    /// Upper-right corner limit (maximum pan, maximum tilt).
    pub up_right: Option<(PanPosition, TiltPosition)>,
    /// Down-left corner limit (minimum pan, minimum tilt).
    pub down_left: Option<(PanPosition, TiltPosition)>,
}

impl Default for PanTiltLimits {
    fn default() -> Self {
        Self::new()
    }
}

impl PanTiltLimits {
    /// Create a new empty `PanTiltLimits` with no corners set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            up_right: None,
            down_left: None,
        }
    }

    /// Check if any limits are set.
    #[must_use]
    pub const fn is_set(&self) -> bool {
        self.up_right.is_some() || self.down_left.is_some()
    }

    /// Check if both limits are set, forming a complete bounding box.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.up_right.is_some() && self.down_left.is_some()
    }
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

    /// Update a cached pan/tilt limit corner.
    ///
    /// This is called internally after a successful `pan_tilt_limit_set()` command.
    pub(crate) fn set_pan_tilt_limit(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) {
        #[cfg(feature = "mode-async")]
        {
            if let Ok(mut guard) = self.inner.lock() {
                match corner {
                    PanTiltLimitCorner::UpRight | PanTiltLimitCorner::UpLeft => {
                        // Both UpRight and UpLeft affect the up_right limit
                        // (VISCA uses UpRight for the upper corner)
                        if matches!(corner, PanTiltLimitCorner::UpRight) {
                            guard.pan_tilt_limits.up_right = Some((pan, tilt));
                        }
                    }
                    PanTiltLimitCorner::DownLeft | PanTiltLimitCorner::DownRight => {
                        // Both DownLeft and DownRight affect the down_left limit
                        // (VISCA uses DownLeft for the lower corner)
                        if matches!(corner, PanTiltLimitCorner::DownLeft) {
                            guard.pan_tilt_limits.down_left = Some((pan, tilt));
                        }
                    }
                }
            }
        }
        #[cfg(not(feature = "mode-async"))]
        {
            let mut inner = self.inner.borrow_mut();
            match corner {
                PanTiltLimitCorner::UpRight | PanTiltLimitCorner::UpLeft => {
                    if matches!(corner, PanTiltLimitCorner::UpRight) {
                        inner.pan_tilt_limits.up_right = Some((pan, tilt));
                    }
                }
                PanTiltLimitCorner::DownLeft | PanTiltLimitCorner::DownRight => {
                    if matches!(corner, PanTiltLimitCorner::DownLeft) {
                        inner.pan_tilt_limits.down_left = Some((pan, tilt));
                    }
                }
            }
        }
    }

    /// Clear a cached pan/tilt limit corner.
    ///
    /// This is called internally after a successful `pan_tilt_limit_clear()` command.
    pub(crate) fn clear_pan_tilt_limit(&self, corner: PanTiltLimitCorner) {
        #[cfg(feature = "mode-async")]
        {
            if let Ok(mut guard) = self.inner.lock() {
                match corner {
                    PanTiltLimitCorner::UpRight | PanTiltLimitCorner::UpLeft => {
                        if matches!(corner, PanTiltLimitCorner::UpRight) {
                            guard.pan_tilt_limits.up_right = None;
                        }
                    }
                    PanTiltLimitCorner::DownLeft | PanTiltLimitCorner::DownRight => {
                        if matches!(corner, PanTiltLimitCorner::DownLeft) {
                            guard.pan_tilt_limits.down_left = None;
                        }
                    }
                }
            }
        }
        #[cfg(not(feature = "mode-async"))]
        {
            let mut inner = self.inner.borrow_mut();
            match corner {
                PanTiltLimitCorner::UpRight | PanTiltLimitCorner::UpLeft => {
                    if matches!(corner, PanTiltLimitCorner::UpRight) {
                        inner.pan_tilt_limits.up_right = None;
                    }
                }
                PanTiltLimitCorner::DownLeft | PanTiltLimitCorner::DownRight => {
                    if matches!(corner, PanTiltLimitCorner::DownLeft) {
                        inner.pan_tilt_limits.down_left = None;
                    }
                }
            }
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
    fn test_pan_tilt_limit_caching() {
        let cache = StateCache::new();

        let pan = PanPosition::CENTER;
        let tilt = TiltPosition::CENTER;

        cache.set_pan_tilt_limit(PanTiltLimitCorner::UpRight, pan, tilt);
        let limits = cache.pan_tilt_limits();
        assert_eq!(limits.up_right, Some((pan, tilt)));
        assert_eq!(limits.down_left, None);

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

        cache.clear();

        assert_eq!(cache.auto_slow_shutter(), None);
        assert_eq!(cache.spotlight(), None);
        assert!(!cache.pan_tilt_limits().is_set());
    }

    #[test]
    fn test_pan_tilt_limits_is_complete() {
        let limits = PanTiltLimits::new();
        assert!(!limits.is_complete());
        assert!(!limits.is_set());

        let limits = PanTiltLimits {
            up_right: Some((PanPosition::CENTER, TiltPosition::CENTER)),
            down_left: None,
        };
        assert!(!limits.is_complete());
        assert!(limits.is_set());

        let limits = PanTiltLimits {
            up_right: Some((PanPosition::CENTER, TiltPosition::CENTER)),
            down_left: Some((PanPosition::CENTER, TiltPosition::CENTER)),
        };
        assert!(limits.is_complete());
        assert!(limits.is_set());
    }
}
