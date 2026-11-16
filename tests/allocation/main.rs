//! Allocation suites share one counting allocator in a process isolated from
//! behavioral suites. This keeps allocator instrumentation out of their workload.

mod counting;
#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
#[path = "../common/profile_encodings.rs"]
mod profile_encodings;

mod dyn_api_integration_test;
mod issue_517_inquiry_encoding;
mod issue_548_allocation_baselines;
