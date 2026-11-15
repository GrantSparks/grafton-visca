//! VISCA Camera Simulator for Testing
//!
//! Simulates Standard VISCA wire replies, per-camera command sockets, timing,
//! cancellation and selected camera state without hardware. Power, pan/tilt
//! absolute/home, zoom/focus direct, and the exposure, white-balance, image,
//! noise-reduction, focus and USB-audio registers answered by inquiries change
//! when their commands complete. Cancellation before completion applies nothing.
//!
//! Continuous drives, relative moves and preset motion do not change reported
//! positions; the simulator does not integrate physical motion or scene-driven
//! auto exposure/focus. It models wire register values without profile-specific
//! limits. Position replies use standard signed 4+4-nibble words; Sony BRC-300's
//! 5+4 and FR7's 5+5 profile codecs are not modeled.

#![allow(clippy::expect_used)]

use tokio::{
    sync::{broadcast, Mutex, Notify, RwLock},
    task::AbortHandle,
    time::{sleep_until, Instant},
};

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex as StdMutex,
    },
    time::Duration,
};

use super::frames;
use crate::{
    command::bytes::VISCA_TERMINATOR,
    transport::{builder::TransportConfig, AsyncTransport, HasTransportConfig, ReceiveOutcome},
    Error,
};

/// Represents the state of a single VISCA socket
#[derive(Debug, Clone)]
enum SocketState {
    /// Socket is free and can accept commands
    Free,
    /// Socket has accepted a command and is executing it
    Executing {
        ack_at: Instant,
        completion_at: Instant,
        ack_pending: bool,
        effect: Option<CommandEffect>,
    },
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

/// Simulator for Standard VISCA framing in integration tests.
///
/// Pan/tilt position state is modeled as `i16`, and position inquiries always
/// emit the standard signed 4+4-nibble reply form. This type has no profile or
/// codec selector and does not model Sony BRC-300's profile-owned 5+4 position
/// codec.
///
/// Construction does not require a runtime. Sending and receiving require a
/// Tokio runtime with its time driver enabled. Replies use datagram semantics
/// and retain their camera source, so one simulator may serve an addressed
/// multi-camera session. Each address has its own command sockets.
///
/// Loss, busy responses and jitter use a fixed seed, making the same sequence
/// of sends reproducible. Loss omits replies; it never fails a local write.
/// The event-driven response task stops when the last simulator clone drops.
pub struct ViscaCameraSimulator {
    inner: Arc<SimulatorInner>,
    // Each clone gets its own receiver to avoid missing broadcasts
    receiver: Option<Arc<Mutex<broadcast::Receiver<Vec<u8>>>>>,
    transport_config: TransportConfig,
}

impl Clone for ViscaCameraSimulator {
    fn clone(&self) -> Self {
        // Create a new receiver for the clone
        let rx = self.inner.response_broadcaster.subscribe();
        Self {
            inner: self.inner.clone(),
            receiver: Some(Arc::new(Mutex::new(rx))),
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
    contrast: u8,
    luminance: u8,
    saturation: u8,
    hue: u8,

    // Image settings
    image_flip_vertical: bool,
    image_flip_horizontal: bool,

    // Noise reduction
    noise_reduction_2d_mode: u8,
    noise_reduction_2d: u8,
    noise_reduction_3d: u8,

    // Focus
    focus_mode: u8,
    focus_zone: u8,
    // NOTE: auto_focus_enabled is not documented in VISCA specs
    // and has been removed from the simulator until proper documentation is found

    // Picture effect
    picture_effect: u8,

    // PTZOptics UAC vendor extension
    usb_audio_enabled: bool,
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
            shutter_speed: 0x01, // Valid shutter speed (0x01-0x11)
            brightness: 0x07,
            gain_level: 0x00,
            gain_limit: 0x07,
            backlight_enabled: false,
            white_balance_mode: 0x00, // Auto
            color_temperature: 3,     // VISCA value 3 = 2800K
            contrast: 0x09,
            luminance: 0x06,
            saturation: 0x07,
            hue: 0x07,
            image_flip_vertical: false,
            image_flip_horizontal: false,
            noise_reduction_2d_mode: 0x02,
            noise_reduction_2d: 0x01,
            noise_reduction_3d: 0x01,
            focus_mode: 0x02,        // Auto
            focus_zone: 0x01,        // Center
            picture_effect: 0x02,    // Off
            usb_audio_enabled: true, // 0x02 = on
        }
    }
}

/// A well-formed command's modeled effect, held by its executing socket.
#[derive(Debug, Clone, Copy)]
enum CommandEffect {
    PanTilt { pan: i16, tilt: i16 },
    CombinedFlip(u8),
    ToggleFocus,
    Power(bool),
    Zoom(u16),
    Focus(u16),
    FocusNearLimit(u16),
    ExposureMode(u8),
    Compensation(i8),
    CompensationEnabled(bool),
    Iris(u16),
    Shutter(u8),
    Brightness(u8),
    Gain(u8),
    GainLimit(u8),
    Backlight(bool),
    WhiteBalance(u8),
    ColorTemperature(u16),
    Contrast(u8),
    Luminance(u8),
    Saturation(u8),
    Hue(u8),
    VerticalFlip(bool),
    HorizontalFlip(bool),
    Noise2dMode(u8),
    Noise2d(u8),
    Noise3d(u8),
    FocusMode(u8),
    FocusZone(u8),
    PictureEffect(u8),
    UsbAudio(bool),
    AdjustCompensation(Adjustment),
    AdjustIris(Adjustment),
    AdjustShutter(Adjustment),
    AdjustBrightness(Adjustment),
    AdjustGain(Adjustment),
    AdjustColorTemperature(Adjustment),
}

#[derive(Debug, Clone, Copy)]
enum Adjustment {
    Reset,
    Up,
    Down,
}

impl Adjustment {
    fn parse(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(Self::Reset),
            0x02 => Some(Self::Up),
            0x03 => Some(Self::Down),
            _ => None,
        }
    }

    fn apply(self, value: i32, reset: i32, lower: i32, upper: i32) -> i32 {
        match self {
            Self::Reset => reset,
            Self::Up => (value + 1).clamp(lower, upper),
            Self::Down => (value - 1).clamp(lower, upper),
        }
    }
}

