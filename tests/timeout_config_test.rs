//! Tests for timeout configuration functionality.

#[cfg(all(feature = "mode-async", feature = "test-utils"))]
mod timeout_config_tests {
    use std::time::Duration;

    #[cfg(feature = "runtime-tokio")]
    use grafton_visca::camera::CameraBuilder;
    use grafton_visca::timeout::{CommandCategory, TimeoutConfig};

    #[test]
    fn test_custom_timeout_config_builder() {
        let timeout_config = TimeoutConfig::builder()
            .ack_timeout(Duration::from_millis(200))
            .quick_timeout(Duration::from_secs(2))
            .movement_timeout(Duration::from_secs(15))
            .preset_timeout(Duration::from_secs(45))
            .long_timeout(Duration::from_secs(180))
            .network_timeout(Duration::from_secs(3))
            .default_timeout(Duration::from_secs(30))
            .build();

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

    /// A partially specified builder must leave the untouched categories at their
    /// documented defaults rather than zeroing them.
    #[test]
    fn test_partial_builder_keeps_defaults_for_unset_categories() {
        let defaults = TimeoutConfig::default();
        let config = TimeoutConfig::builder()
            .movement_timeout(Duration::from_secs(7))
            .build();

        assert_eq!(config.movement_timeout, Duration::from_secs(7));
        assert_eq!(config.quick_timeout, defaults.quick_timeout);
        assert_eq!(config.preset_timeout, defaults.preset_timeout);
        assert_eq!(config.long_timeout, defaults.long_timeout);
        assert_eq!(config.network_timeout, defaults.network_timeout);
        assert_eq!(config.ack_timeout, defaults.ack_timeout);
        assert_eq!(config.default_timeout, defaults.default_timeout);
        assert_eq!(
            config.get_timeout(CommandCategory::Movement),
            Duration::from_secs(7),
            "the overridden category must be the one the runtime reads back"
        );
    }

    /// `CameraBuilder::timeout_config` must be more than a setter: the value has
    /// to reach the runtime that enforces deadlines. A 150ms movement timeout on
    /// a camera that never sends a Completion must fail in well under the 30s
    /// default.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn test_builder_timeout_config_is_enforced_by_the_runtime() {
        use std::sync::Arc;
        use std::time::Instant;

        use grafton_visca::{
            camera::profiles::GenericVisca,
            testing::testkit::{helpers, ScriptedTransport, Step},
            TokioExecutor, ZoomControl,
        };

        let executor = Arc::new(TokioExecutor::from_current().expect("tokio runtime"));
        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: None,
                responses: vec![helpers::ack(1)], // ACK only: the completion never arrives.
            }])
            .with_executor(executor.clone());

        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor)
            .timeout_config(
                TimeoutConfig::builder()
                    .ack_timeout(Duration::from_millis(100))
                    .movement_timeout(Duration::from_millis(150))
                    .build(),
            )
            .open_async::<GenericVisca, _>(transport)
            .await
            .expect("camera should open");

        let started = Instant::now();
        let result = tokio::time::timeout(Duration::from_secs(5), camera.zoom_tele(None))
            .await
            .expect("the custom movement timeout must fire long before the default");
        let elapsed = started.elapsed();

        assert!(
            result.is_err(),
            "a movement command with no completion must fail, got {result:?}"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "the builder's 150ms movement timeout must be used, not the 30s default (took {elapsed:?})"
        );
    }

    #[test]
    fn test_uniform_timeout_config() {
        let uniform_timeout = Duration::from_secs(10);
        let config = TimeoutConfig::uniform(uniform_timeout);

        assert_eq!(config.ack_timeout, uniform_timeout);
        assert_eq!(config.quick_timeout, uniform_timeout);
        assert_eq!(config.movement_timeout, uniform_timeout);
        assert_eq!(config.preset_timeout, uniform_timeout);
        assert_eq!(config.long_timeout, uniform_timeout);
        assert_eq!(config.network_timeout, uniform_timeout);
        assert_eq!(config.default_timeout, uniform_timeout);

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
