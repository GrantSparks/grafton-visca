//! Streaming-output and USB-audio requests.

use crate::command::nouns::{forward_inquiry, forward_request};
use crate::types::NdiQuality;

/// Reads whether USB audio is enabled.
#[derive(Debug, Clone, Copy)]
pub struct UsbAudioInquiry;
forward_inquiry!(UsbAudioInquiry, crate::command::UsbAudioInquiry; crate::command::UsbAudioInquiry);

/// Enables or disables multicast streaming.
#[derive(Debug, Clone, Copy)]
pub struct SetMulticast {
    inner: crate::command::MulticastStreamingCommand,
}

impl SetMulticast {
    /// Creates a multicast-streaming setting.
    #[must_use]
    pub const fn new(enabled: bool) -> Self {
        Self {
            inner: crate::command::MulticastStreamingCommand::new(enabled),
        }
    }

    /// Returns the requested multicast state.
    #[must_use]
    pub const fn enabled(self) -> bool {
        self.inner.enabled()
    }
}
forward_request!(
    SetMulticast,
    crate::command::MulticastStreamingCommand,
    Plain,
    rows = [MulticastStreamingOn, MulticastStreamingOff]
);

/// Selects NDI streaming quality and waits for protocol completion.
#[derive(Debug, Clone, Copy)]
pub struct SetNdiQuality {
    inner: crate::command::SetNdiQuality,
}

impl SetNdiQuality {
    /// Creates an NDI-quality setting.
    #[must_use]
    pub fn new(quality: NdiQuality) -> Self {
        Self {
            inner: crate::command::SetNdiQuality::new(quality),
        }
    }

    /// Returns the requested quality setting.
    #[must_use]
    pub const fn quality(self) -> NdiQuality {
        self.inner.quality
    }
}
forward_request!(
    SetNdiQuality,
    crate::command::SetNdiQuality,
    Plain,
    rows = [NdiQuality]
);

/// Enables or disables USB audio and waits for protocol completion.
#[derive(Debug, Clone, Copy)]
pub struct SetUsbAudio {
    inner: crate::command::UsbAudioCommand,
}

impl SetUsbAudio {
    /// Creates a USB-audio setting.
    #[must_use]
    pub const fn new(enabled: bool) -> Self {
        Self {
            inner: crate::command::UsbAudioCommand::new(enabled),
        }
    }

    /// Returns the requested USB-audio state.
    #[must_use]
    pub const fn enabled(self) -> bool {
        self.inner.enabled()
    }
}
forward_request!(
    SetUsbAudio,
    crate::command::UsbAudioCommand,
    Plain,
    rows = [UsbAudioOn, UsbAudioOff]
);
