// Shared by the feature-gated testkit exports and feature-independent docs.
macro_rules! testkit_utility_exports {
    ($emit:ident) => {
        test_utility_exports! { @$emit;
            all(feature = "test-utils", feature = "async") => scripted_transport {
                Immediate, ScriptedTransport,
            };

            all(feature = "test-utils", feature = "blocking") => scripted_transport {
                ScriptedBlockingTransport,
            };

            all(feature = "test-utils", any(feature = "blocking", feature = "async")) => crate::testing::manual_clock {
                ClockJoinHandle, ClockParticipant, ManualClock,
            };

            all(feature = "test-utils", feature = "async") => crate::testing::manual_clock {
                LocalExecutor, ManualClockExecutor, ManualClockTimer,
            };

            feature = "test-utils" => scripted_transport {
                helpers, DynamicResponseFn, ScriptedTransportIdentity, Step,
            };
        }
    };
}
