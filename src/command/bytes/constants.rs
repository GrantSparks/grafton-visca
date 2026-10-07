//! The VISCA command byte catalogue.
//!
//! Every built-in command encoder reads its bytes from here, so each command
//! byte sequence is written exactly once. Each constant is an address-free
//! *body*: [`FrameWriter`](super::FrameWriter) supplies the camera address
//! byte and the terminator. A `*_STEP` prefix is followed by a
//! [`Step`](super::Step) byte, a prefix named for a direct value by that
//! value's encoding, and any other prefix by the parameter its encoder
//! documents.
//!
//! A register that both a command family and an inquiry use is declared once,
//! with [`shared_register!`], which derives the command prefix (`01 r r …`)
//! and the inquiry body (`09 r r`) from the same two bytes; the built-in
//! inquiry table reads those `*_INQUIRY` bodies. Other shared bytes (two
//! command families on one opcode, the tally and USB-audio registers) are
//! assembled from one declaration with [`concat`]. Inquiries whose register no
//! command uses keep their bodies in the inquiry table
//! (`crate::command::inquiry_structs`).

use super::concat;

/// The VISCA command category byte.
pub const COMMAND: u8 = 0x01;

/// The VISCA inquiry category byte.
pub const INQUIRY: u8 = 0x09;

