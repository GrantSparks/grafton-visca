# grafton-visca-macros

[![Crates.io](https://img.shields.io/crates/v/grafton-visca-macros.svg)](https://crates.io/crates/grafton-visca-macros)
[![Documentation](https://docs.rs/grafton-visca-macros/badge.svg)](https://docs.rs/grafton-visca-macros)
[![License](https://img.shields.io/crates/l/grafton-visca-macros.svg)](../LICENSE-MIT)

Procedural macros for the grafton-visca crate, providing derive macros to eliminate boilerplate in VISCA protocol implementations.

## Overview

This crate provides three derive macros that work together to create type-safe, efficient VISCA protocol implementations:

- **`InquiryCommand`** - Generate inquiry command implementations
- **`ViscaEnum`** - Automatic enum/u8 conversions for protocol values
- **`ViscaValue`** - Value wrapper types with VISCA encoding

## InquiryCommand

Generates complete `Command` trait implementations for inquiry commands, including response parsing.

### Basic Usage

```rust
use grafton_visca_macros::InquiryCommand;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x00, response = "Power", parser = "bool")]
pub struct PowerInquiry;

// For commands with subcategories:
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x12, sub_command = 0x06, response = "PanTiltPosition", parser = "pan_tilt")]
pub struct PanTiltPositionInquiry;
```

### Generated Code

The macro generates:
- `Command` trait implementation
- `to_bytes()` method returning VISCA command bytes
- `response_type()` method returning expected `ResponseType`
- `parse_response()` method when parser is specified

### Parser Types

- `"bool"` - Boolean values (0x02 = true, 0x03 = false)
- `"byte"` - Direct byte value
- `"position"` - 4-nibble position value (converts to u16)
- `"nibble"` - Extended nibble encoding
- `"offset"` - Byte value with offset subtraction
- `"flags"` - Bit flags (for image flip)
- `"mode"` - Enum value parsing
- `"pan_tilt"` - Special parser for pan/tilt positions

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

## Integration with grafton-visca

These macros are re-exported by the main grafton-visca crate:

```rust
use grafton_visca::{InquiryCommand, ViscaEnum, ViscaValue};
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
use grafton_visca_macros::{InquiryCommand, ViscaEnum};

#[derive(Debug, Copy, Clone, PartialEq, Eq, ViscaEnum)]
pub enum ExposureMode {
    Auto = 0x00,
    Manual = 0x03,
    Shutter = 0x0A,
    Iris = 0x0B,
}

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x39, response = "ExposureMode", parser = "mode")]
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

- Rust 1.70 or later (for const trait implementations)
- The `response` attribute in `InquiryCommand` must reference existing `ResponseType` variants
- Enums using `ViscaEnum` must have explicit discriminant values
- All discriminant values must be unique and valid u8 values (0-255)

## Version Compatibility

This crate follows the same versioning as the main `grafton-visca` crate. Always use matching versions:

```toml
[dependencies]
grafton-visca = "0.7"
grafton-visca-macros = "0.7"
```

## License

Licensed under MIT OR Apache-2.0 dual license. See the LICENSE-MIT and LICENSE-APACHE files in the repository root.
