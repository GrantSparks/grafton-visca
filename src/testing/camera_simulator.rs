//! VISCA Camera Simulator for Testing
//!
//! This module provides a high-fidelity simulator of VISCA camera behavior
//! that can be used for integration testing without requiring real hardware.

#![allow(clippy::expect_used)]

use tokio::sync::{broadcast, RwLock};
use tokio::time::sleep;

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    command::bytes::VISCA_TERMINATOR,
    transport::{builder::TransportConfig, AsyncTransport, HasTransportConfig},
    Error,
};

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
    /// Preset memory operations
    Preset,
    /// Pan/tilt movement commands
    PanTilt,
    /// Zoom control commands
    Zoom,
    /// Focus control commands
    Focus,
    /// Inquiry/query commands
    Inquiry,
    /// Power control commands
    Power,
    /// All other command types
    Other,
}

/// VISCA Camera Simulator that accurately models protocol behavior
pub struct ViscaCameraSimulator {
    inner: Arc<SimulatorInner>,
    // Each clone gets its own receiver to avoid missing broadcasts
    receiver: Option<Arc<tokio::sync::Mutex<broadcast::Receiver<Vec<u8>>>>>,
    transport_config: TransportConfig,
}

impl Clone for ViscaCameraSimulator {
    fn clone(&self) -> Self {
        // Create a new receiver for the clone
        let rx = self.inner.response_broadcaster.subscribe();
        Self {
            inner: self.inner.clone(),
            receiver: Some(Arc::new(tokio::sync::Mutex::new(rx))),
            transport_config: self.transport_config,
        }
    }
}

impl std::fmt::Debug for ViscaCameraSimulator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViscaCameraSimulator").finish()
    }
}

/// Simulated camera state for generating inquiry responses
#[derive(Debug, Clone)]
struct CameraState {
    // Power and system
    power_on: bool,

    // Position
    pan_position: i16,
    tilt_position: i16,
    zoom_position: u16,
    focus_position: u16,
    focus_near_limit: u16,

    // Exposure
    exposure_mode: u8,
    exposure_compensation: i8,
    exposure_compensation_enabled: bool,
    iris_position: u16,
    shutter_speed: u8,
    brightness: u8,
    gain_level: u8,
    gain_limit: u8,
    backlight_enabled: bool,

    // White balance
    white_balance_mode: u8,
    color_temperature: u16,

    // Image adjustments
    // NOTE: sharpness, contrast, and luminance are not documented in VISCA specs
    // and have been removed from the simulator until proper documentation is found
    saturation: u8,
    hue: u8,

    // Image settings
    image_flip_vertical: bool,
    image_flip_horizontal: bool,

    // Noise reduction
    noise_reduction_2d: u8,
    noise_reduction_3d: u8,

    // Focus
    focus_mode: u8,
    // NOTE: auto_focus_enabled is not documented in VISCA specs
    // and has been removed from the simulator until proper documentation is found

    // Resolution
    resolution: u8,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            power_on: true,
            pan_position: 0,
            tilt_position: 0,
            zoom_position: 0x0000,
            focus_position: 0x1000,
            focus_near_limit: 0x1000,
            exposure_mode: 0x00, // Auto
            exposure_compensation: 0,
            exposure_compensation_enabled: false,
            iris_position: 0x0000,
            shutter_speed: 0x00,
            brightness: 0x07,
            gain_level: 0x00,
            gain_limit: 0x07,
            backlight_enabled: false,
            white_balance_mode: 0x00, // Auto
            color_temperature: 3,     // VISCA value 3 = 2800K
            saturation: 0x07,
            hue: 0x07,
            image_flip_vertical: false,
            image_flip_horizontal: false,
            noise_reduction_2d: 0x01,
            noise_reduction_3d: 0x01,
            focus_mode: 0x02, // Auto
            resolution: 0x00, // 1080p
        }
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

    // Camera state
    camera_state: RwLock<CameraState>,
}

