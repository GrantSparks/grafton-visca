# grafton-visca-macros

[![Crates.io](https://img.shields.io/crates/v/grafton-visca-macros.svg)](https://crates.io/crates/grafton-visca-macros)
[![Documentation](https://docs.rs/grafton-visca-macros/badge.svg)](https://docs.rs/grafton-visca-macros)
[![License](https://img.shields.io/crates/l/grafton-visca-macros.svg)](https://github.com/GrantSparks/grafton-visca#license)

Procedural macros for the grafton-visca crate, providing derive macros to eliminate boilerplate in VISCA protocol implementations.

## Overview

This crate provides three downstream derive macros for type-safe VISCA protocol
implementations:

- **`ViscaInquiry`** - Generate inquiry command implementations with parser support
- **`ViscaEnum`** - Automatic enum/u8 conversions for protocol values
- **`ViscaValue`** - Validated value wrapper types

## ViscaInquiry

Generates typed `Request` and `Inquiry` implementations for inquiry commands,
including exact-size, zero-allocation `write_into` encoding and optional response
parsing.

### Basic Usage

```rust
use grafton_visca::{CameraId, Request, ViscaInquiry};

#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(opcode = 0x00, response = Power, parser = Bool)]
pub struct PowerInquiry;

// For commands with subcategories:
#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(opcode = 0x12, subcode = 0x06, response = PanTiltPosition, parser = PanTilt)]
pub struct PanTiltPositionInquiry;

let mut buffer = [0u8; <PowerInquiry as Request>::MAX_SIZE];
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
- `Request` and `Inquiry` trait implementations
- exact `MAX_SIZE`
- `write_into()` for caller-provided buffers
- `parse_response()` method when parser is specified
- `ResponseParser` implementation when typed response attributes are specified

### Parser Types

- `Bool` - Boolean values (0x02 = true, 0x03 = false)
- `Byte` / `DirectByte` - Direct byte value
- `Position` - 4-nibble position value (converts to u16)
- `Nibble` / `ExtendedNibble` - Extended nibble encoding
- `Flags` / `BitFlags` - Bit flags (for image flip)
- `Mode` / `ModeEnum` - Enum value parsing
- `PanTilt` - Standard VISCA 4+4-nibble parser for signed pan/tilt replies.
  It widens the two signed 16-bit wire values to public `i32` coordinates and
  does not parse profile-owned codecs such as Sony BRC-300's 5+4 form.
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

Creates value wrapper types with construction-time validation and configurable
display formatting. It does not generate VISCA byte encoding or decoding APIs.

### Basic Usage

```rust
use grafton_visca_macros::ViscaValue;

#[derive(ViscaValue, Debug, Copy, Clone)]
#[visca_value(min = "0x0000", max = "0x4000")]
struct ZoomPosition(u16);

#[derive(ViscaValue, Debug, Copy, Clone)]
#[visca_value(valid_values = "[0x01, 0x02, 0x03]")]
struct ZoomSpeed(u8);
```

### Attributes and Generated API

The supported `visca_value` keys are `min`, `max`, `valid_values`,
`display_format`, and `display_prefix`. `min` and `max` must be specified
together as unsuffixed integer literals inside strings, for example
`min = "0x0000"`. `valid_values` is the alternative validation form. Every key
may appear only once; unknown keys, including `bytes`, are rejected.

The macro generates:

- `new(value)` with range or valid-value validation
- `value()`, `TryFrom<Inner>`, and `From<Wrapper> for Inner`
- `MIN` and `MAX` when bounds or `valid_values` are specified
- `Display` with optional format and prefix

Use the derive macros above with the typed request contracts documented by the
main crate. There is no forwarding attribute or mode-specific generated API.

## Integration with grafton-visca

The supported downstream derive macros are re-exported by the main
`grafton-visca` crate:

```rust
use grafton_visca::{ViscaInquiry, ViscaEnum, ViscaValue};
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
use grafton_visca::{command::ExposureMode, ViscaInquiry};

#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(opcode = 0x39, response = ExposureMode, parser = Mode, value_type = ExposureMode)]
pub struct ExposureModeInquiry;
```

Generated parsers return the named built-in `InquiryData` variant, so a Mode
parser's value type must be the corresponding `grafton_visca::command` enum.
Use `ViscaEnum` independently for downstream wire enums whose response parsing
is implemented by downstream code.

### Value Types with Validation

```rust
use grafton_visca_macros::ViscaValue;

#[derive(ViscaValue, Debug, Copy, Clone)]
#[visca_value(min = "0x0000", max = "0x4000")]
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

- Rust 1.88 or later
- The `response` attribute in `ViscaInquiry` must reference existing `InquiryKind` variants, or `Raw` for a raw custom inquiry whose `ResponseParser` is implemented manually
- Downstream `ViscaInquiry` derives use the standard five-byte VISCA inquiry form
- Enums using `ViscaEnum` must have explicit discriminant values
- All discriminant values must be unique and valid u8 values (0-255)

## Version Compatibility

This crate follows the same versioning as the main `grafton-visca` crate. Most
users need only the derives re-exported by the main crate. Direct macro-crate
users must use matching versions:

```toml
[dependencies]
grafton-visca = "=2.0.0-rc.1"
grafton-visca-macros = "=2.0.0-rc.1"
```

During a release, this macro crate is published and indexed before the
same-version main crate. See the repository's `RELEASING.md` for the guarded
two-crate sequence.

## License

Licensed under MIT OR Apache-2.0 dual license. Both texts are shipped inside
this package: see [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).
