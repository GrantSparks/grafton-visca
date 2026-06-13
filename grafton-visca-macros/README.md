# grafton-visca-macros

[![Crates.io](https://img.shields.io/crates/v/grafton-visca-macros.svg)](https://crates.io/crates/grafton-visca-macros)
[![Documentation](https://docs.rs/grafton-visca-macros/badge.svg)](https://docs.rs/grafton-visca-macros)
[![License](https://img.shields.io/crates/l/grafton-visca-macros.svg)](../LICENSE-MIT)

Procedural macros for the grafton-visca crate, providing derive macros to eliminate boilerplate in VISCA protocol implementations.

## Overview

This crate provides four macros that work together to create type-safe, efficient VISCA protocol implementations:

- **`ViscaInquiry`** - Generate inquiry command implementations with parser support
- **`ViscaEnum`** - Automatic enum/u8 conversions for protocol values
- **`ViscaValue`** - Value wrapper types with VISCA encoding
- **`delegate_to_session`** - Auto-generate CameraSession forwarding implementations

## ViscaInquiry

Generates complete `ViscaCommand` implementations for inquiry commands, including exact-size,
zero-allocation `write_into` encoding and optional response parsing.

### Basic Usage

```rust
use grafton_visca::{command::ViscaCommand, CameraId, ViscaInquiry};

#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(opcode = 0x00, response = Power, parser = Bool)]
pub struct PowerInquiry;

// For commands with subcategories:
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(opcode = 0x12, subcode = 0x06, response = PanTiltPosition, parser = PanTilt)]
pub struct PanTiltPositionInquiry;

let mut buffer = [0u8; PowerInquiry::MAX_SIZE];
let len = PowerInquiry
    .write_into(CameraId::CAMERA_1, &mut buffer)
    .expect("inquiry should encode");
assert_eq!(
    &buffer[..len],
    &[0x81, 0x09, 0x04, 0x00, grafton_visca::command::VISCA_TERMINATOR]
);
```

### Generated Code

The macro generates:
- `ViscaCommand` trait implementation
- exact `MAX_SIZE`
- `write_into()` for caller-provided buffers
- `behavior()` returning built-in or raw inquiry response routing metadata
- `parse_response()` method when parser is specified
- `ResponseParser` implementation when typed response attributes are specified

### Parser Types

- `Bool` - Boolean values (0x02 = true, 0x03 = false)
- `Byte` / `DirectByte` - Direct byte value
- `Position` - 4-nibble position value (converts to u16)
- `Nibble` / `ExtendedNibble` - Extended nibble encoding
- `Flags` / `BitFlags` - Bit flags (for image flip)
- `Mode` / `ModeEnum` - Enum value parsing
- `PanTilt` - Special parser for pan/tilt positions
- `LastNibble` - Last nibble from a nibble-encoded payload
- `BoolConvention` - Boolean parsing with an explicit convention

## ViscaEnum

Generates bidirectional u8 conversions for enums representing VISCA protocol values.

### Basic Usage

```rust
use grafton_visca_macros::ViscaEnum;

#[derive(Debug, Copy, Clone, PartialEq, Eq, ViscaEnum)]
pub enum ExposureMode {
    Auto = 0x00,
    Manual = 0x03,
    Shutter = 0x0A,
    Iris = 0x0B,
    Bright = 0x0D,
}
```

### Generated Implementations

- `TryFrom<u8>` - Converts u8 to enum with validation
- `From<Enum> for u8` - Converts enum to u8 value
- `is_valid_discriminant(u8) -> bool` - Const validation function

### Advanced Features

```rust
#[derive(Debug, Copy, Clone, PartialEq, Eq, ViscaEnum)]
#[visca_enum(error_type = MyError, exhaustive = false)]
pub enum Mode {
    #[visca_enum(name = "Automatic Mode")]
    Auto = 0x00,

    #[visca_enum(name = "Manual Control")]
    Manual = 0x03,

    #[visca_enum(skip)]  // Excluded from TryFrom<u8>
    _Reserved = 0xFF,
}
```

## ViscaValue

Creates value wrapper types with VISCA encoding and validation.

### Basic Usage

```rust
use grafton_visca_macros::ViscaValue;

#[derive(ViscaValue, Debug, Copy, Clone)]
#[visca_value(bytes = 2)]
struct ZoomPosition(u16);

#[derive(ViscaValue, Debug, Copy, Clone)]
#[visca_value(bytes = 1)]
struct ZoomSpeed(u8);
```

### Generated Methods

The macro generates methods for:
- Converting to/from byte representations
- Validation of value ranges
- VISCA protocol encoding

## delegate_to_session

Attribute macro for auto-generating CameraSession forwarding implementations.

### Usage

```rust
use grafton_visca_macros::delegate_to_session;

#[delegate_to_session]
pub trait ZoomControl {
    type Mode: Mode;

    fn zoom_stop(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;
    fn zoom_tele_std(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;
    // ... more methods
}
```

The macro generates forwarding implementations that delegate from `CameraSession` to the inner `Camera` instance with proper error handling.

## Integration with grafton-visca

These macros are re-exported by the main grafton-visca crate:

```rust
use grafton_visca::{ViscaInquiry, ViscaEnum, ViscaValue};
use grafton_visca::camera::delegate_to_session;
```

## Benefits

1. **Type Safety** - Each command and value is a distinct type
2. **Zero Boilerplate** - Macros generate all repetitive code
3. **Protocol Safety** - Automatic VISCA terminator handling
4. **Compile-time Validation** - Invalid attributes caught at compile time
5. **Consistent API** - All commands follow the same patterns
6. **Performance** - Zero-cost abstractions with const functions where possible

## Examples

### Example Inquiry and Enum

```rust
use grafton_visca_macros::{ViscaInquiry, ViscaEnum};

#[derive(Debug, Copy, Clone, PartialEq, Eq, ViscaEnum)]
pub enum ExposureMode {
    Auto = 0x00,
    Manual = 0x03,
    Shutter = 0x0A,
    Iris = 0x0B,
}

#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(opcode = 0x39, response = ExposureMode, parser = Mode, value_type = ExposureMode)]
pub struct ExposureModeInquiry;
```

### Value Types with Validation

```rust
use grafton_visca_macros::ViscaValue;

#[derive(ViscaValue, Debug, Copy, Clone)]
#[visca_value(bytes = 2, min = 0x0000, max = 0x4000)]
pub struct ZoomPosition(u16);

impl ZoomPosition {
    pub fn from_percentage(percent: f32) -> Result<Self, Error> {
        let value = (percent.clamp(0.0, 100.0) * 0x4000 as f32 / 100.0) as u16;
        Ok(Self(value))
    }

    pub fn to_percentage(&self) -> f32 {
        self.0 as f32 * 100.0 / 0x4000 as f32
    }
}
```

## Requirements

- Rust 1.80 or later
- The `response` attribute in `ViscaInquiry` must reference existing `InquiryKind` variants, or `Raw` for a raw custom inquiry whose `ResponseParser` is implemented manually
- Downstream `ViscaInquiry` derives use the standard five-byte VISCA inquiry form
- Enums using `ViscaEnum` must have explicit discriminant values
- All discriminant values must be unique and valid u8 values (0-255)

## Version Compatibility

This crate follows the same versioning as the main `grafton-visca` crate. Always use matching versions:

```toml
[dependencies]
grafton-visca = "1"
grafton-visca-macros = "1"
```

## License

Licensed under MIT OR Apache-2.0 dual license. See the LICENSE-MIT and LICENSE-APACHE files in the repository root.
