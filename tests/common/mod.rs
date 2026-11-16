//! Helpers shared by the integration and allocation test modules.

#[cfg(all(
    feature = "async",
    any(
        feature = "test-utils",
        feature = "runtime-tokio",
        feature = "runtime-smol"
    )
))]
pub(crate) mod counting_executor;
pub(crate) mod profile_encodings;
