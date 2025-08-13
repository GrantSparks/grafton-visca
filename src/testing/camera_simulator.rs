//! VISCA Camera Simulator for Testing
//!
//! This module provides a high-fidelity simulator of VISCA camera behavior
//! that can be used for integration testing without requiring real hardware.

#![allow(clippy::expect_used)]

use crate::command::const_encoding::VISCA_TERMINATOR;
use crate::transport::AsyncTransport;
use crate::Error;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use tokio::time::sleep;

/// Represents the state of a single VISCA socket
#[derive(Debug, Clone)]
enum SocketState {
    /// Socket is free and can accept commands
    Free,
    /// Socket has accepted a command and is executing it
    Executing { completion_time: Instant },
}

/// Command type categorization for execution timing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandType {
    Preset,
    PanTilt,
    Zoom,
    Focus,
    Inquiry,
    Power,
    Other,
}

/// VISCA Camera Simulator that accurately models protocol behavior
#[derive(Clone)]
pub struct ViscaCameraSimulator {
    inner: Arc<SimulatorInner>,
}

impl std::fmt::Debug for ViscaCameraSimulator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViscaCameraSimulator").finish()
    }
}

// Manual Debug implementation for SimulatorInner
struct SimulatorInner {
    // Socket management
    socket_states: RwLock<[SocketState; 2]>,

    // Response management
    response_broadcaster: broadcast::Sender<Vec<u8>>,

    // Configuration
    config: SimulatorConfig,

    // Statistics
    stats: RwLock<SimulatorStats>,
}

#[derive(Debug, Clone)]
pub struct SimulatorConfig {
    // Timing configuration
    ack_delay: Duration,
    command_execution_times: HashMap<CommandType, Duration>,

    // Behavior configuration
    packet_loss_rate: f32,
    busy_probability: f32,
    network_jitter_ms: u32,

    // Protocol configuration
    max_sockets: usize,
}

#[derive(Debug, Default)]
struct SimulatorStats {
    commands_received: usize,
    commands_executed: usize,
    busy_responses_sent: usize,
    packets_dropped: usize,
}

impl Default for SimulatorConfig {
    fn default() -> Self {
        let mut execution_times = HashMap::new();
        execution_times.insert(CommandType::Preset, Duration::from_millis(500));
        execution_times.insert(CommandType::PanTilt, Duration::from_millis(200));
        execution_times.insert(CommandType::Zoom, Duration::from_millis(100));
        execution_times.insert(CommandType::Focus, Duration::from_millis(100));
        execution_times.insert(CommandType::Inquiry, Duration::from_millis(10));
        execution_times.insert(CommandType::Power, Duration::from_millis(1000));
        execution_times.insert(CommandType::Other, Duration::from_millis(50));

        Self {
            ack_delay: Duration::from_millis(5),
            command_execution_times: execution_times,
            packet_loss_rate: 0.0,
            busy_probability: 0.0,
            network_jitter_ms: 0,
            max_sockets: 2,
        }
    }
}

impl Default for ViscaCameraSimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl ViscaCameraSimulator {
    /// Create a new simulator with default configuration
    pub fn new() -> Self {
        Self::with_config(SimulatorConfig::default())
    }

