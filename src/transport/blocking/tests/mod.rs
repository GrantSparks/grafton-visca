//! Regression tests for blocking UDP transport.

mod issue_843_udp_transient;
#[cfg(target_os = "linux")]
mod issue_869_linux_udp_send;
