# grafton-visca-macros

Procedural macros for the grafton-visca crate, providing derive macros to eliminate boilerplate in VISCA protocol implementations.

## Overview

This crate provides four derive macros that work together to create type-safe, efficient VISCA command implementations:

- **`InquiryCommand`** - Generate inquiry command implementations
- **`ViscaEnum`** - Automatic enum/u8 conversions for protocol values  
- **`ViscaValue`** - Value wrapper types with VISCA encoding
- **`ViscaEncode`** - Automatic VISCA command encoding

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

## ViscaEncode

Generates `EncodeVisca` trait implementations with automatic terminator handling and type safety.

### For Structs

```rust
use grafton_visca_macros::ViscaEncode;

#[derive(ViscaEncode, Debug, Copy, Clone)]
#[visca_encode(response = "Completion", max_size = 6, timeout = "Quick")]
struct ZoomStop;
```

### For Enums

```rust
#[derive(ViscaEncode, Debug, Copy, Clone)]
#[visca_encode(max_size = 6, timeout = "Quick")]
enum PowerCommand {
    #[visca_bytes(0x81, 0x01, 0x04, 0x00, 0x02)]
    On,
    #[visca_bytes(0x81, 0x01, 0x04, 0x00, 0x03)]
    Standby,
}
```

### Attributes

**Type-level** (`#[visca_encode(...)]`):
- `response` - ResponseType variant name
- `max_size` - Maximum command size (default: 32)
- `timeout` - CommandCategory for timeout (default: "Custom")
- `prefix` - Common prefix bytes for all variants

**Variant-level** (`#[visca_bytes(...)]`):
- List of byte values for the command

## Integration with grafton-visca

These macros are re-exported by the main grafton-visca crate:

```rust
use grafton_visca::{InquiryCommand, ViscaEnum, ViscaValue, ViscaEncode};
```

## Benefits

1. **Type Safety** - Each command and value is a distinct type
2. **Zero Boilerplate** - Macros generate all repetitive code
3. **Protocol Safety** - Automatic VISCA terminator handling  
4. **Compile-time Validation** - Invalid attributes caught at compile time
5. **Consistent API** - All commands follow the same patterns
6. **Performance** - Zero-cost abstractions with const functions where possible

## Requirements

- The `response` attribute in `InquiryCommand` must reference existing `ResponseType` variants
- Enums using `ViscaEnum` must have explicit discriminant values
- All discriminant values must be unique and valid u8 values (0-255)

## License

Licensed under Apache-2.0. See the LICENSE file in the repository root.