impl CommandEffect {
    fn parse(data: &[u8]) -> Option<Self> {
        let body = data.get(1..data.len().checked_sub(1)?)?;
        match body {
            [0x01, 0x06, 0x04] => Some(Self::PanTilt { pan: 0, tilt: 0 }),
            [0x01, 0x06, 0x02, _, _, positions @ ..] if positions.len() == 8 => {
                let pan = decode_position(&positions[..4])?;
                let tilt = decode_position(&positions[4..])?;
                Some(Self::PanTilt {
                    pan: i16::from_be_bytes(pan.to_be_bytes()),
                    tilt: i16::from_be_bytes(tilt.to_be_bytes()),
                })
            }
            [0x2A, 0x02, 0xA0, 0x04, value @ (0x02 | 0x03)] => Some(Self::UsbAudio(*value == 0x02)),
            [0x01, 0x04, register, value @ ..] => match (*register, value) {
                (0x00, [value @ (0x02 | 0x03)]) => Some(Self::Power(*value == 0x02)),
                (0x3E, [value @ (0x02 | 0x03)]) => Some(Self::CompensationEnabled(*value == 0x02)),
                (0x33, [value @ (0x02 | 0x03)]) => Some(Self::Backlight(*value == 0x02)),
                (0x66, [value @ (0x02 | 0x03)]) => Some(Self::VerticalFlip(*value == 0x02)),
                (0x61, [value @ (0x02 | 0x03)]) => Some(Self::HorizontalFlip(*value == 0x02)),
                (0x38, [0x10]) => Some(Self::ToggleFocus),
                (0xA4, [value @ 0x00..=0x03]) => Some(Self::CombinedFlip(*value)),
                (0x39, [value]) => Some(Self::ExposureMode(*value)),
                (0x35, [value]) => Some(Self::WhiteBalance(*value)),
                (0x2C, [value]) => Some(Self::GainLimit(*value)),
                (0x50, [value]) => Some(Self::Noise2dMode(*value)),
                (0x53, [value]) => Some(Self::Noise2d(*value)),
                (0x54, [value]) => Some(Self::Noise3d(*value)),
                (0x38, [value]) => Some(Self::FocusMode(*value)),
                (0xAA, [value]) => Some(Self::FocusZone(*value)),
                (0x63, [value]) => Some(Self::PictureEffect(*value)),
                (0x47, value) if value.len() == 4 => Some(Self::Zoom(decode_position(value)?)),
                (0x48, value) if value.len() == 4 => Some(Self::Focus(decode_position(value)?)),
                (0x28, value) if value.len() == 4 => {
                    Some(Self::FocusNearLimit(decode_position(value)?))
                }
                (0x4B, value) if value.len() == 4 => Some(Self::Iris(decode_position(value)?)),
                (0x4A, value) if value.len() == 4 => {
                    Some(Self::Shutter(u8::try_from(decode_position(value)?).ok()?))
                }
                (0x4D, value) if value.len() == 4 => Some(Self::Brightness(
                    u8::try_from(decode_position(value)?).ok()?,
                )),
                (0x4C, value) if value.len() == 4 => {
                    Some(Self::Gain(u8::try_from(decode_position(value)?).ok()?))
                }
                (0xA2, value) if value.len() == 4 => {
                    Some(Self::Contrast(u8::try_from(decode_position(value)?).ok()?))
                }
                (0xA1, value) if value.len() == 4 => {
                    Some(Self::Luminance(u8::try_from(decode_position(value)?).ok()?))
                }
                (0x49, value) if value.len() == 4 => Some(Self::Saturation(
                    u8::try_from(decode_position(value)?).ok()?,
                )),
                (0x4F, value) if value.len() == 4 => {
                    Some(Self::Hue(u8::try_from(decode_position(value)?).ok()?))
                }
                (0x4E, value) if value.len() == 4 => {
                    let value = decode_position(value)?;
                    (value <= 14).then(|| {
                        Self::Compensation(
                            i8::try_from(value).expect("compensation is at most fourteen") - 7,
                        )
                    })
                }
                (0x20, value) if value.len() == 2 => {
                    Some(Self::ColorTemperature(decode_position(value)?))
                }
                (0x0E, [step]) => Some(Self::AdjustCompensation(Adjustment::parse(*step)?)),
                (0x0B, [step]) => Some(Self::AdjustIris(Adjustment::parse(*step)?)),
                (0x0A, [step]) => Some(Self::AdjustShutter(Adjustment::parse(*step)?)),
                (0x0D, [step]) => Some(Self::AdjustBrightness(Adjustment::parse(*step)?)),
                (0x0C, [step]) => Some(Self::AdjustGain(Adjustment::parse(*step)?)),
                (0x20, [step]) => Some(Self::AdjustColorTemperature(Adjustment::parse(*step)?)),
                _ => None,
            },
            _ => None,
        }
    }

    fn apply(self, state: &mut CameraState) {
        match self {
            Self::PanTilt { pan, tilt } => {
                state.pan_position = pan;
                state.tilt_position = tilt;
            }
            Self::CombinedFlip(value) => {
                state.image_flip_horizontal = value & 1 != 0;
                state.image_flip_vertical = value & 2 != 0;
            }
            Self::ToggleFocus => {
                state.focus_mode = if state.focus_mode == 0x02 { 0x03 } else { 0x02 }
            }
            Self::Power(value) => state.power_on = value,
            Self::Zoom(value) => state.zoom_position = value,
            Self::Focus(value) => state.focus_position = value,
            Self::FocusNearLimit(value) => state.focus_near_limit = value,
            Self::ExposureMode(value) => state.exposure_mode = value,
            Self::Compensation(value) => state.exposure_compensation = value,
            Self::CompensationEnabled(value) => state.exposure_compensation_enabled = value,
            Self::Iris(value) => state.iris_position = value,
            Self::Shutter(value) => state.shutter_speed = value,
            Self::Brightness(value) => state.brightness = value,
            Self::Gain(value) => state.gain_level = value,
            Self::GainLimit(value) => state.gain_limit = value,
            Self::Backlight(value) => state.backlight_enabled = value,
            Self::WhiteBalance(value) => state.white_balance_mode = value,
            Self::ColorTemperature(value) => state.color_temperature = value,
            Self::Contrast(value) => state.contrast = value,
            Self::Luminance(value) => state.luminance = value,
            Self::Saturation(value) => state.saturation = value,
            Self::Hue(value) => state.hue = value,
            Self::VerticalFlip(value) => state.image_flip_vertical = value,
            Self::HorizontalFlip(value) => state.image_flip_horizontal = value,
            Self::Noise2dMode(value) => state.noise_reduction_2d_mode = value,
            Self::Noise2d(value) => state.noise_reduction_2d = value,
            Self::Noise3d(value) => state.noise_reduction_3d = value,
            Self::FocusMode(value) => state.focus_mode = value,
            Self::FocusZone(value) => state.focus_zone = value,
            Self::PictureEffect(value) => state.picture_effect = value,
            Self::UsbAudio(value) => state.usb_audio_enabled = value,
            Self::AdjustCompensation(step) => {
                state.exposure_compensation =
                    i8::try_from(step.apply(i32::from(state.exposure_compensation), 0, -7, 7))
                        .expect("adjustment stays in the register range")
            }
            Self::AdjustIris(step) => {
                state.iris_position =
                    u16::try_from(step.apply(i32::from(state.iris_position), 0, 0, 255))
                        .expect("adjustment stays in the register range")
            }
            Self::AdjustShutter(step) => {
                state.shutter_speed =
                    u8::try_from(step.apply(i32::from(state.shutter_speed), 1, 0, 255))
                        .expect("adjustment stays in the register range")
            }
            Self::AdjustBrightness(step) => {
                state.brightness = u8::try_from(step.apply(i32::from(state.brightness), 7, 0, 255))
                    .expect("adjustment stays in the register range")
            }
            Self::AdjustGain(step) => {
                state.gain_level = u8::try_from(step.apply(i32::from(state.gain_level), 0, 0, 255))
                    .expect("adjustment stays in the register range")
            }
            Self::AdjustColorTemperature(step) => {
                state.color_temperature =
                    u16::try_from(step.apply(i32::from(state.color_temperature), 3, 0, 255))
                        .expect("adjustment stays in the register range")
            }
        }
    }
}