    /// Create a simulator with custom configuration
    pub fn with_config(config: SimulatorConfig) -> Self {
        let (tx, _) = broadcast::channel(100);

        let inner = Arc::new(SimulatorInner {
            socket_states: RwLock::new([SocketState::Free, SocketState::Free]),
            response_broadcaster: tx,
            config,
            stats: RwLock::new(SimulatorStats::default()),
        });

        // Start the background response processor
        let processor_inner = inner.clone();
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_millis(1)).await;
                Self::process_responses(processor_inner.clone()).await;
            }
        });

        Self { inner }
    }

    /// Create a builder for configuring the simulator
    pub fn builder() -> SimulatorBuilder {
        SimulatorBuilder::default()
    }

    /// Process pending responses and socket state transitions
    async fn process_responses(inner: Arc<SimulatorInner>) {
        let now = Instant::now();

        // Check for completed commands
        {
            let mut socket_states = inner.socket_states.write().await;
            for (socket_idx, state) in socket_states.iter_mut().enumerate() {
                if let SocketState::Executing { completion_time } = state {
                    if now >= *completion_time {
                        // Send completion response
                        let socket_num = (socket_idx + 1) as u8;
                        let completion = make_completion_response(socket_num);

                        // Add a small delay to ensure ACK is received first
                        sleep(Duration::from_millis(1)).await;
                        let _ = inner.response_broadcaster.send(completion.clone());

                        // Free the socket
                        *state = SocketState::Free;

                        // Update stats
                        let mut stats = inner.stats.write().await;
                        stats.commands_executed += 1;
                    }
                }
            }
        }
    }

    /// Parse command type from VISCA command bytes
    fn parse_command_type(data: &[u8]) -> CommandType {
        if data.len() < 3 {
            return CommandType::Other;
        }

        // Command format: 0x8X 0x01 [command bytes] 0xFF
        // OR inquiry format: 0x8X 0x09 [inquiry bytes] 0xFF

        match (data[1], data.get(2)) {
            (0x01, Some(0x04)) if data.get(3) == Some(&0x3F) => CommandType::Preset,
            (0x01, Some(0x06)) if data.get(3) == Some(&0x01) => CommandType::PanTilt,
            (0x01, Some(0x04)) if data.get(3) == Some(&0x07) => CommandType::Zoom,
            (0x01, Some(0x04)) if data.get(3) == Some(&0x08) => CommandType::Focus,
            (0x01, Some(0x04)) if data.get(3) == Some(&0x00) => CommandType::Power,
            (0x09, _) => CommandType::Inquiry,
            _ => CommandType::Other,
        }
    }

    /// Allocate a socket for a command
    async fn allocate_socket(&self) -> Option<u8> {
        let mut socket_states = self.inner.socket_states.write().await;

        for (idx, state) in socket_states.iter_mut().enumerate() {
            if matches!(state, SocketState::Free) {
                return Some((idx + 1) as u8);
            }
        }

        None
    }

    /// Should drop packet based on configured loss rate
    fn should_drop_packet(&self) -> bool {
        if self.inner.config.packet_loss_rate == 0.0 {
            return false;
        }

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::SystemTime;

        let mut hasher = DefaultHasher::new();
        SystemTime::now().hash(&mut hasher);
        let hash = hasher.finish();
        let random = (hash % 1000) as f32 / 1000.0;

        random < self.inner.config.packet_loss_rate
    }

    /// Calculate network jitter
    fn calculate_jitter(&self) -> Duration {
        if self.inner.config.network_jitter_ms == 0 {
            return Duration::from_millis(0);
        }

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::SystemTime;

        let mut hasher = DefaultHasher::new();
        SystemTime::now().hash(&mut hasher);
        let hash = hasher.finish();
        let jitter = hash % (self.inner.config.network_jitter_ms as u64);

        Duration::from_millis(jitter)
    }
}

impl AsyncTransport for ViscaCameraSimulator {
    async fn send(&self, data: &[u8]) -> Result<(), Error> {
        let data_vec = data.to_vec();
        let inner = self.inner.clone();

        // Update stats
        {
            let mut stats = inner.stats.write().await;
            stats.commands_received += 1;
        }

        // Simulate packet loss
        if Self::should_drop_packet(&ViscaCameraSimulator {
            inner: inner.clone(),
        }) {
            let mut stats = inner.stats.write().await;
            stats.packets_dropped += 1;
            return Err(Error::Timeout);
        }

        // Try to allocate a socket
        let socket_num = {
            let simulator = ViscaCameraSimulator {
                inner: inner.clone(),
            };
            simulator.allocate_socket().await
        };

        if let Some(socket_num) = socket_num {
            // Socket allocated - send ACK and schedule completion
            let cmd_type = Self::parse_command_type(&data_vec);
            let default_duration = Duration::from_millis(50);
            let execution_time = inner
                .config
                .command_execution_times
                .get(&cmd_type)
                .unwrap_or(&default_duration);

            // Mark socket as executing
            {
                let mut socket_states = inner.socket_states.write().await;
                let socket_idx = (socket_num - 1) as usize;
                socket_states[socket_idx] = SocketState::Executing {
                    completion_time: Instant::now() + *execution_time,
                };
            }

            // Send ACK immediately
            let ack = make_ack_response(socket_num);
            let _ = inner.response_broadcaster.send(ack);

            Ok(())
        } else {
            // All sockets busy - send busy response
            let mut stats = inner.stats.write().await;
            stats.busy_responses_sent += 1;

            let busy = make_busy_response(1);
            let _ = inner.response_broadcaster.send(busy);

            Ok(())
        }
    }