/// Declares one register shared by a command family and its inquiry.
///
/// `COMMAND_NAME [suffix…], INQUIRY_NAME = [r0, r1];` declares the command
/// prefix `01 r0 r1 suffix…` and the inquiry body `09 r0 r1`, writing the
/// register bytes once.
macro_rules! shared_register {
    (
        $(#[$command_meta:meta])*
        $command:ident $([$($suffix:literal),+])?,
        $(#[$inquiry_meta:meta])*
        $inquiry:ident = [$r0:literal, $r1:literal];
    ) => {
        $(#[$command_meta])*
        pub const $command: [u8; 3 $($(+ shared_register!(@one $suffix))+)?] =
            [super::COMMAND, $r0, $r1 $($(, $suffix)+)?];

        $(#[$inquiry_meta])*
        pub const $inquiry: [u8; 3] = [super::INQUIRY, $r0, $r1];
    };
    (@one $byte:literal) => {
        1
    };
}

/// Power commands.
pub mod power {
    use super::concat;

    shared_register! {
        /// Power state prefix.
        STATE,
        /// Power state inquiry.
        STATE_INQUIRY = [0x04, 0x00];
    }

    /// Power on.
    pub const ON: [u8; 4] = concat(&[&STATE, &[0x02]]);

    /// Power off (standby).
    pub const STANDBY: [u8; 4] = concat(&[&STATE, &[0x03]]);
}

/// Pan/tilt commands.
pub mod pan_tilt {
    use super::COMMAND;

    /// Drive prefix, followed by pan speed, tilt speed and a direction pair.
    pub const DRIVE: [u8; 3] = [COMMAND, 0x06, 0x01];

    /// Absolute position prefix.
    pub const ABSOLUTE: [u8; 3] = [COMMAND, 0x06, 0x02];

    /// Relative position prefix.
    pub const RELATIVE: [u8; 3] = [COMMAND, 0x06, 0x03];

    /// Home.
    pub const HOME: [u8; 3] = [COMMAND, 0x06, 0x04];

    /// Reset.
    pub const RESET: [u8; 3] = [COMMAND, 0x06, 0x05];

    /// Limit set prefix, followed by the corner and the position.
    pub const LIMIT_SET: [u8; 4] = [COMMAND, 0x06, 0x07, 0x00];

    /// Limit clear prefix, followed by the corner and the cleared position.
    pub const LIMIT_CLEAR: [u8; 4] = [COMMAND, 0x06, 0x07, 0x01];
}

/// Zoom commands.
pub mod zoom {
    use super::COMMAND;

    /// Drive prefix: stop, tele and wide steps or variable-speed drive bytes.
    pub const DRIVE: [u8; 3] = [COMMAND, 0x04, 0x07];

    shared_register! {
        /// Direct position prefix, followed by four position nibbles.
        DIRECT,
        /// Zoom position inquiry.
        POSITION_INQUIRY = [0x04, 0x47];
    }

    /// Digital zoom on (`02`) / off (`03`) prefix.
    pub const DIGITAL: [u8; 3] = [COMMAND, 0x04, 0x06];
}

/// Preset commands.
pub mod preset {
    use super::COMMAND;

    /// Memory prefix, followed by the action and the preset number.
    pub const MEMORY: [u8; 3] = [COMMAND, 0x04, 0x3F];

    /// Preset recall speed prefix, followed by one speed byte.
    ///
    /// PTZOptics overloads the pan/tilt drive opcode with this shorter
    /// one-parameter form, so the bytes are the drive prefix's.
    pub const RECALL_SPEED: [u8; 3] = super::pan_tilt::DRIVE;
}

/// Focus commands.
pub mod focus {
    use super::COMMAND;

    /// Drive prefix: stop, far and near steps or variable-speed drive bytes.
    pub const DRIVE: [u8; 3] = [COMMAND, 0x04, 0x08];

    shared_register! {
        /// Direct position prefix, followed by four position nibbles.
        DIRECT,
        /// Focus position inquiry.
        POSITION_INQUIRY = [0x04, 0x48];
    }

    shared_register! {
        /// Mode prefix: auto, manual, snap or toggle.
        MODE,
        /// Focus mode inquiry.
        MODE_INQUIRY = [0x04, 0x38];
    }

    /// One-push trigger (`01`) / infinity (`02`) prefix.
    pub const ONE_PUSH: [u8; 3] = [COMMAND, 0x04, 0x18];

    /// PTZOptics focus lock on (`02`) / off (`03`) prefix (vendor category
    /// `0A`).
    pub const LOCK: [u8; 3] = [0x0A, 0x04, 0x68];

    /// Sony FR7 push AF press (`01`) / release (`00`) prefix.
    pub const PUSH_AF: [u8; 4] = [COMMAND, 0x7E, 0x04, 0x58];

    shared_register! {
        /// AF zone prefix, followed by the zone byte.
        ZONE,
        /// AF zone inquiry.
        ZONE_INQUIRY = [0x04, 0xAA];
    }

    shared_register! {
        /// AF sensitivity prefix, followed by the sensitivity byte.
        AF_SENSITIVITY,
        /// AF sensitivity inquiry.
        AF_SENSITIVITY_INQUIRY = [0x04, 0x58];
    }

    shared_register! {
        /// Near limit prefix, followed by four position nibbles.
        NEAR_LIMIT,
        /// Focus near limit inquiry.
        NEAR_LIMIT_INQUIRY = [0x04, 0x28];
    }
}

/// Exposure commands.
pub mod exposure {
    use super::{concat, COMMAND};

    shared_register! {
        /// AE mode prefix, followed by the mode byte.
        MODE,
        /// AE mode inquiry.
        MODE_INQUIRY = [0x04, 0x39];
    }

    shared_register! {
        /// Exposure compensation on (`02`) / off (`03`) prefix.
        COMPENSATION_SWITCH,
        /// Exposure compensation on/off inquiry.
        COMPENSATION_SWITCH_INQUIRY = [0x04, 0x3E];
    }

    /// Exposure compensation step prefix.
    pub const COMPENSATION_STEP: [u8; 3] = [COMMAND, 0x04, 0x0E];

    shared_register! {
        /// Exposure compensation direct prefix, followed by `0p 0q`.
        COMPENSATION_DIRECT [0x00, 0x00],
        /// Exposure compensation position inquiry.
        COMPENSATION_INQUIRY = [0x04, 0x4E];
    }

    /// Iris step prefix.
    pub const IRIS_STEP: [u8; 3] = [COMMAND, 0x04, 0x0B];

    shared_register! {
        /// Iris direct prefix, followed by `0p 0q`.
        IRIS_DIRECT [0x00, 0x00],
        /// Iris position inquiry.
        IRIS_INQUIRY = [0x04, 0x4B];
    }

    /// Shutter step prefix.
    pub const SHUTTER_STEP: [u8; 3] = [COMMAND, 0x04, 0x0A];

    shared_register! {
        /// Shutter direct prefix, followed by `0p 0q`.
        SHUTTER_DIRECT [0x00, 0x00],
        /// Shutter position inquiry.
        SHUTTER_INQUIRY = [0x04, 0x4A];
    }

    /// Bright step prefix.
    pub const BRIGHT_STEP: [u8; 3] = [COMMAND, 0x04, 0x0D];

    shared_register! {
        /// Bright direct prefix (`04 4D`, see the reference's Bright Direct
        /// erratum), followed by `0p 0q`.
        BRIGHT_DIRECT [0x00, 0x00],
        /// Bright position inquiry.
        BRIGHT_INQUIRY = [0x04, 0x4D];
    }

    /// PTZOptics anti-flicker prefix, followed by the mode byte.
    pub const ANTI_FLICKER: [u8; 3] = [COMMAND, 0x04, 0x23];

    shared_register! {
        /// PTZOptics dynamic range control prefix, followed by `0p`.
        DYNAMIC_RANGE [0x00, 0x00, 0x00],
        /// Dynamic range inquiry.
        DYNAMIC_RANGE_INQUIRY = [0x04, 0x25];
    }

    const SPOTLIGHT: [u8; 3] = [COMMAND, 0x04, 0x3A];

    /// Spotlight on.
    pub const SPOTLIGHT_ON: [u8; 4] = concat(&[&SPOTLIGHT, &[0x02]]);

    /// Spotlight off.
    pub const SPOTLIGHT_OFF: [u8; 4] = concat(&[&SPOTLIGHT, &[0x03]]);

    const AUTO_SLOW_SHUTTER: [u8; 3] = [COMMAND, 0x04, 0x5A];

    /// Automatic slow shutter on.
    pub const AUTO_SLOW_SHUTTER_ON: [u8; 4] = concat(&[&AUTO_SLOW_SHUTTER, &[0x02]]);

    /// Automatic slow shutter off.
    pub const AUTO_SLOW_SHUTTER_OFF: [u8; 4] = concat(&[&AUTO_SLOW_SHUTTER, &[0x03]]);
}

/// Gain commands.
pub mod gain {
    use super::COMMAND;

    /// Gain step prefix.
    pub const STEP: [u8; 3] = [COMMAND, 0x04, 0x0C];

    shared_register! {
        /// Gain direct prefix, followed by `0p 0q`.
        DIRECT [0x00, 0x00],
        /// Gain position inquiry.
        INQUIRY = [0x04, 0x4C];
    }

    shared_register! {
        /// Gain limit prefix, followed by `0p`.
        LIMIT,
        /// Gain limit inquiry.
        LIMIT_INQUIRY = [0x04, 0x2C];
    }
}

/// White balance commands.
pub mod white_balance {
    use super::COMMAND;

    shared_register! {
        /// White balance mode prefix, followed by the mode byte.
        MODE,
        /// White balance mode inquiry.
        MODE_INQUIRY = [0x04, 0x35];
    }

    shared_register! {
        /// PTZOptics AWB sensitivity prefix, followed by the sensitivity byte.
        AWB_SENSITIVITY,
        /// AWB sensitivity inquiry (PTZOptics also reads it as tally auto-adjust).
        AWB_SENSITIVITY_INQUIRY = [0x04, 0xA9];
    }

    /// One-push white balance trigger.
    pub const ONE_PUSH_TRIGGER: [u8; 4] = [COMMAND, 0x04, 0x10, 0x05];
}

/// Color commands.
pub mod color {
    use super::{concat, COMMAND};

    /// Red gain step prefix.
    pub const RED_GAIN_STEP: [u8; 3] = [COMMAND, 0x04, 0x03];

    shared_register! {
        /// Red gain direct prefix, followed by `0p 0q`.
        RED_GAIN_DIRECT [0x00, 0x00],
        /// Red gain inquiry (also read as red tuning).
        RED_GAIN_INQUIRY = [0x04, 0x43];
    }

    /// Red tuning prefix on the red gain opcode, followed by the whole tuning
    /// code in one byte (the reference's color-tuning erratum).
    pub const RED_TUNING_DIRECT: [u8; 6] = concat(&[&RED_GAIN_DIRECT, &[0x00]]);

    /// Blue gain step prefix.
    pub const BLUE_GAIN_STEP: [u8; 3] = [COMMAND, 0x04, 0x04];

    shared_register! {
        /// Blue gain direct prefix, followed by `0p 0q`.
        BLUE_GAIN_DIRECT [0x00, 0x00],
        /// Blue gain inquiry (also read as blue tuning).
        BLUE_GAIN_INQUIRY = [0x04, 0x44];
    }

    /// Blue tuning prefix on the blue gain opcode, followed by the whole
    /// tuning code in one byte.
    pub const BLUE_TUNING_DIRECT: [u8; 6] = concat(&[&BLUE_GAIN_DIRECT, &[0x00]]);

    shared_register! {
        /// Color temperature register: steps and the direct `0p 0q` value share
        /// it.
        TEMPERATURE,
        /// Color temperature inquiry.
        TEMPERATURE_INQUIRY = [0x04, 0x20];
    }

    shared_register! {
        /// Saturation prefix, followed by `0p`.
        SATURATION [0x00, 0x00, 0x00],
        /// Saturation inquiry.
        SATURATION_INQUIRY = [0x04, 0x49];
    }

    shared_register! {
        /// Hue prefix, followed by `0p`.
        HUE [0x00, 0x00, 0x00],
        /// Hue inquiry.
        HUE_INQUIRY = [0x04, 0x4F];
    }
}

/// Image processing commands.
pub mod image {
    use super::COMMAND;

    shared_register! {
        /// Sharpness mode prefix, followed by the mode byte.
        SHARPNESS_MODE,
        /// Sharpness mode inquiry.
        SHARPNESS_MODE_INQUIRY = [0x04, 0x05];
    }

    /// Sharpness step prefix.
    pub const SHARPNESS_STEP: [u8; 3] = [COMMAND, 0x04, 0x02];

    shared_register! {
        /// Sharpness direct prefix, followed by `0p 0q`.
        SHARPNESS_DIRECT [0x00, 0x00],
        /// Sharpness position inquiry.
        SHARPNESS_INQUIRY = [0x04, 0x42];
    }

    shared_register! {
        /// Luminance direct prefix, followed by `0p 0q`.
        LUMINANCE [0x00, 0x00],
        /// Luminance inquiry.
        LUMINANCE_INQUIRY = [0x04, 0xA1];
    }

    shared_register! {
        /// Contrast direct prefix, followed by `0p 0q`.
        CONTRAST [0x00, 0x00],
        /// Contrast inquiry.
        CONTRAST_INQUIRY = [0x04, 0xA2];
    }

    shared_register! {
        /// Gamma prefix, followed by `0p`.
        GAMMA,
        /// Gamma inquiry.
        GAMMA_INQUIRY = [0x04, 0x5B];
    }

    shared_register! {
        /// Backlight compensation on (`02`) / off (`03`) prefix.
        BACKLIGHT,
        /// Backlight inquiry.
        BACKLIGHT_INQUIRY = [0x04, 0x33];
    }

    shared_register! {
        /// 2D noise-reduction mode prefix, followed by the mode byte.
        NOISE_REDUCTION_2D_MODE,
        /// 2D noise-reduction mode inquiry.
        NOISE_REDUCTION_2D_MODE_INQUIRY = [0x04, 0x50];
    }

    shared_register! {
        /// 2D noise-reduction level prefix, followed by `0p`.
        NOISE_REDUCTION_2D,
        /// 2D noise-reduction level inquiry.
        NOISE_REDUCTION_2D_INQUIRY = [0x04, 0x53];
    }

    shared_register! {
        /// 3D noise-reduction level prefix, followed by `0p`.
        NOISE_REDUCTION_3D,
        /// 3D noise-reduction level inquiry.
        NOISE_REDUCTION_3D_INQUIRY = [0x04, 0x54];
    }

    shared_register! {
        /// PTZOptics combined flip prefix, followed by the flip mode byte.
        FLIP_COMBINED,
        /// Combined flip inquiry.
        FLIP_COMBINED_INQUIRY = [0x04, 0xA4];
    }

    shared_register! {
        /// Picture effect prefix, followed by the effect byte.
        PICTURE_EFFECT,
        /// Picture effect inquiry.
        PICTURE_EFFECT_INQUIRY = [0x04, 0x63];
    }
}

/// Legacy flip, mirror and freeze commands.
pub mod flip {
    use super::COMMAND;

    /// Vertical picture flip on (`02`) / off (`03`) prefix.
    pub const VERTICAL: [u8; 3] = [COMMAND, 0x04, 0x66];

    /// Horizontal mirror on (`02`) / off (`03`) prefix.
    pub const HORIZONTAL: [u8; 3] = [COMMAND, 0x04, 0x61];

    /// Image freeze on (`02`) / off (`03`) prefix.
    pub const FREEZE: [u8; 3] = [COMMAND, 0x04, 0x62];
}

/// PTZOptics OSD menu commands.
pub mod menu {
    use super::{concat, COMMAND};

    shared_register! {
        /// Menu register: display on (`02`) / off (`03`), enter (`05`) and
        /// return (`04`).
        MENU,
        /// Menu open/close inquiry.
        MENU_INQUIRY = [0x06, 0x06];
    }

    /// OSD navigation prefix: the pan/tilt drive at the fixed `0E 0E` speed
    /// pair, followed by a pan/tilt direction pair.
    pub const NAVIGATE: [u8; 5] = concat(&[&super::pan_tilt::DRIVE, &[0x0E, 0x0E]]);

    /// Sony FR7 direct menu prefix, followed by the two control bytes.
    pub const DIRECT: [u8; 4] = [COMMAND, 0x7E, 0x04, 0x72];
}

/// Tally commands and the registers their inquiries share.
pub mod tally {
    use super::{concat, COMMAND, INQUIRY};

    /// Sony FR7 red tally register (`7E 01 0A`).
    const RED: [u8; 3] = [0x7E, 0x01, 0x0A];

    /// Sony FR7 green tally register (`7E 04 1A`).
    const GREEN: [u8; 3] = [0x7E, 0x04, 0x1A];

    /// Red tally on.
    pub const RED_ON: [u8; 6] = concat(&[&[COMMAND], &RED, &[0x00, 0x02]]);

    /// Red tally off.
    pub const RED_OFF: [u8; 6] = concat(&[&[COMMAND], &RED, &[0x00, 0x03]]);

    /// Tally lamp brightness low.
    pub const BRIGHT_LOW: [u8; 6] = concat(&[&[COMMAND], &RED, &[0x01, 0x04]]);

    /// Tally lamp brightness high.
    pub const BRIGHT_HIGH: [u8; 6] = concat(&[&[COMMAND], &RED, &[0x01, 0x05]]);

    /// Red tally inquiry.
    pub const RED_INQUIRY: [u8; 4] = concat(&[&[INQUIRY], &RED]);

    /// Green tally on.
    pub const GREEN_ON: [u8; 6] = concat(&[&[COMMAND], &GREEN, &[0x00, 0x02]]);

    /// Green tally off.
    pub const GREEN_OFF: [u8; 6] = concat(&[&[COMMAND], &GREEN, &[0x00, 0x03]]);

    /// Green tally inquiry.
    pub const GREEN_INQUIRY: [u8; 4] = concat(&[&[INQUIRY], &GREEN]);

    const PTZOPTICS: [u8; 3] = [0x0A, 0x02, 0x02];

    /// PTZOptics tally flash.
    pub const PTZOPTICS_FLASH: [u8; 4] = concat(&[&PTZOPTICS, &[0x01]]);

    /// PTZOptics tally on.
    pub const PTZOPTICS_ON: [u8; 4] = concat(&[&PTZOPTICS, &[0x02]]);

    /// PTZOptics tally off.
    pub const PTZOPTICS_OFF: [u8; 4] = concat(&[&PTZOPTICS, &[0x03]]);
}

/// PTZOptics streaming and USB audio commands.
pub mod streaming {
    /// Multicast on (`01`) / off (`02`) prefix.
    pub const MULTICAST: [u8; 3] = [0x0B, 0x01, 0x23];

    /// NDI quality prefix, followed by the quality byte.
    pub const NDI_QUALITY: [u8; 3] = [0x0B, 0x01, 0x01];

    /// USB audio (UAC) register: the command appends on (`02`) / off (`03`)
    /// and the inquiry is the bare register.
    pub const USB_AUDIO: [u8; 4] = [0x2A, 0x02, 0xA0, 0x04];
}

/// Sony FR7 ND filter commands.
pub mod nd_filter {
    use super::COMMAND;

    /// Preset (`00`) / variable (`01`) mode prefix.
    pub const MODE: [u8; 4] = [COMMAND, 0x7E, 0x04, 0x52];

    /// Variable ND direct prefix, followed by `0p 0q`.
    pub const DIRECT: [u8; 5] = [COMMAND, 0x7E, 0x04, 0x42, 0x00];

    /// Variable ND step prefix.
    pub const STEP: [u8; 4] = [COMMAND, 0x7E, 0x04, 0x12];

    /// Automatic ND on (`02`) / off (`03`) prefix.
    pub const AUTO: [u8; 4] = [COMMAND, 0x7E, 0x04, 0x53];
}

/// PTZOptics motion sync commands.
pub mod motion_sync {
    /// Motion sync mode prefix, followed by the mode byte.
    pub const MODE: [u8; 3] = [0x0A, 0x11, 0x13];

    /// Motion sync speed prefix, followed by the speed byte.
    pub const SPEED: [u8; 3] = [0x0A, 0x11, 0x14];
}

/// Sony FR7 pan/tilt speed-step range command.
pub mod variable_speed {
    use super::COMMAND;

    /// Speed-step range prefix, followed by `08` (24 steps) or `18` (50 steps).
    pub const STEP_RANGE: [u8; 3] = [COMMAND, 0x06, 0x45];
}

/// System commands. Address Set and I/F Clear are broadcast (`88`) frames.
pub mod system {
    use super::COMMAND;

    /// Address Set command byte: the command is `88 30 01 FF` and the final
    /// reply `88 30 0p FF`.
    pub const ADDRESS_SET_REGISTER: u8 = 0x30;

    /// Address Set body; the broadcast frame is `88 30 01 FF`.
    pub const ADDRESS_SET: [u8; 2] = [ADDRESS_SET_REGISTER, 0x01];

    /// I/F Clear body; the broadcast frame is `88 01 00 01 FF`.
    pub const INTERFACE_CLEAR: [u8; 3] = [COMMAND, 0x00, 0x01];

    /// Command Cancel opcode nibble: the frame is `8x 2y FF`, whose one body
    /// byte carries the socket `y` in its low nibble.
    pub const COMMAND_CANCEL: u8 = 0x20;

    /// PTZOptics settings save.
    pub const SETTINGS_SAVE: [u8; 4] = [COMMAND, 0x04, 0xA5, 0x10];
}