/// Standard position and register words are nibble-clean, most significant first.
fn decode_position(nibbles: &[u8]) -> Option<u16> {
    if nibbles.len() > 4 || nibbles.iter().any(|nibble| *nibble > 0x0F) {
        return None;
    }
    Some(
        nibbles
            .iter()
            .fold(0, |value, nibble| (value << 4) | u16::from(*nibble)),
    )
}

struct SimulatorInner {
    state: Mutex<SimulatorState>,
    processing: Mutex<()>,

    // Response management
    response_broadcaster: broadcast::Sender<Vec<u8>>,

    // Configuration
    config: SimulatorConfig,

    // Statistics
    stats: RwLock<SimulatorStats>,

    changed: Arc<Notify>,
    processor: StdMutex<Option<AbortHandle>>,
    random: AtomicU64,
}

/// Inquiry/error replies have the same bounded capacity as the broadcast lane.
const RESPONSE_CAPACITY: usize = 100;

struct ScheduledReply {
    at: Instant,
    frame: Vec<u8>,
}

struct SimulatorState {
    /// State and sockets share one lock, making completion/application atomic.
    cameras: [CameraState; 7],
    socket_states: [Vec<SocketState>; 7],
    replies: VecDeque<ScheduledReply>,
}

impl Drop for SimulatorInner {
    fn drop(&mut self) {
        if let Some(processor) = self.processor.get_mut().expect("processor lock").take() {
            processor.abort();
        }
    }
}