    async fn recv(&self) -> Result<Bytes, Error> {
        let mut rx = self.inner.response_broadcaster.subscribe();

        // Add network jitter
        let jitter = self.calculate_jitter();
        if jitter > Duration::from_millis(0) {
            sleep(jitter).await;
        }

        // Wait for response with timeout
        match tokio::time::timeout(Duration::from_secs(30), rx.recv()).await {
            Ok(Ok(response)) => Ok(Bytes::from(response)),
            Ok(Err(_)) => Err(Error::Timeout),
            Err(_) => Err(Error::Timeout),
        }
    }
}

/// Builder for configuring ViscaCameraSimulator
#[derive(Default, Debug)]
pub struct SimulatorBuilder {
    config: SimulatorConfig,
}

impl SimulatorBuilder {
    /// Set the number of sockets (default: 2)
    pub fn with_socket_count(mut self, count: usize) -> Self {
        self.config.max_sockets = count;
        self
    }

    /// Set ACK delay (default: 5ms)
    pub fn with_ack_delay(mut self, delay: Duration) -> Self {
        self.config.ack_delay = delay;
        self
    }

    /// Set execution time for a specific command type
    pub fn with_command_execution_time(
        mut self,
        cmd_type: CommandType,
        duration: Duration,
    ) -> Self {
        self.config
            .command_execution_times
            .insert(cmd_type, duration);
        self
    }

    /// Set packet loss rate (0.0 to 1.0)
    pub fn with_packet_loss(mut self, rate: f32) -> Self {
        self.config.packet_loss_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Set probability of busy responses (0.0 to 1.0)
    pub fn with_busy_probability(mut self, prob: f32) -> Self {
        self.config.busy_probability = prob.clamp(0.0, 1.0);
        self
    }

    /// Set network jitter in milliseconds
    pub fn with_network_jitter(mut self, jitter_ms: u32) -> Self {
        self.config.network_jitter_ms = jitter_ms;
        self
    }

    /// Build the simulator
    pub fn build(self) -> ViscaCameraSimulator {
        ViscaCameraSimulator::with_config(self.config)
    }
}

// Helper functions for VISCA response generation
fn make_ack_response(socket_num: u8) -> Vec<u8> {
    vec![0x90, 0x40 | socket_num, VISCA_TERMINATOR]
}

fn make_completion_response(socket_num: u8) -> Vec<u8> {
    vec![0x90, 0x50 | socket_num, VISCA_TERMINATOR]
}

fn make_busy_response(socket_num: u8) -> Vec<u8> {
    vec![0x90, 0x60 | socket_num, 0x03, VISCA_TERMINATOR]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_type_parsing() {
        // Test preset command
        let preset_cmd = vec![0x81, 0x01, 0x04, 0x3F, 0x01, 0x00, VISCA_TERMINATOR];
        assert_eq!(
            ViscaCameraSimulator::parse_command_type(&preset_cmd),
            CommandType::Preset
        );

        // Test zoom command
        let zoom_cmd = vec![0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR];
        assert_eq!(
            ViscaCameraSimulator::parse_command_type(&zoom_cmd),
            CommandType::Zoom
        );

        // Test inquiry command
        let inquiry_cmd = vec![0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR];
        assert_eq!(
            ViscaCameraSimulator::parse_command_type(&inquiry_cmd),
            CommandType::Inquiry
        );
    }

    #[tokio::test]
    async fn test_simulator_basic_operation() {
        let simulator = ViscaCameraSimulator::new();

        // Subscribe to responses before sending command
        let mut rx = simulator.inner.response_broadcaster.subscribe();

        // Send a command
        let command = vec![0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR]; // Zoom in
        simulator.send(&command).await.expect("should send command");

        // Should receive ACK
        let response = rx.recv().await.expect("should receive ACK");
        assert_eq!(response[0], 0x90);
        assert_eq!(response[1] & 0xF0, 0x40); // ACK response

        // Wait a bit for completion to be generated
        sleep(Duration::from_millis(100)).await;

        // Should eventually receive completion
        let response = rx.recv().await.expect("should receive completion");
        assert_eq!(response[0], 0x90);
        assert_eq!(response[1] & 0xF0, 0x50); // Completion response
    }
}
