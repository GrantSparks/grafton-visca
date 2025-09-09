#!/usr/bin/env rust-script
//! Test minimal timeout behavior

use grafton_visca::{
    runtime::handle::RuntimeHandle,
    testing::testkit::{
        deterministic_executor::{DeterministicExecutor, DeterministicExecutorExt},
        ScriptedTransport, Step,
    },
    timeout::TimeoutConfig,
    Error,
};
use std::time::Duration;

fn main() {
    println!("Starting minimal timeout test...");

    let (executor, _clock) = DeterministicExecutor::new();

    // Create transport with no response (will timeout)
    let transport: ScriptedTransport<DeterministicExecutor> =
        ScriptedTransport::new(vec![Step::OnSend {
            matches: None,
            responses: vec![], // No response - command will timeout
        }])
        .with_executor(executor.clone());

    // Custom timeout config with very short ACK timeout
    let timeout_config = TimeoutConfig::builder()
        .ack_timeout(Duration::from_millis(100))
        .tick_duration(Duration::from_millis(20))
        .build();

    let exec = executor.clone();
    executor.block_on_bg(async move {
        println!("Creating runtime handle...");

        // Create runtime handle directly
        let handle = RuntimeHandle::new_with_style_and_timeout(
            transport,
            exec.clone(),
            grafton_visca::prelude::advanced::ProtocolStyle::RawVisca,
            timeout_config,
        )
        .await
        .unwrap();

        println!("Runtime handle created, sending command...");

        // Send a simple command
        use grafton_visca::command::power::PowerInquiry;
        let result = handle
            .send_inquiry(&PowerInquiry, grafton_visca::camera_id::CameraId::CAMERA_1)
            .await;

        println!("Command result: {:?}", result);

        // Explicitly shutdown
        println!("Shutting down...");
        handle.shutdown().await;
        drop(handle);
        println!("Shutdown complete");
    });

    println!("Test complete!");
}