/// Configuration for [`ViscaCameraSimulator`]'s Standard VISCA behavior.
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

    /// Create a simulator with custom configuration.
    ///
    /// This creates no tasks and may be called outside a Tokio runtime.
    pub fn with_config(mut config: SimulatorConfig) -> Self {
        config.max_sockets = config.max_sockets.clamp(1, 2);
        let (tx, rx) = broadcast::channel(RESPONSE_CAPACITY);

        let inner = Arc::new(SimulatorInner {
            state: Mutex::new(SimulatorState {
                cameras: std::array::from_fn(|_| CameraState::default()),
                socket_states: std::array::from_fn(|_| vec![SocketState::Free; config.max_sockets]),
                replies: VecDeque::new(),
            }),
            processing: Mutex::new(()),
            response_broadcaster: tx,
            config,
            stats: RwLock::new(SimulatorStats::default()),
            changed: Arc::new(Notify::new()),
            processor: StdMutex::new(None),
            random: AtomicU64::new(0x517c_c1b7_2722_0a95),
        });

        Self {
            inner,
            receiver: Some(Arc::new(Mutex::new(rx))),
            transport_config: TransportConfig {
                ..TransportConfig::default()
            },
        }
    }

    /// Create a builder for configuring the simulator
    pub fn builder() -> SimulatorBuilder {
        SimulatorBuilder::default()
    }

    /// Starts one processor lazily. Its weak reference never owns the simulator
    /// while waiting, and the final transport drop aborts its idle wait.
    fn start_processor(&self) {
        let mut processor = self.inner.processor.lock().expect("processor lock");
        if processor.is_some() {
            return;
        }
        let weak = Arc::downgrade(&self.inner);
        let task = tokio::spawn(async move {
            loop {
                let Some(inner) = weak.upgrade() else {
                    return;
                };
                let changed = Arc::clone(&inner.changed);
                let notified = changed.notified();
                let deadline = Self::process_responses(&inner).await;
                drop(inner);
                match deadline {
                    Some(deadline) => {
                        tokio::select! {
                            () = sleep_until(deadline) => {},
                            () = notified => {},
                        }
                    }
                    None => notified.await,
                }
            }
        });
        *processor = Some(task.abort_handle());
    }

    /// Process only due events and return the next deadline; no polling timer
    /// or lock is retained while sleeping.
    async fn process_responses(inner: &SimulatorInner) -> Option<Instant> {
        let _processing = inner.processing.lock().await;
        let now = Instant::now();
        let mut due = Vec::new();
        let mut next = None;
        let mut completed = 0;
        let mut completed_effects = Vec::new();
        {
            let mut state = inner.state.lock().await;
            for (camera_idx, sockets) in state.socket_states.iter_mut().enumerate() {
                let camera = u8::try_from(camera_idx + 1).expect("seven camera addresses");
                for (socket_idx, socket) in sockets.iter_mut().enumerate() {
                    if let SocketState::Executing {
                        ack_at,
                        completion_at,
                        ack_pending,
                        effect,
                    } = socket
                    {
                        let number = u8::try_from(socket_idx + 1).expect("two VISCA sockets");
                        if *ack_pending {
                            if now >= *ack_at {
                                due.push((*ack_at, frames::ack_from(camera, number)));
                                *ack_pending = false;
                            } else {
                                keep_earliest(&mut next, *ack_at);
                            }
                        }
                        if now >= *completion_at {
                            if let Some(effect) = effect.take() {
                                completed_effects.push((*completion_at, camera_idx, effect));
                            }
                            due.push((*completion_at, addressed(camera, frames::complete(number))));
                            *socket = SocketState::Free;
                            completed += 1;
                        } else {
                            keep_earliest(&mut next, *completion_at);
                        }
                    }
                }
            }
            // A later completion wins even when it occupied the lower socket.
            completed_effects.sort_by_key(|(at, _, _)| *at);
            for (_, camera_idx, effect) in completed_effects {
                effect.apply(&mut state.cameras[camera_idx]);
            }
            state.replies.retain(|reply| {
                if now >= reply.at {
                    due.push((reply.at, reply.frame.clone()));
                    false
                } else {
                    keep_earliest(&mut next, reply.at);
                    true
                }
            });
        }
        inner.stats.write().await.commands_executed += completed;
        due.sort_by_key(|(at, _)| *at);
        for (_, response) in due {
            if inner.chance(inner.config.packet_loss_rate) {
                inner.stats.write().await.packets_dropped += 1;
            } else {
                let _ = inner.response_broadcaster.send(response);
            }
        }
        next
    }

    /// Parse command type from VISCA command bytes
    fn parse_command_type(data: &[u8]) -> CommandType {
        if data.len() < 3 {
            return CommandType::Other;
        }

        // Command format: 0x8X 0x01 [command bytes] 0xFF
        // OR inquiry format: 0x8X 0x09 [inquiry bytes] 0xFF. PTZOptics UAC
        // USB-audio is the documented parameterless vendor exception.
        if Self::is_usb_audio_inquiry(data) {
            return CommandType::Inquiry;
        }

        match (data[1], data.get(2)) {
            (0x01, Some(0x04)) if data.get(3) == Some(&0x3F) => CommandType::Preset,
            (0x01, Some(0x06)) if matches!(data.get(3), Some(0x01..=0x05)) => CommandType::PanTilt,
            (0x01, Some(0x04)) if matches!(data.get(3), Some(0x07 | 0x47)) => CommandType::Zoom,
            (0x01, Some(0x04)) if matches!(data.get(3), Some(0x08 | 0x48)) => CommandType::Focus,
            (0x01, Some(0x04)) if data.get(3) == Some(&0x00) => CommandType::Power,
            (0x09, _) => CommandType::Inquiry,
            _ => CommandType::Other,
        }
    }

    fn is_usb_audio_inquiry(data: &[u8]) -> bool {
        matches!(
            data,
            [0x81..=0x88, 0x2A, 0x02, 0xA0, 0x04, VISCA_TERMINATOR]
        )
    }

    /// Generate inquiry response based on the inquiry command
    async fn generate_inquiry_response(&self, data: &[u8]) -> Option<Vec<u8>> {
        let usb_audio_inquiry = Self::is_usb_audio_inquiry(data);
        if data.len() < 4 || (data[1] != 0x09 && !usb_audio_inquiry) {
            tracing::debug!("Not an inquiry command: {:02X?}", data);
            return None;
        }

        let camera_index = usize::from(data.first()?.checked_sub(0x81)?);
        let state = self.inner.state.lock().await;
        let state = state.cameras.get(camera_index)?;

        tracing::debug!(
            "Processing inquiry: cmd[2]={:02X?}, cmd[3]={:02X?}, full command: {:02X?}",
            data.get(2),
            data.get(3),
            data
        );

        if usb_audio_inquiry {
            let status = if state.usb_audio_enabled { 0x02 } else { 0x03 };
            return Some(frames::inquiry_reply(&[status]));
        }

        // Parse inquiry type from command bytes.
        match (data.get(2), data.get(3)) {
            // Power inquiry: 0x81 0x09 0x04 0x00 0xFF
            (Some(0x04), Some(0x00)) => {
                let status = if state.power_on { 0x02 } else { 0x03 };
                Some(frames::inquiry_reply(&[status]))
            }

            // Pan/Tilt position inquiry: 0x81 0x09 0x06 0x12 0xFF
            (Some(0x06), Some(0x12)) => {
                let pan_bytes = encode_signed_position(state.pan_position);
                let tilt_bytes = encode_signed_position(state.tilt_position);
                Some(frames::inquiry_reply(&[
                    pan_bytes[0],
                    pan_bytes[1],
                    pan_bytes[2],
                    pan_bytes[3],
                    tilt_bytes[0],
                    tilt_bytes[1],
                    tilt_bytes[2],
                    tilt_bytes[3],
                ]))
            }

            // Zoom position inquiry: 0x81 0x09 0x04 0x47 0xFF
            (Some(0x04), Some(0x47)) => {
                let zoom_bytes = encode_position(state.zoom_position);
                Some(frames::inquiry_reply(&[
                    zoom_bytes[0],
                    zoom_bytes[1],
                    zoom_bytes[2],
                    zoom_bytes[3],
                ]))
            }

            // Focus position inquiry: 0x81 0x09 0x04 0x48 0xFF
            (Some(0x04), Some(0x48)) => {
                let focus_bytes = encode_position(state.focus_position);
                Some(frames::inquiry_reply(&[
                    focus_bytes[0],
                    focus_bytes[1],
                    focus_bytes[2],
                    focus_bytes[3],
                ]))
            }

            // Focus near limit inquiry: 0x81 0x09 0x04 0x28 0xFF
            (Some(0x04), Some(0x28)) => {
                let limit_bytes = encode_position(state.focus_near_limit);
                Some(frames::inquiry_reply(&[
                    limit_bytes[0],
                    limit_bytes[1],
                    limit_bytes[2],
                    limit_bytes[3],
                ]))
            }

            // Exposure mode inquiry: 0x81 0x09 0x04 0x39 0xFF
            (Some(0x04), Some(0x39)) => Some(frames::inquiry_reply(&[state.exposure_mode])),

            // Exposure compensation inquiry: 0x81 0x09 0x04 0x4E 0xFF
            (Some(0x04), Some(0x4E)) => {
                // Exposure compensation value needs to be offset by 7 and encoded as nibbles
                // The parser expects 4 bytes: 0x00, 0x00, nibble1, nibble2
                let adjusted_value = (state.exposure_compensation + 7) as u8;
                let nibble_high = (adjusted_value >> 4) & 0x0F;
                let nibble_low = adjusted_value & 0x0F;
                let response = frames::inquiry_reply(&[0x00, 0x00, nibble_high, nibble_low]);
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
                Some(frames::inquiry_reply(&[status]))
            }

            // Exposure compensation on/off inquiry: 0x81 0x09 0x04 0x3F 0xFF
            (Some(0x04), Some(0x3F)) => {
                let status = if state.exposure_compensation_enabled {
                    0x02
                } else {
                    0x03
                };
                Some(frames::inquiry_reply(&[status]))
            }

            // Iris inquiry: 0x81 0x09 0x04 0x4B 0xFF
            (Some(0x04), Some(0x4B)) => {
                let iris_bytes = encode_position(state.iris_position);
                Some(frames::inquiry_reply(&[
                    iris_bytes[0],
                    iris_bytes[1],
                    iris_bytes[2],
                    iris_bytes[3],
                ]))
            }

            // Shutter inquiry: 0x81 0x09 0x04 0x4A 0xFF
            (Some(0x04), Some(0x4A)) => {
                // Shutter speed needs to be encoded as 4 nibbles
                let shutter_bytes = encode_position(state.shutter_speed as u16);
                Some(frames::inquiry_reply(&[
                    shutter_bytes[0],
                    shutter_bytes[1],
                    shutter_bytes[2],
                    shutter_bytes[3],
                ]))
            }

            // Brightness inquiry: 0x81 0x09 0x04 0x4D 0xFF
            (Some(0x04), Some(0x4D)) => {
                // Brightness needs to be encoded as 4 nibbles
                let bright_bytes = encode_position(state.brightness as u16);
                Some(frames::inquiry_reply(&[
                    bright_bytes[0],
                    bright_bytes[1],
                    bright_bytes[2],
                    bright_bytes[3],
                ]))
            }

            // Gain inquiry: 0x81 0x09 0x04 0x4C 0xFF
            (Some(0x04), Some(0x4C)) => {
                // Gain needs to be encoded as 4 nibbles
                let gain_bytes = encode_position(state.gain_level as u16);
                Some(frames::inquiry_reply(&[
                    gain_bytes[0],
                    gain_bytes[1],
                    gain_bytes[2],
                    gain_bytes[3],
                ]))
            }

            // Gain limit inquiry: 0x81 0x09 0x04 0x2C 0xFF
            (Some(0x04), Some(0x2C)) => Some(frames::inquiry_reply(&[state.gain_limit])),

            // Backlight inquiry: 0x81 0x09 0x04 0x33 0xFF
            (Some(0x04), Some(0x33)) => {
                let status = if state.backlight_enabled { 0x02 } else { 0x03 };
                Some(frames::inquiry_reply(&[status]))
            }

            // White balance mode inquiry: 0x81 0x09 0x04 0x35 0xFF
            (Some(0x04), Some(0x35)) => Some(frames::inquiry_reply(&[state.white_balance_mode])),

            // Color temperature inquiry: 0x81 0x09 0x04 0x20 0xFF
            // G2 cameras return a single raw byte (not 4 nibbles)
            (Some(0x04), Some(0x20)) => {
                let temp_value = state.color_temperature as u8;
                Some(frames::inquiry_reply(&[temp_value]))
            }

            // Contrast inquiry: 0x81 0x09 0x04 0xA2 0xFF
            (Some(0x04), Some(0xA2)) => {
                let contrast_bytes = encode_position(state.contrast as u16);
                Some(frames::inquiry_reply(&[
                    contrast_bytes[0],
                    contrast_bytes[1],
                    contrast_bytes[2],
                    contrast_bytes[3],
                ]))
            }

            // Saturation inquiry: 0x81 0x09 0x04 0x49 0xFF
            (Some(0x04), Some(0x49)) => {
                // Saturation needs to be encoded as 4 nibbles
                let sat_bytes = encode_position(state.saturation as u16);
                Some(frames::inquiry_reply(&[
                    sat_bytes[0],
                    sat_bytes[1],
                    sat_bytes[2],
                    sat_bytes[3],
                ]))
            }

            // Hue inquiry: 0x81 0x09 0x04 0x4F 0xFF
            (Some(0x04), Some(0x4F)) => {
                // Hue needs to be encoded as 4 nibbles, with the value in the last position
                Some(frames::inquiry_reply(&[0x00, 0x00, 0x00, state.hue]))
            }

            // Luminance inquiry: 0x81 0x09 0x04 0xA1 0xFF
            (Some(0x04), Some(0xA1)) => {
                let luminance_bytes = encode_position(state.luminance as u16);
                Some(frames::inquiry_reply(&[
                    luminance_bytes[0],
                    luminance_bytes[1],
                    luminance_bytes[2],
                    luminance_bytes[3],
                ]))
            }

            // Combined flip inquiry (CAM_FlipInq): 0x81 0x09 0x04 0xA4 0xFF
            (Some(0x04), Some(0xA4)) => {
                let flip_mode = match (state.image_flip_vertical, state.image_flip_horizontal) {
                    (false, false) => 0x00, // Off
                    (false, true) => 0x01,  // Horizontal
                    (true, false) => 0x02,  // Vertical
                    (true, true) => 0x03,   // Both
                };
                Some(frames::inquiry_reply(&[flip_mode]))
            }

            // Noise reduction 2D mode inquiry: 0x81 0x09 0x04 0x50 0xFF
            (Some(0x04), Some(0x50)) => {
                Some(frames::inquiry_reply(&[state.noise_reduction_2d_mode]))
            }

            // Noise reduction 2D inquiry: 0x81 0x09 0x04 0x53 0xFF
            (Some(0x04), Some(0x53)) => Some(frames::inquiry_reply(&[state.noise_reduction_2d])),

            // Noise reduction 3D inquiry: 0x81 0x09 0x04 0x54 0xFF
            (Some(0x04), Some(0x54)) => Some(frames::inquiry_reply(&[state.noise_reduction_3d])),

            // Focus mode inquiry: 0x81 0x09 0x04 0x38 0xFF
            (Some(0x04), Some(0x38)) => Some(frames::inquiry_reply(&[state.focus_mode])),

            // Focus zone inquiry: 0x81 0x09 0x04 0xAA 0xFF
            (Some(0x04), Some(0xAA)) => Some(frames::inquiry_reply(&[state.focus_zone])),

            // NOTE: AutoFocus inquiry is not documented in VISCA specs
            // and has been disabled until proper documentation is found.

            // // Auto focus inquiry: 0x81 0x09 0x04 0x18 0xFF
            // (Some(0x04), Some(0x18)) => {
            //     let status = if state.auto_focus_enabled { 0x02 } else { 0x03 };
            //     Some(frames::inquiry_reply(&[status]))
            // }

            // Picture effect inquiry: 0x81 0x09 0x04 0x63 0xFF
            (Some(0x04), Some(0x63)) => Some(frames::inquiry_reply(&[state.picture_effect])),

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

    async fn queue_reply(&self, camera: u8, frame: Vec<u8>, delay: Duration) -> Result<(), Error> {
        let at = response_deadline(Instant::now(), delay.saturating_add(self.inner.jitter()))?;
        let mut state = self.inner.state.lock().await;
        if state.replies.len() == RESPONSE_CAPACITY {
            state.replies.pop_front();
        }
        state.replies.push_back(ScheduledReply {
            at,
            frame: addressed(camera, frame),
        });
        drop(state);
        self.inner.changed.notify_one();
        Ok(())
    }
}

impl SimulatorInner {
    /// SplitMix64 with a fixed initial seed: no clock or platform entropy enters
    /// option behavior. Each configured random decision advances the stream.
    fn random(&self) -> u64 {
        let mut value = self
            .random
            .fetch_add(0x9e37_79b9_7f4a_7c15, Ordering::Relaxed);
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn chance(&self, probability: f32) -> bool {
        if probability <= 0.0 || probability.is_nan() {
            return false;
        }
        if probability >= 1.0 {
            return true;
        }
        let sample = (self.random() >> 40) as f32 / 16_777_216.0;
        sample < probability
    }

    fn jitter(&self) -> Duration {
        let limit = u64::from(self.config.network_jitter_ms);
        if limit == 0 {
            Duration::ZERO
        } else {
            Duration::from_millis(self.random() % (limit + 1))
        }
    }
}

fn addressed(camera: u8, mut frame: Vec<u8>) -> Vec<u8> {
    frame[0] = frames::reply_address(camera);
    frame
}

fn keep_earliest(next: &mut Option<Instant>, at: Instant) {
    *next = Some(next.map_or(at, |previous| previous.min(at)));
}

fn response_deadline(now: Instant, delay: Duration) -> Result<Instant, Error> {
    now.checked_add(delay).ok_or_else(|| {
        Error::InvalidRequest("simulator response delay cannot be represented by the clock".into())
    })
}

impl AsyncTransport for ViscaCameraSimulator {
    async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        self.start_processor();
        self.inner.stats.write().await.commands_received += 1;
        Self::process_responses(&self.inner).await;

        let camera = match data.first() {
            Some(address @ 0x81..=0x87) => address & 0x07,
            _ => {
                return Err(Error::InvalidRequest(
                    "the simulator requires an individual VISCA camera address (1 through 7)"
                        .into(),
                ));
            }
        };
        if data.last() != Some(&VISCA_TERMINATOR) || data.len() < 3 {
            return self
                .queue_reply(camera, frames::syntax_error(), Duration::ZERO)
                .await;
        }

        if let [_, cancel @ 0x20..=0x2f, VISCA_TERMINATOR] = data {
            let number = cancel & 0x0f;
            let mut state = self.inner.state.lock().await;
            let socket = number.checked_sub(1).and_then(|index| {
                state.socket_states[usize::from(camera - 1)].get_mut(usize::from(index))
            });
            let reply = if let Some(socket @ SocketState::Executing { .. }) = socket {
                *socket = SocketState::Free;
                frames::canceled(number)
            } else {
                frames::no_socket(number)
            };
            drop(state);
            return self.queue_reply(camera, reply, Duration::ZERO).await;
        }

        let command_type = Self::parse_command_type(data);
        let duration = self
            .inner
            .config
            .command_execution_times
            .get(&command_type)
            .copied()
            .unwrap_or(Duration::from_millis(50));
        if command_type == CommandType::Inquiry {
            let reply = self
                .generate_inquiry_response(data)
                .await
                .unwrap_or_else(frames::syntax_error);
            return self.queue_reply(camera, reply, duration).await;
        }

        let now = Instant::now();
        let ack_at = response_deadline(
            now,
            self.inner
                .config
                .ack_delay
                .saturating_add(self.inner.jitter()),
        )?;
        let completion_at =
            response_deadline(ack_at, duration.saturating_add(self.inner.jitter()))?;
        let mut state = self.inner.state.lock().await;
        let sockets = &mut state.socket_states[usize::from(camera - 1)];
        let available = sockets
            .iter_mut()
            .find(|socket| matches!(socket, SocketState::Free));
        match (
            self.inner.chance(self.inner.config.busy_probability),
            available,
        ) {
            (false, Some(socket)) => {
                *socket = SocketState::Executing {
                    ack_at,
                    completion_at,
                    ack_pending: true,
                    effect: CommandEffect::parse(data),
                };
                drop(state);
                self.inner.changed.notify_one();
                Ok(())
            }
            (true, _) | (false, None) => {
                drop(state);
                self.inner.stats.write().await.busy_responses_sent += 1;
                self.queue_reply(camera, frames::buffer_full(), Duration::ZERO)
                    .await
            }
        }
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        self.start_processor();
        let receiver = self.receiver.get_or_insert_with(|| {
            Arc::new(Mutex::new(self.inner.response_broadcaster.subscribe()))
        });
        let mut receiver = receiver.lock().await;
        match tokio::time::timeout(Duration::from_secs(30), receiver.recv()).await {
            Ok(Ok(response)) => Ok(ReceiveOutcome::copy_message(&response, dst)),
            Ok(Err(broadcast::error::RecvError::Closed)) => Err(Error::ConnectionClosed {
                source: Some(Arc::new(Error::from(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "simulator response lane closed",
                )))),
            }),
            Ok(Err(broadcast::error::RecvError::Lagged(_))) | Err(_) => Err(Error::io_timeout()),
        }
    }
}

