# Grafton VISCA Macros

This crate provides procedural macros to reduce boilerplate in the grafton-visca library.

## InquiryCommand Derive Macro

The basic `InquiryCommand` derive macro generates implementations for the `Command` trait:

```rust
#[derive(InquiryCommand)]
enum Inquiry {
    #[visca(0x00, response = Power)]
    Power,
    
    #[visca(0x47, response = ZoomPosition)]
    ZoomPos,
    
    #[visca(0x12, subcategory = 0x06, response = PanTiltPosition)]
    PanTiltPos,
}
```

This generates:
- `to_bytes()` method that creates the VISCA command bytes
- `response_type()` method that maps to the expected ResponseType
- `command_category()` method that returns `CommandCategory::Quick`

## InquiryCommandWithParser Derive Macro

The enhanced `InquiryCommandWithParser` macro also generates parser functions based on parser categories:

```rust
#[derive(InquiryCommandWithParser)]
enum EnhancedInquiry {
    #[visca(0x00, response = Power, parser = "bool")]
    Power,
    
    #[visca(0x47, response = ZoomPosition, parser = "position")]
    ZoomPos,
    
    #[visca(0xA1, response = Luminance, parser = "byte", field = "level")]
    Luminance,
    
    #[visca(0x44, response = RedGain, parser = "offset", field = "gain", offset = 10)]
    RedGain,
    
    #[visca(0x66, response = ImageFlip, parser = "flags")]
    ImageFlip,
}
```

### Parser Categories

1. **`bool`** - Boolean values (0x02 = true, 0x03 = false)
   ```rust
   #[visca(0x00, response = Power, parser = "bool")]
   ```

2. **`byte`** - Direct byte value (used as-is)
   ```rust
   #[visca(0xA1, response = Luminance, parser = "byte", field = "level")]
   ```

3. **`position`** - 4-nibble position value combined into u16
   ```rust
   #[visca(0x47, response = ZoomPosition, parser = "position")]
   ```

4. **`nibble`** - Extended nibble value (2 bytes combined)
   ```rust
   #[visca(0x42, response = Sharpness, parser = "nibble", field = "value")]
   ```

5. **`offset`** - Byte value with offset subtraction
   ```rust
   #[visca(0x44, response = RedGain, parser = "offset", field = "gain", offset = 10)]
   ```

6. **`flags`** - Bit flags (for horizontal/vertical flip)
   ```rust
   #[visca(0x66, response = ImageFlip, parser = "flags")]
   ```

7. **`mode`** - Enum mode values
   ```rust
   #[visca(0x39, response = ExposureMode, parser = "mode", mode_type = "ExposureMode")]
   ```

8. **`pan_tilt`** - Special parser for PanTiltPosition (8 bytes -> two i16 values)
   ```rust
   #[visca(0x12, subcategory = 0x06, response = PanTiltPosition, parser = "pan_tilt")]
   ```

9. **`custom`** - Custom parser function
   ```rust
   #[visca(0x00, response = Custom, parser = "custom", fn = "parse_custom_response")]
   ```

## Other Macros

### visca_method

Generates async/sync method pairs:

```rust
#[visca_method]
pub fn power_on(&self) -> PowerCommand {
    PowerCommand { power: Power::On }
}
```

### visca_inquiry_method

Generates inquiry methods that return parsed responses:

```rust
#[visca_inquiry_method]
pub fn power_status(&self) -> InquiryCommand {
    InquiryCommand::Power
}
```

## Benefits

- Reduces boilerplate by ~7 lines per inquiry command
- Provides compile-time validation
- Maintains type safety
- Generates consistent, correct implementations
- Makes the API more discoverable