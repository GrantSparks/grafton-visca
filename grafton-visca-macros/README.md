# grafton-visca-macros

Procedural macros for the grafton-visca crate.

## InquiryCommand Derive Macro

The `InquiryCommand` derive macro generates implementations for the `Command` trait:

```rust
#[derive(InquiryCommand)]
#[visca(command = 0x00, response = "Power")]
struct PowerInquiry;
```

This generates:
- `Command` trait implementation
- `to_bytes()` method that returns the VISCA command bytes
- `response_type()` method that returns the expected `ResponseType`
- `command_category()` method that returns `CommandCategory::Quick`

### Attributes

- `command`: The command byte (required)
- `sub_command`: Optional sub-command byte for commands that need it
- `response`: The response type variant name (required)

## Advanced Usage

For inquiry commands with subcategories:

```rust
#[derive(InquiryCommand)]
#[visca(command = 0x12, sub_command = 0x06, response = "PanTiltPosition")]
struct PanTiltPositionInquiry;
```

This generates command bytes: `[0x81, 0x09, 0x06, 0x12, 0xFF]`

## Parser Support

The macro can also generate parser functions for specific response types:

```rust
#[derive(InquiryCommand)]
#[visca(command = 0x47, response = "ZoomPosition", parser = "position")]
struct ZoomPositionInquiry;
```

Supported parser types:
- `bool` - Parses boolean responses
- `position` - Parses 4-byte position values
- `byte` - Direct byte value
- `nibble` - Extended nibble format
- `offset` - Value with offset applied
- `mode` - Enum mode parsing
- `custom` - Custom parser function

## Usage in Libraries

The generated structs implement the `Command` trait and can be used directly:

```rust
let inquiry = PowerInquiry;
let bytes = inquiry.to_bytes()?;
let response_type = inquiry.response_type();
```

## Benefits

1. **Type Safety**: Each inquiry is a distinct type
2. **No Boilerplate**: The macro generates all the repetitive code
3. **Maintainability**: Adding new inquiries only requires defining a struct
4. **Compile-time Verification**: Invalid attributes are caught at compile time