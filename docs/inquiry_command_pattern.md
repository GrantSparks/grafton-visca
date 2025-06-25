# InquiryCommand Pattern Documentation

## Overview

The `InquiryCommand` derive macro provides a clean way to implement VISCA inquiry commands while maintaining type safety and avoiding the limitations of Rust's macro system. This pattern separates the enum definition from the implementation details, making the codebase more maintainable and easier to understand.

## The Pattern

### 1. Define the Enum Manually

The `InquiryCommand` enum is defined manually in `src/command/inquiry.rs` with all possible variants:

```rust
#[derive(Debug, Copy, Clone)]
pub enum InquiryCommand {
    Power,
    PanTiltPosition,
    ZoomPosition,
    // ... other variants
}
```

### 2. Create Struct Implementations

For each inquiry command, create a struct with the `InquiryCommand` derive macro:

```rust
use grafton_visca_macros::InquiryCommand;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x00, response = "Power", inquiry_variant = "Power")]
struct PowerInquiry;

#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x47, response = "ZoomPosition", inquiry_variant = "ZoomPosition", parser = "position")]
struct ZoomPositionInquiry;
```

### 3. Generated Code

The macro generates:

1. **Command trait implementation**:
   ```rust
   impl Command for PowerInquiry {
       fn to_bytes(&self) -> Result<Vec<u8>, Error> {
           Ok(vec![0x81, 0x09, 0x04, 0x00, 0xFF])
       }
       
       fn response_type(&self) -> Option<ResponseType> {
           Some(ResponseType::Power)
       }
       
       fn command_category(&self) -> CommandCategory {
           CommandCategory::Quick
       }
   }
   ```

2. **From conversion**:
   ```rust
   impl From<PowerInquiry> for InquiryCommand {
       fn from(_: PowerInquiry) -> Self {
           InquiryCommand::Power
       }
   }
   ```

3. **Parser implementation** (if specified):
   ```rust
   impl PowerInquiry {
       pub fn parse_response(&self, data: &[u8]) -> Result<InquiryResponse, Error> {
           // Generated parser code
       }
   }
   ```

## Attributes

The `#[visca(...)]` attribute accepts the following parameters:

- `command`: The command byte (required)
- `response`: The ResponseType variant name (required)
- `inquiry_variant`: The InquiryCommand enum variant name (required)
- `sub_command`: The subcategory byte (optional, defaults to 0x04)
- `parser`: The parser type (optional)
- `type`: Additional type info for mode parsers (optional)
- `field`: Field name for value parsers (optional)
- `offset`: Offset for offset parsers (optional)
- `custom_fn`: Custom parser function name (optional)

## Parser Types

Available parser types:

- `"bool"`: Parses boolean responses (On/Off)
- `"byte"` or `"direct_byte"`: Parses single byte values
- `"position"`: Parses 4-byte position values
- `"nibble"` or `"extended_nibble"`: Parses nibble-encoded values
- `"offset"`: Parses values with an offset
- `"flags"` or `"bit_flags"`: Parses bit flag responses
- `"mode"` or `"mode_enum"`: Parses mode enum values
- `"pan_tilt"`: Parses pan/tilt position responses
- `"custom"`: Uses a custom parser function

## Adding a New Inquiry Command

To add a new inquiry command:

1. Add the variant to the `InquiryCommand` enum:
   ```rust
   pub enum InquiryCommand {
       // existing variants...
       NewFeature,
   }
   ```

2. Add the corresponding variant to the `ResponseType` enum:
   ```rust
   pub enum ResponseType {
       // existing variants...
       NewFeature,
   }
   ```

3. Create the struct implementation:
   ```rust
   #[derive(InquiryCommand, Debug, Copy, Clone)]
   #[visca(command = 0xXX, response = "NewFeature", inquiry_variant = "NewFeature")]
   struct NewFeatureInquiry;
   ```

4. Update the manual `Command` implementation in `inquiry.rs` to handle the new variant:
   ```rust
   impl Command for InquiryCommand {
       fn to_bytes(&self) -> Result<Vec<u8>, Error> {
           let bytes = match self {
               // existing cases...
               Self::NewFeature => vec![0x81, 0x09, 0x04, 0xXX, 0xFF],
           };
           Ok(bytes)
       }
       
       fn response_type(&self) -> Option<ResponseType> {
           match self {
               // existing cases...
               Self::NewFeature => Some(ResponseType::NewFeature),
           }
       }
   }
   ```

## Benefits

1. **Type Safety**: Full compile-time checking of all conversions
2. **Discoverability**: All command variants are visible in the enum definition
3. **Reduced Boilerplate**: The macro eliminates repetitive implementation code
4. **Maintainability**: Clear separation between API surface and implementation
5. **Flexibility**: Each command can have custom parsing logic

## Testing

The pattern includes compile-time verification:

```rust
#[test]
fn all_inquiry_variants_have_implementations() {
    let commands = vec![
        InquiryCommand::Power,
        InquiryCommand::ZoomPosition,
        // ... all variants
    ];
    
    for cmd in commands {
        assert!(cmd.to_bytes().is_ok());
        assert!(cmd.response_type().is_some());
    }
}
```

This ensures that every enum variant has a proper implementation and helps catch any missing implementations at compile time.