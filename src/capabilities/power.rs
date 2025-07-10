//! Power capability trait and associated types.

use std::time::Duration;

/// Trait for cameras that support power control.
///
/// This trait defines the constants and capabilities for power management
/// including power on/off timing and standby modes.
pub trait Power {
    /// Time required for camera to fully power on and be ready for commands.
    const POWER_ON_TIME: Duration;

    /// Whether camera supports standby mode (vs full power off).
    const SUPPORTS_STANDBY: bool;

    /// Time required to enter standby mode.
    const STANDBY_TIME: Duration = Duration::from_secs(1);

    /// Whether camera supports wake-on-network functionality.
    const SUPPORTS_WAKE_ON_LAN: bool = false;

    /// Whether camera retains settings when powered off.
    const RETAINS_SETTINGS_ON_POWER_OFF: bool = true;

    /// Whether camera returns to home position on power up.
    const HOME_ON_POWER_UP: bool = false;
}

/// Extension trait that adds power-related helper methods.
pub trait PowerExt: Power {
    /// Get the time to wait after power on before sending commands.
    fn power_on_delay(&self) -> Duration {
        Self::POWER_ON_TIME
    }

    /// Check if standby mode is available.
    fn can_standby(&self) -> bool {
        Self::SUPPORTS_STANDBY
    }

    /// Get the time to wait for standby mode.
    fn standby_delay(&self) -> Duration {
        Self::STANDBY_TIME
    }

    /// Check if wake-on-LAN is supported.
    fn supports_wake_on_lan(&self) -> bool {
        Self::SUPPORTS_WAKE_ON_LAN
    }

    /// Check if settings are retained on power off.
    fn retains_settings(&self) -> bool {
        Self::RETAINS_SETTINGS_ON_POWER_OFF
    }

    /// Check if camera homes on power up.
    fn homes_on_power_up(&self) -> bool {
        Self::HOME_ON_POWER_UP
    }
}

// Automatic implementation for all types that support power
impl<T: Power> PowerExt for T {}

/// Power states for VISCA cameras.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerState {
    /// Camera is fully powered on and operational.
    On,
    /// Camera is in standby/sleep mode (if supported).
    Standby,
    /// Camera is powered off.
    Off,
    /// Power state is unknown or transitioning.
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FastCamera;

    impl Power for FastCamera {
        const POWER_ON_TIME: Duration = Duration::from_secs(5);
        const SUPPORTS_STANDBY: bool = true;
        const SUPPORTS_WAKE_ON_LAN: bool = true;
    }

    struct SlowCamera;

    impl Power for SlowCamera {
        const POWER_ON_TIME: Duration = Duration::from_secs(30);
        const SUPPORTS_STANDBY: bool = false;
        const HOME_ON_POWER_UP: bool = true;
    }

    #[test]
    fn test_power_on_delay() {
        let fast = FastCamera;
        let slow = SlowCamera;

        assert_eq!(fast.power_on_delay(), Duration::from_secs(5));
        assert_eq!(slow.power_on_delay(), Duration::from_secs(30));
    }

    #[test]
    fn test_standby_support() {
        let fast = FastCamera;
        let slow = SlowCamera;

        assert!(fast.can_standby());
        assert!(!slow.can_standby());
    }

    #[test]
    fn test_wake_on_lan() {
        let fast = FastCamera;
        let slow = SlowCamera;

        assert!(fast.supports_wake_on_lan());
        assert!(!slow.supports_wake_on_lan());
    }

    #[test]
    fn test_home_on_power() {
        let fast = FastCamera;
        let slow = SlowCamera;

        assert!(!fast.homes_on_power_up());
        assert!(slow.homes_on_power_up());
    }
}
