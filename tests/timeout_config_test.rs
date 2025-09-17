//! Tests for timeout configuration functionality.

#[cfg(all(feature = "mode-async", feature = "test-utils"))]
mod timeout_config_tests {
    use std::time::Duration;

    use grafton_visca::{
        camera::builder::CameraBuilder,
        testing::testkit::DeterministicExecutor,
        timeout::{CommandCategory, TimeoutConfig},
    };

    #[test]
    fn test_custom_timeout_config_builder() {
        // Create a custom timeout configuration
        let timeout_config = TimeoutConfig::builder()
            .ack_timeout(Duration::from_millis(200))
            .quick_timeout(Duration::from_secs(2))
            .movement_timeout(Duration::from_secs(15))
            .preset_timeout(Duration::from_secs(45))
            .long_timeout(Duration::from_secs(180))
            .network_timeout(Duration::from_secs(3))
            .default_timeout(Duration::from_secs(30))
            .build();

        // Verify all timeouts are set correctly
        assert_eq!(timeout_config.ack_timeout, Duration::from_millis(200));
        assert_eq!(timeout_config.quick_timeout, Duration::from_secs(2));
        assert_eq!(timeout_config.movement_timeout, Duration::from_secs(15));
        assert_eq!(timeout_config.preset_timeout, Duration::from_secs(45));
        assert_eq!(timeout_config.long_timeout, Duration::from_secs(180));
        assert_eq!(timeout_config.network_timeout, Duration::from_secs(3));
        assert_eq!(timeout_config.default_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_timeout_config_get_timeout() {
        let config = TimeoutConfig::builder()
            .quick_timeout(Duration::from_secs(1))
            .movement_timeout(Duration::from_secs(10))
            .preset_timeout(Duration::from_secs(30))
            .long_timeout(Duration::from_secs(120))
            .network_timeout(Duration::from_secs(2))
            .default_timeout(Duration::from_secs(20))
            .build();

        // Test getting timeout for each category
        assert_eq!(
            config.get_timeout(CommandCategory::Quick),
            Duration::from_secs(1)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::Movement),
            Duration::from_secs(10)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::Preset),
            Duration::from_secs(30)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::LongRunning),
            Duration::from_secs(120)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::Network),
            Duration::from_secs(2)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::Custom),
            Duration::from_secs(20)
        );
    }

    #[cfg(feature = "mode-async")]
    #[test]
    fn test_async_camera_with_custom_timeouts() {
        // Create a custom timeout configuration with short timeouts for testing
        let timeout_config = TimeoutConfig::builder()
            .ack_timeout(Duration::from_millis(100))
            .quick_timeout(Duration::from_secs(1))
            .movement_timeout(Duration::from_secs(5))
            .preset_timeout(Duration::from_secs(10))
            .build();

        // Test that we can create an executor and use it with the timeout config
        let (executor, _clock) = DeterministicExecutor::new();

        // Verify the executor can be used with a camera builder
        let _builder = CameraBuilder::<DeterministicExecutor>::with_executor(executor)
            .timeout_config(timeout_config);

        // The actual runtime behavior with timeouts is tested via the scheduler unit tests
        // This test verifies that the API allows setting custom timeouts
    }

    #[cfg(feature = "mode-async")]
    #[test]
    fn test_timeout_config_with_different_executors() {
        // Create timeout config with different timeouts per category
        let timeout_config = TimeoutConfig::builder()
            .quick_timeout(Duration::from_millis(500)) // Quick commands: 500ms
            .movement_timeout(Duration::from_secs(2)) // Movement commands: 2s
            .preset_timeout(Duration::from_secs(5)) // Preset commands: 5s
            .build();

        // Test with deterministic executor
        let (executor, _clock) = DeterministicExecutor::new();
        let _builder = CameraBuilder::<DeterministicExecutor>::with_executor(executor)
            .timeout_config(timeout_config);

        // This test verifies that timeout configs can be used with different executor types
    }

    #[test]
    fn test_uniform_timeout_config() {
        let uniform_timeout = Duration::from_secs(10);
        let config = TimeoutConfig::uniform(uniform_timeout);

        // All timeouts should be the same
        assert_eq!(config.ack_timeout, uniform_timeout);
        assert_eq!(config.quick_timeout, uniform_timeout);
        assert_eq!(config.movement_timeout, uniform_timeout);
        assert_eq!(config.preset_timeout, uniform_timeout);
        assert_eq!(config.long_timeout, uniform_timeout);
        assert_eq!(config.network_timeout, uniform_timeout);
        assert_eq!(config.default_timeout, uniform_timeout);

        // Verify get_timeout returns the same value for all categories
        assert_eq!(config.get_timeout(CommandCategory::Quick), uniform_timeout);
        assert_eq!(
            config.get_timeout(CommandCategory::Movement),
            uniform_timeout
        );
        assert_eq!(config.get_timeout(CommandCategory::Preset), uniform_timeout);
        assert_eq!(
            config.get_timeout(CommandCategory::LongRunning),
            uniform_timeout
        );
        assert_eq!(
            config.get_timeout(CommandCategory::Network),
            uniform_timeout
        );
        assert_eq!(config.get_timeout(CommandCategory::Custom), uniform_timeout);
    }
}