/// Configuration for the VISCA camera simulator
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
        let (tx, rx) = broadcast::channel(100);

        let inner = Arc::new(SimulatorInner {
            socket_states: RwLock::new([SocketState::Free, SocketState::Free]),
            response_broadcaster: tx,
            config,
            stats: RwLock::new(SimulatorStats::default()),
            camera_state: RwLock::new(CameraState::default()),
        });

        // Start the background response processor
        let processor_inner = inner.clone();
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_millis(1)).await;
                Self::process_responses(processor_inner.clone()).await;
            }
        });

        Self {
            inner,
            receiver: Some(Arc::new(tokio::sync::Mutex::new(rx))),
            transport_config: TransportConfig::default(),
        }
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

    /// Generate inquiry response based on the inquiry command
    async fn generate_inquiry_response(&self, data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 4 || data[1] != 0x09 {
            tracing::debug!("Not an inquiry command: {:02X?}", data);
            return None;
        }

        let state = self.inner.camera_state.read().await;

        tracing::debug!(
            "Processing inquiry: cmd[2]={:02X?}, cmd[3]={:02X?}, full command: {:02X?}",
            data.get(2),
            data.get(3),
            data
        );

        // Parse inquiry type from command bytes
        match (data.get(2), data.get(3)) {
            // Power inquiry: 0x81 0x09 0x04 0x00 0xFF
            (Some(0x04), Some(0x00)) => {
                let status = if state.power_on { 0x02 } else { 0x03 };
                Some(vec![0x90, 0x50, status, VISCA_TERMINATOR])
            }

            // Pan/Tilt position inquiry: 0x81 0x09 0x06 0x12 0xFF
            (Some(0x06), Some(0x12)) => {
                let pan_bytes = encode_signed_position(state.pan_position);
                let tilt_bytes = encode_signed_position(state.tilt_position);
                Some(vec![
                    0x90,
                    0x50,
                    pan_bytes[0],
                    pan_bytes[1],
                    pan_bytes[2],
                    pan_bytes[3],
                    tilt_bytes[0],
                    tilt_bytes[1],
                    tilt_bytes[2],
                    tilt_bytes[3],
                    VISCA_TERMINATOR,
                ])
            }

            // Zoom position inquiry: 0x81 0x09 0x04 0x47 0xFF
            (Some(0x04), Some(0x47)) => {
                let zoom_bytes = encode_position(state.zoom_position);
                Some(vec![
                    0x90,
                    0x50,
                    zoom_bytes[0],
                    zoom_bytes[1],
                    zoom_bytes[2],
                    zoom_bytes[3],
                    VISCA_TERMINATOR,
                ])
            }

            // Focus position inquiry: 0x81 0x09 0x04 0x48 0xFF
            (Some(0x04), Some(0x48)) => {
                let focus_bytes = encode_position(state.focus_position);
                Some(vec![
                    0x90,
                    0x50,
                    focus_bytes[0],
                    focus_bytes[1],
                    focus_bytes[2],
                    focus_bytes[3],
                    VISCA_TERMINATOR,
                ])
            }

            // Focus near limit inquiry: 0x81 0x09 0x04 0x28 0xFF
            (Some(0x04), Some(0x28)) => {
                let limit_bytes = encode_position(state.focus_near_limit);
                Some(vec![
                    0x90,
                    0x50,
                    limit_bytes[0],
                    limit_bytes[1],
                    limit_bytes[2],
                    limit_bytes[3],
                    VISCA_TERMINATOR,
                ])
            }

            // Exposure mode inquiry: 0x81 0x09 0x04 0x39 0xFF
            (Some(0x04), Some(0x39)) => {
                Some(vec![0x90, 0x50, state.exposure_mode, VISCA_TERMINATOR])
            }

            // Exposure compensation inquiry: 0x81 0x09 0x04 0x4E 0xFF
            (Some(0x04), Some(0x4E)) => {
                // Exposure compensation value needs to be offset by 7 and encoded as nibbles
                // The parser expects 4 bytes: 0x00, 0x00, nibble1, nibble2
                let adjusted_value = (state.exposure_compensation + 7) as u8;
                let nibble_high = (adjusted_value >> 4) & 0x0F;
                let nibble_low = adjusted_value & 0x0F;
                let response = vec![
                    0x90,
                    0x50,
                    0x00,
                    0x00,
                    nibble_high,
                    nibble_low,
                    VISCA_TERMINATOR,
                ];
                tracing::debug!("Exposure compensation inquiry response: {:02X?}", response);
                Some(response)
            }

            // Exposure compensation mode inquiry: 0x81 0x09 0x04 0x3E 0xFF
            (Some(0x04), Some(0x3E)) => {
                let status = if state.exposure_compensation_enabled {
                    0x02
                } else {
                    0x03
                };
                Some(vec![0x90, 0x50, status, VISCA_TERMINATOR])
            }

            // Exposure compensation on/off inquiry: 0x81 0x09 0x04 0x3F 0xFF
            (Some(0x04), Some(0x3F)) => {
                let status = if state.exposure_compensation_enabled {
                    0x02
                } else {
                    0x03
                };
                Some(vec![0x90, 0x50, status, VISCA_TERMINATOR])
            }

            // Iris inquiry: 0x81 0x09 0x04 0x4B 0xFF
            (Some(0x04), Some(0x4B)) => {
                let iris_bytes = encode_position(state.iris_position);
                Some(vec![
                    0x90,
                    0x50,
                    iris_bytes[0],
                    iris_bytes[1],
                    iris_bytes[2],
                    iris_bytes[3],
                    VISCA_TERMINATOR,
                ])
            }

            // Shutter inquiry: 0x81 0x09 0x04 0x4A 0xFF
            (Some(0x04), Some(0x4A)) => {
                // Shutter speed needs to be encoded as 4 nibbles
                let shutter_bytes = encode_position(state.shutter_speed as u16);
                Some(vec![
                    0x90,
                    0x50,
                    shutter_bytes[0],
                    shutter_bytes[1],
                    shutter_bytes[2],
                    shutter_bytes[3],
                    VISCA_TERMINATOR,
                ])
            }

            // Brightness inquiry: 0x81 0x09 0x04 0x4D 0xFF
            (Some(0x04), Some(0x4D)) => {
                // Brightness needs to be encoded as 4 nibbles
                let bright_bytes = encode_position(state.brightness as u16);
                Some(vec![
                    0x90,
                    0x50,
                    bright_bytes[0],
                    bright_bytes[1],
                    bright_bytes[2],
                    bright_bytes[3],
                    VISCA_TERMINATOR,
                ])
            }

            // Gain inquiry: 0x81 0x09 0x04 0x4C 0xFF
            (Some(0x04), Some(0x4C)) => {
                // Gain needs to be encoded as 4 nibbles
                let gain_bytes = encode_position(state.gain_level as u16);
                Some(vec![
                    0x90,
                    0x50,
                    gain_bytes[0],
                    gain_bytes[1],
                    gain_bytes[2],
                    gain_bytes[3],
                    VISCA_TERMINATOR,
                ])
            }

            // Gain limit inquiry: 0x81 0x09 0x04 0x2C 0xFF
            (Some(0x04), Some(0x2C)) => Some(vec![0x90, 0x50, state.gain_limit, VISCA_TERMINATOR]),

            // Backlight inquiry: 0x81 0x09 0x04 0x33 0xFF
            (Some(0x04), Some(0x33)) => {
                let status = if state.backlight_enabled { 0x02 } else { 0x03 };
                Some(vec![0x90, 0x50, status, VISCA_TERMINATOR])
            }

            // White balance mode inquiry: 0x81 0x09 0x04 0x35 0xFF
            (Some(0x04), Some(0x35)) => {
                Some(vec![0x90, 0x50, state.white_balance_mode, VISCA_TERMINATOR])
            }

            // Color temperature inquiry: 0x81 0x09 0x04 0x20 0xFF
            (Some(0x04), Some(0x20)) => {
                // Color temperature is returned as 2 nibbles at positions 2 and 3
                // The parser does: temperature = nibbles.u8_pair(2) as u16
                // For 2800K which maps to value 55 (0x37 in the VISCA scale),
                // we need nibbles: [0x00, 0x00, 0x03, 0x07]
                // But if the state is storing the actual K value (2800), we need to convert
                // The simulator appears to be using a simplified mapping where the value
                // is just sent as nibbles directly
                let temp_value = state.color_temperature;
                // Extract nibbles from the temperature value
                let nibble_high = ((temp_value >> 4) & 0x0F) as u8;
                let nibble_low = (temp_value & 0x0F) as u8;
                Some(vec![
                    0x90,
                    0x50,
                    0x00,
                    0x00,
                    nibble_high,
                    nibble_low,
                    VISCA_TERMINATOR,
                ])
            }

            // NOTE: Sharpness and contrast inquiries are not documented in VISCA specs
            // and have been disabled until proper documentation is found.

            // // Sharpness inquiry: 0x81 0x09 0x04 0x42 0xFF
            // (Some(0x04), Some(0x42)) => Some(vec![0x90, 0x50, state.sharpness, VISCA_TERMINATOR]),

            // // Contrast inquiry: 0x81 0x09 0x4E 0x50 0xFF
            // (Some(0x4E), Some(0x50)) => {
            //     Some(vec![0x90, 0x50, 0x00, state.contrast, VISCA_TERMINATOR])
            // }

            // Saturation inquiry: 0x81 0x09 0x04 0x49 0xFF
            (Some(0x04), Some(0x49)) => {
                // Saturation needs to be encoded as 4 nibbles
                let sat_bytes = encode_position(state.saturation as u16);
                Some(vec![
                    0x90,
                    0x50,
                    sat_bytes[0],
                    sat_bytes[1],
                    sat_bytes[2],
                    sat_bytes[3],
                    VISCA_TERMINATOR,
                ])
            }

            // Hue inquiry: 0x81 0x09 0x04 0x4F 0xFF
            (Some(0x04), Some(0x4F)) => {
                // Hue needs to be encoded as 4 nibbles, with the value in the last position
                Some(vec![
                    0x90,
                    0x50,
                    0x00,
                    0x00,
                    0x00,
                    state.hue,
                    VISCA_TERMINATOR,
                ])
            }

            // NOTE: Luminance inquiry is not documented in VISCA specs
            // and has been disabled until proper documentation is found.

            // // Luminance inquiry: 0x81 0x09 0x4D 0x50 0xFF
            // (Some(0x4D), Some(0x50)) => Some(vec![0x90, 0x50, state.luminance, VISCA_TERMINATOR]),

            // Image flip inquiry: 0x81 0x09 0x04 0x66 0xFF
            (Some(0x04), Some(0x66)) => {
                let flip_mode = match (state.image_flip_vertical, state.image_flip_horizontal) {
                    (false, false) => 0x00, // Off
                    (false, true) => 0x01,  // Horizontal
                    (true, false) => 0x02,  // Vertical
                    (true, true) => 0x03,   // Both
                };
                Some(vec![0x90, 0x50, flip_mode, VISCA_TERMINATOR])
            }

            // Noise reduction 2D inquiry: 0x81 0x09 0x04 0x53 0xFF
            (Some(0x04), Some(0x53)) => {
                Some(vec![0x90, 0x50, state.noise_reduction_2d, VISCA_TERMINATOR])
            }

            // Noise reduction 3D inquiry: 0x81 0x09 0x04 0x54 0xFF
            (Some(0x04), Some(0x54)) => {
                Some(vec![0x90, 0x50, state.noise_reduction_3d, VISCA_TERMINATOR])
            }

            // Focus mode inquiry: 0x81 0x09 0x04 0x38 0xFF
            (Some(0x04), Some(0x38)) => Some(vec![0x90, 0x50, state.focus_mode, VISCA_TERMINATOR]),

            // NOTE: AutoFocus inquiry is not documented in VISCA specs
            // and has been disabled until proper documentation is found.

            // // Auto focus inquiry: 0x81 0x09 0x04 0x18 0xFF
            // (Some(0x04), Some(0x18)) => {
            //     let status = if state.auto_focus_enabled { 0x02 } else { 0x03 };
            //     Some(vec![0x90, 0x50, status, VISCA_TERMINATOR])
            // }

            // Resolution inquiry: 0x81 0x09 0x04 0x63 0xFF
            (Some(0x04), Some(0x63)) => Some(vec![0x90, 0x50, state.resolution, VISCA_TERMINATOR]),

            _ => {
                tracing::warn!(
                    "Unhandled inquiry command: cmd[2]={:02X?}, cmd[3]={:02X?}, full: {:02X?}",
                    data.get(2),
                    data.get(3),
                    data
                );
                None
            }
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
    async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        let data_vec = data.to_vec();
        let inner = self.inner.clone();
        let receiver = self.receiver.clone();

        // Create a simulator instance for method calls
        let simulator = ViscaCameraSimulator {
            inner: inner.clone(),
            receiver: receiver.clone(),
            transport_config: TransportConfig::default(),
        };

        // Update stats
        {
            let mut stats = inner.stats.write().await;
            stats.commands_received += 1;
        }

        tracing::trace!("Simulator received command: {:02X?}", data_vec);

        // Simulate packet loss
        if ViscaCameraSimulator::should_drop_packet(&ViscaCameraSimulator {
            inner: inner.clone(),
            receiver: None,
            transport_config: TransportConfig::default(),
        }) {
            let mut stats = inner.stats.write().await;
            stats.packets_dropped += 1;
            return Err(Error::Timeout);
        }

        // Check if this is an inquiry command
        let cmd_type = ViscaCameraSimulator::parse_command_type(&data_vec);
        if cmd_type == CommandType::Inquiry {
            // Handle inquiry immediately - generate and broadcast Data Reply
            if let Some(response) = simulator.generate_inquiry_response(&data_vec).await {
                // Add a small delay to ensure receivers are ready
                // This simulates real network latency and prevents race conditions in tests
                sleep(Duration::from_millis(10)).await;

                // Broadcast inquiry response immediately (no ACK for inquiries)
                tracing::debug!("Broadcasting inquiry response: {:02X?}", response);
                match inner.response_broadcaster.send(response.clone()) {
                    Ok(count) => {
                        tracing::debug!("Inquiry response broadcast to {} receivers", count);
                    }
                    Err(e) => {
                        tracing::error!("Failed to broadcast inquiry response: {:?}", e);
                    }
                }
                return Ok(());
            } else {
                tracing::warn!(
                    "No response generated for inquiry command: {:02X?}",
                    data_vec
                );
            }
        }

        // Try to allocate a socket for non-inquiry commands
        let socket_num = {
            let simulator = ViscaCameraSimulator {
                inner: inner.clone(),
                receiver: None,
                transport_config: TransportConfig::default(),
            };
            simulator.allocate_socket().await
        };

        if let Some(socket_num) = socket_num {
            // Socket allocated - send ACK and schedule completion
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

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        let inner = self.inner.clone();
        let receiver = self.receiver.clone();
        let jitter = self.calculate_jitter();

        // Add network jitter
        if jitter > Duration::from_millis(0) {
            sleep(jitter).await;
        }

        // Use persistent receiver if available
        let response = if let Some(ref receiver) = receiver {
            let receiver = receiver.clone();
            let mut rx = receiver.lock().await;

            // Wait for response with timeout
            match tokio::time::timeout(Duration::from_secs(30), rx.recv()).await {
                Ok(Ok(response)) => response,
                Ok(Err(_)) => return Err(Error::Timeout),
                Err(_) => return Err(Error::Timeout),
            }
        } else {
            // Fallback: create a new subscriber
            let mut rx = inner.response_broadcaster.subscribe();

            // Wait for response with timeout
            match tokio::time::timeout(Duration::from_secs(30), rx.recv()).await {
                Ok(Ok(response)) => response,
                Ok(Err(_)) => return Err(Error::Timeout),
                Err(_) => return Err(Error::Timeout),
            }
        };

        // Copy response data into the provided buffer
        let len = response.len().min(dst.len());
        dst[..len].copy_from_slice(&response[..len]);
        Ok(len)
    }
}

impl HasTransportConfig for ViscaCameraSimulator {
    fn transport_config(&self) -> &TransportConfig {
        &self.transport_config
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

// Helper functions for encoding position values
fn encode_position(value: u16) -> [u8; 4] {
    [
        (value >> 12) as u8 & 0x0F,
        (value >> 8) as u8 & 0x0F,
        (value >> 4) as u8 & 0x0F,
        value as u8 & 0x0F,
    ]
}

fn encode_signed_position(value: i16) -> [u8; 4] {
    let abs_val = value.unsigned_abs();
    let nibbles = encode_position(abs_val);

    if value >= 0 {
        nibbles
    } else {
        // For negative values, set high bit of each nibble
        [
            nibbles[0] | 0x0F,
            nibbles[1] | 0x0F,
            nibbles[2] | 0x0F,
            nibbles[3] | 0x0F,
        ]
    }
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
        let mut simulator = ViscaCameraSimulator::new();

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

    #[tokio::test]
    async fn test_simulator_inquiry_responses() {
        let mut simulator = ViscaCameraSimulator::new();

        // Subscribe to responses before sending command
        let mut rx = simulator.inner.response_broadcaster.subscribe();

        // Test power inquiry
        let power_inquiry = vec![0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR];
        simulator
            .send(&power_inquiry)
            .await
            .expect("should send inquiry");

        // Should receive data reply immediately (no ACK for inquiries)
        let response = rx.recv().await.expect("should receive data reply");
        assert_eq!(response[0], 0x90);
        assert_eq!(response[1], 0x50); // Data reply
        assert_eq!(response[2], 0x02); // Power on
        assert_eq!(response[3], VISCA_TERMINATOR);

        // Test zoom position inquiry
        let zoom_inquiry = vec![0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR];
        simulator
            .send(&zoom_inquiry)
            .await
            .expect("should send inquiry");

        let response = rx.recv().await.expect("should receive zoom reply");
        assert_eq!(response[0], 0x90);
        assert_eq!(response[1], 0x50); // Data reply
        assert_eq!(response.len(), 7); // 0x90 0x50 [4 nibbles] 0xFF

        // Test pan/tilt position inquiry
        let pt_inquiry = vec![0x81, 0x09, 0x06, 0x12, VISCA_TERMINATOR];
        simulator
            .send(&pt_inquiry)
            .await
            .expect("should send inquiry");

        let response = rx.recv().await.expect("should receive pan/tilt reply");
        assert_eq!(response[0], 0x90);
        assert_eq!(response[1], 0x50); // Data reply
        assert_eq!(response.len(), 11); // 0x90 0x50 [8 nibbles] 0xFF
    }
}