impl HasTransportConfig for ViscaCameraSimulator {
    fn identity(&self) -> crate::transport::TransportIdentity<'_> {
        crate::transport::TransportIdentity::Custom {
            semantics: crate::transport::SendSemantics::Datagram,
            addressing: crate::transport::AddressingMode::Serial,
        }
    }

    fn transport_config(&self) -> &TransportConfig {
        &self.transport_config
    }
}

/// Builder for configuring [`ViscaCameraSimulator`]'s Standard VISCA behavior.
#[derive(Default, Debug)]
pub struct SimulatorBuilder {
    config: SimulatorConfig,
}

impl SimulatorBuilder {
    /// Set the number of sockets per camera (default: 2), clamped to `1..=2`.
    pub fn with_socket_count(mut self, count: usize) -> Self {
        self.config.max_sockets = count.clamp(1, 2);
        self
    }

    /// Set the delay before an ACK becomes available (default: 5 ms).
    ///
    /// Command execution starts after this delay, so completion never overtakes ACK.
    pub fn with_ack_delay(mut self, delay: Duration) -> Self {
        self.config.ack_delay = delay;
        self
    }

    /// Set the time from ACK to completion for a command type.
    ///
    /// For inquiries this is the delay before their single data/error reply.
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

    /// Set the fraction of reply frames omitted, clamped to `0.0..=1.0`.
    ///
    /// A lost reply does not fail `send` or prevent command execution.
    /// The fixed-seed decision sequence is reproducible; NaN disables loss.
    pub fn with_packet_loss(mut self, rate: f32) -> Self {
        self.config.packet_loss_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Set the probability a command is refused as Buffer Full.
    ///
    /// Values are clamped to `0.0..=1.0`; NaN disables random refusals.
    /// Inquiries and socket cancellation remain available while busy.
    pub fn with_busy_probability(mut self, prob: f32) -> Self {
        self.config.busy_probability = prob.clamp(0.0, 1.0);
        self
    }

    /// Set maximum added reply latency in milliseconds (inclusive).
    ///
    /// Each scheduled reply receives deterministic jitter once. Cancelling and
    /// resuming a receive does not restart its delay.
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
    encode_position(u16::from_be_bytes(value.to_be_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn reply(simulator: &mut ViscaCameraSimulator) -> Vec<u8> {
        let mut bytes = [0; 32];
        let count = simulator
            .recv_into(&mut bytes)
            .await
            .expect("simulator reply")
            .copied_len();
        bytes[..count].to_vec()
    }

    async fn complete(simulator: &mut ViscaCameraSimulator, command: &[u8]) {
        simulator.send(command).await.expect("send command");
        assert_eq!(reply(simulator).await[1] & 0xF0, 0x40, "ACK");
        assert_eq!(reply(simulator).await[1] & 0xF0, 0x50, "completion");
    }

    async fn inquire(simulator: &mut ViscaCameraSimulator, command: &[u8]) -> Vec<u8> {
        simulator.send(command).await.expect("send inquiry");
        reply(simulator).await
    }

    #[allow(clippy::panic)]
    fn decoded_position(bytes: &[u8]) -> (i32, i32) {
        use crate::command::{InquiryData, InquiryKind, Response};
        let decoded = Response::parse_with_type(bytes, &InquiryKind::PanTiltPosition)
            .expect("the production decoder accepts simulator positions");
        match decoded {
            Response::Inquiry(InquiryData::PanTiltPosition { pan, tilt }) => (pan, tilt),
            other => panic!("expected position, got {other:?}"),
        }
    }

    #[test]
    fn every_signed_position_round_trips_through_the_production_decoder() {
        for position in i16::MIN..=i16::MAX {
            let encoded = encode_signed_position(position);
            let payload = [encoded, encoded].concat();
            let response = frames::inquiry_reply(&payload);
            assert_eq!(
                decoded_position(&response),
                (i32::from(position), i32::from(position))
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn power_inquiries_follow_completed_commands() {
        let mut simulator = ViscaCameraSimulator::new();
        for power in [0x03, 0x02] {
            complete(
                &mut simulator,
                &[0x81, 0x01, 0x04, 0x00, power, VISCA_TERMINATOR],
            )
            .await;
            let response =
                inquire(&mut simulator, &[0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR]).await;
            assert_eq!(response, [0x90, 0x50, power, VISCA_TERMINATOR]);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn absolute_and_home_positions_follow_completion_but_cancelled_moves_do_not() {
        let mut simulator = ViscaCameraSimulator::new();
        // Standard two's-complement targets: pan -1000, tilt -200.
        let absolute = [
            0x81,
            0x01,
            0x06,
            0x02,
            1,
            1,
            0x0F,
            0x0C,
            1,
            8,
            0x0F,
            0x0F,
            3,
            8,
            VISCA_TERMINATOR,
        ];
        let inquiry = [0x81, 0x09, 0x06, 0x12, VISCA_TERMINATOR];
        simulator.send(&absolute).await.expect("send move");
        assert_eq!(reply(&mut simulator).await, frames::ack(1));
        assert_eq!(
            decoded_position(
                &simulator
                    .generate_inquiry_response(&inquiry)
                    .await
                    .expect("position inquiry")
            ),
            (0, 0),
            "ACK has no applied effect"
        );
        simulator
            .send(&[0x81, 0x21, VISCA_TERMINATOR])
            .await
            .expect("cancel move");
        assert_eq!(reply(&mut simulator).await, frames::canceled(1));
        tokio::time::advance(Duration::from_secs(60)).await;
        assert_eq!(
            decoded_position(&inquire(&mut simulator, &inquiry).await),
            (0, 0)
        );

        complete(&mut simulator, &absolute).await;
        assert_eq!(
            decoded_position(&inquire(&mut simulator, &inquiry).await),
            (-1000, -200)
        );
        complete(&mut simulator, &[0x81, 0x01, 0x06, 0x04, VISCA_TERMINATOR]).await;
        assert_eq!(
            decoded_position(&inquire(&mut simulator, &inquiry).await),
            (0, 0)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn addressed_cameras_keep_independent_power_and_positions() {
        let mut simulator = ViscaCameraSimulator::new();
        complete(
            &mut simulator,
            &[0x82, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        )
        .await;
        complete(
            &mut simulator,
            &[
                0x82,
                0x01,
                0x06,
                0x02,
                1,
                1,
                0x0F,
                0x0C,
                1,
                8,
                0,
                0,
                0,
                5,
                VISCA_TERMINATOR,
            ],
        )
        .await;
        assert_eq!(
            inquire(&mut simulator, &[0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR]).await,
            [0x90, 0x50, 0x02, VISCA_TERMINATOR]
        );
        assert_eq!(
            inquire(&mut simulator, &[0x82, 0x09, 0x04, 0x00, VISCA_TERMINATOR]).await,
            [0xA0, 0x50, 0x03, VISCA_TERMINATOR]
        );
        assert_eq!(
            decoded_position(
                &inquire(&mut simulator, &[0x81, 0x09, 0x06, 0x12, VISCA_TERMINATOR]).await
            ),
            (0, 0)
        );
        assert_eq!(
            decoded_position(
                &inquire(&mut simulator, &[0x82, 0x09, 0x06, 0x12, VISCA_TERMINATOR]).await
            ),
            (-1000, 5)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn dropped_completion_replies_still_apply_commands() {
        let mut simulator = ViscaCameraSimulator::builder()
            .with_packet_loss(1.0)
            .build();
        simulator
            .send(&[0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR])
            .await
            .expect("command accepted");
        tokio::time::advance(Duration::from_secs(2)).await;
        ViscaCameraSimulator::process_responses(&simulator.inner).await;
        assert_eq!(
            simulator
                .generate_inquiry_response(&[0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR])
                .await
                .expect("power inquiry"),
            [0x90, 0x50, 0x03, VISCA_TERMINATOR]
        );
    }

    #[tokio::test(start_paused = true)]
    #[allow(clippy::panic)]
    async fn simultaneous_due_effects_follow_completion_order_across_sockets() {
        let mut simulator = ViscaCameraSimulator::builder()
            .with_command_execution_time(CommandType::Power, Duration::from_secs(1))
            .with_command_execution_time(CommandType::Other, Duration::from_millis(50))
            .build();
        // Socket 1 completes after socket 2: its late off must win.
        simulator
            .send(&[0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR])
            .await
            .expect("command accepted");
        let mut state = simulator.inner.state.lock().await;
        let SocketState::Executing { completion_at, .. } = state.socket_states[0][0].clone() else {
            panic!("executing")
        };
        state.socket_states[0][1] = SocketState::Executing {
            ack_at: Instant::now(),
            completion_at: completion_at - Duration::from_millis(10),
            ack_pending: false,
            effect: Some(CommandEffect::Power(true)),
        };
        drop(state);
        tokio::time::advance(Duration::from_secs(2)).await;
        ViscaCameraSimulator::process_responses(&simulator.inner).await;
        assert!(!simulator.inner.state.lock().await.cameras[0].power_on);
    }

    #[tokio::test(start_paused = true)]
    async fn modeled_register_setters_are_reported_by_their_inquiries() {
        let mut simulator = ViscaCameraSimulator::new();
        let cases: &[(u8, &[u8], &[u8])] = &[
            (0x47, &[1, 2, 3, 4], &[1, 2, 3, 4]),
            (0x48, &[2, 3, 4, 5], &[2, 3, 4, 5]),
            (0x28, &[0, 1, 2, 3], &[0, 1, 2, 3]),
            (0x39, &[3], &[3]),
            (0x4E, &[0, 0, 0, 12], &[0, 0, 0, 12]),
            (0x3E, &[2], &[2]),
            (0x4B, &[0, 0, 0, 9], &[0, 0, 0, 9]),
            (0x4A, &[0, 0, 1, 0], &[0, 0, 1, 0]),
            (0x4D, &[0, 0, 0, 8], &[0, 0, 0, 8]),
            (0x4C, &[0, 0, 0, 6], &[0, 0, 0, 6]),
            (0x2C, &[5], &[5]),
            (0x33, &[2], &[2]),
            (0x35, &[4], &[4]),
            (0x20, &[1, 2], &[0x12]),
            (0xA2, &[0, 0, 0, 5], &[0, 0, 0, 5]),
            (0xA1, &[0, 0, 0, 4], &[0, 0, 0, 4]),
            (0x49, &[0, 0, 0, 3], &[0, 0, 0, 3]),
            (0x4F, &[0, 0, 0, 2], &[0, 0, 0, 2]),
            (0xA4, &[3], &[3]),
            (0x50, &[3], &[3]),
            (0x53, &[4], &[4]),
            (0x54, &[5], &[5]),
            (0x38, &[3], &[3]),
            (0xAA, &[2], &[2]),
            (0x63, &[1], &[1]),
        ];
        for (register, value, expected) in cases {
            let mut command = vec![0x81, 0x01, 0x04, *register];
            command.extend_from_slice(value);
            command.push(VISCA_TERMINATOR);
            complete(&mut simulator, &command).await;
            let response = inquire(
                &mut simulator,
                &[0x81, 0x09, 0x04, *register, VISCA_TERMINATOR],
            )
            .await;
            assert_eq!(
                response,
                frames::inquiry_reply(expected),
                "register {register:02X}"
            );
        }
        complete(
            &mut simulator,
            &[0x81, 0x2A, 0x02, 0xA0, 0x04, 0x03, VISCA_TERMINATOR],
        )
        .await;
        assert_eq!(
            inquire(
                &mut simulator,
                &[0x81, 0x2A, 0x02, 0xA0, 0x04, VISCA_TERMINATOR]
            )
            .await,
            frames::inquiry_reply(&[0x03])
        );
    }

    #[test]
    fn construction_needs_no_runtime_and_socket_counts_are_bounded() {
        for (requested, expected) in [(0, 1), (1, 1), (2, 2), (256, 2), (usize::MAX, 2)] {
            let simulator = ViscaCameraSimulator::builder()
                .with_socket_count(requested)
                .build();
            assert!(simulator
                .inner
                .processor
                .lock()
                .expect("processor lock")
                .is_none());
            assert!(simulator
                .inner
                .state
                .blocking_lock()
                .socket_states
                .iter()
                .all(|sockets| sockets.len() == expected));
        }
    }

    #[tokio::test(start_paused = true)]
    async fn final_clone_drop_stops_the_response_processor() {
        let mut simulator = ViscaCameraSimulator::new();
        let clone = simulator.clone();
        let weak = Arc::downgrade(&simulator.inner);
        simulator
            .send(&[0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR])
            .await
            .expect("inquiry sent");
        tokio::task::yield_now().await;
        let processor = simulator
            .inner
            .processor
            .lock()
            .expect("processor lock")
            .clone()
            .expect("processor started");
        drop(simulator);
        assert!(
            weak.upgrade().is_some(),
            "one clone still owns the simulator"
        );
        assert!(!processor.is_finished());
        drop(clone);
        tokio::task::yield_now().await;
        assert!(
            weak.upgrade().is_none(),
            "the task owns no simulator reference"
        );
        assert!(processor.is_finished());
    }

    #[tokio::test(start_paused = true)]
    async fn idle_cancel_names_its_socket_and_camera_without_allocating_one() {
        let mut simulator = ViscaCameraSimulator::new();
        simulator
            .send(&[0x83, 0x22, VISCA_TERMINATOR])
            .await
            .expect("cancel sent");
        let mut response = [0; 16];
        let count = simulator
            .recv_into(&mut response)
            .await
            .expect("No Socket reply")
            .copied_len();
        assert_eq!(&response[..count], &[0xb0, 0x62, 0x05, 0xff]);
        assert!(simulator
            .inner
            .state
            .lock()
            .await
            .socket_states
            .iter()
            .flatten()
            .all(|socket| matches!(socket, SocketState::Free)));
    }

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

        // PTZOptics UAC USB-audio inquiry is parameterless and does not use
        // the standard 0x09 inquiry byte.
        let usb_audio_inquiry = vec![0x81, 0x2A, 0x02, 0xA0, 0x04, VISCA_TERMINATOR];
        assert_eq!(
            ViscaCameraSimulator::parse_command_type(&usb_audio_inquiry),
            CommandType::Inquiry
        );

        let wrong_destination = vec![0x90, 0x2A, 0x02, 0xA0, 0x04, VISCA_TERMINATOR];
        assert_eq!(
            ViscaCameraSimulator::parse_command_type(&wrong_destination),
            CommandType::Other
        );

        // The UAC control command has the same prefix but carries a value;
        // only the exact parameterless inquiry frame is an inquiry.
        let usb_audio_control = vec![0x81, 0x2A, 0x02, 0xA0, 0x04, 0x02, VISCA_TERMINATOR];
        assert_eq!(
            ViscaCameraSimulator::parse_command_type(&usb_audio_control),
            CommandType::Other
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
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should eventually receive completion
        let response = rx.recv().await.expect("should receive completion");
        assert_eq!(response[0], 0x90);
        assert_eq!(response[1] & 0xF0, 0x50); // Completion response
    }

    #[tokio::test]
    async fn busy_simulator_refuses_with_the_one_buffer_full_frame() {
        let mut simulator = ViscaCameraSimulator::builder()
            .with_socket_count(1)
            .with_command_execution_time(CommandType::Zoom, Duration::from_secs(60))
            .build();
        let mut rx = simulator.inner.response_broadcaster.subscribe();
        let zoom = [0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR];

        simulator.send(&zoom).await.expect("first command is sent");
        assert_eq!(rx.recv().await.expect("ACK"), frames::ack(1));
        simulator.send(&zoom).await.expect("second command is sent");
        assert_eq!(
            rx.recv().await.expect("Buffer Full"),
            frames::buffer_full(),
            "the simulator emits the socketless Buffer Full frame"
        );
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

    #[tokio::test]
    async fn picture_effect_inquiry_uses_04_63_and_returns_its_own_value() {
        let simulator = ViscaCameraSimulator::new();

        let response = simulator
            .generate_inquiry_response(&[0x81, 0x09, 0x04, 0x63, VISCA_TERMINATOR])
            .await;

        assert_eq!(response, Some(vec![0x90, 0x50, 0x02, VISCA_TERMINATOR]));
    }

    #[tokio::test]
    async fn focus_zone_and_usb_audio_inquiries_use_documented_wire_forms() {
        let simulator = ViscaCameraSimulator::new();

        assert_eq!(
            simulator
                .generate_inquiry_response(&[0x81, 0x09, 0x04, 0xAA, VISCA_TERMINATOR])
                .await,
            Some(vec![0x90, 0x50, 0x01, VISCA_TERMINATOR])
        );
        assert_eq!(
            simulator
                .generate_inquiry_response(&[0x81, 0x2A, 0x02, 0xA0, 0x04, VISCA_TERMINATOR])
                .await,
            Some(vec![0x90, 0x50, 0x02, VISCA_TERMINATOR])
        );

        simulator.inner.state.lock().await.cameras[0].usb_audio_enabled = false;
        assert_eq!(
            simulator
                .generate_inquiry_response(&[0x81, 0x2A, 0x02, 0xA0, 0x04, VISCA_TERMINATOR])
                .await,
            Some(vec![0x90, 0x50, 0x03, VISCA_TERMINATOR])
        );
    }
}
