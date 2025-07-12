# Domain Types in VISCA Codebase

## Core Unit Types (src/units.rs)

### Basic Wrapper Types
- `Degrees<T = f32>` - Position in degrees
- `ViscaUnits<T>` - Position in VISCA protocol units
- `Normalized<T = f32>` - Normalized position (0.0 to 1.0 or -1.0 to 1.0)
- `Percentage<T = f32>` - Percentage value (0.0 to 100.0)
- `Raw<T>` - Raw VISCA value
- `Magnification<T = f32>` - Magnification factor (e.g., 1.0x, 10.0x)
- `Kelvin` - Color temperature in Kelvin
- `Fraction` - Shutter speed as fraction (numerator, denominator)

## Domain-Specific Types (src/types.rs)

### Position Types
- `ZoomPosition(u16)` - Range: 0x0000 to 0x7000
- `FocusPosition(u16)` - Range: 0x1000 to 0xF000
- `PanPosition(i16)` - Range: -2448 to +2448
- `TiltPosition(i16)` - Range: -432 to +1296

### Speed Types
- `PanSpeed(u8)` - Range: 0 to 24
- `TiltSpeed(u8)` - Range: 0 to 20

### Exposure & Image Types
- `IrisLevel(u8)` - Range: 0x00 to 0x0C
- `ShutterSpeed(u16)` - Valid values: specific set
- `BrightnessLevel(u16)` - Range: 0x00 to 0x11
- `Gain(u8)` - Range: 0x00 to 0x07
- `GainLimit(u8)` - Range: 0x0 to 0xF

### Image Quality Types
- `SharpnessLevel(u8)` - Range: 0x00 to 0x0B
- `LuminanceLevel(u8)` - Range: 0x0 to 0xE
- `ContrastLevel(u8)` - Range: 0x0 to 0xE
- `DynamicRangeLevel(u8)` - Range: 0x0 to 0x8

### Color Types
- `ColorTemperature(u16)` - Range: 0x00 to 0x37
- `RedGain(u8)` - Range: 0x00 to 0xFF
- `BlueGain(u8)` - Range: 0x00 to 0xFF
- `SaturationLevel(u8)` - Range: 0x00 to 0x0E
- `HueLevel(u8)` - Range: 0x00 to 0x0E
- `RedTuning(i8)` - Range: -10 to +10
- `BlueTuning(i8)` - Range: -10 to +10

### Noise Reduction Types
- `NoiseReduction2DLevel(u8)` - Range: 1 to 5
- `NoiseReduction3DLevel(u8)` - Range: 1 to 8

## TryFrom Implementations Available

### For ZoomPosition
- `TryFrom<Percentage<f32>>`
- `TryFrom<Normalized<f32>>`
- `TryFrom<Magnification<f32>>`
- `From<Raw<u16>>`
- `TryFrom<f32>` (normalized value)

### For FocusPosition
- `TryFrom<Percentage<f32>>`
- `TryFrom<Normalized<f32>>`
- `From<Raw<u16>>`
- `TryFrom<f32>` (normalized value)

### For IrisLevel
- `TryFrom<Percentage<f32>>`
- `From<FStop>`
- `TryFrom<u8>`

### For Speed Types
- `TryFrom<Percentage<f32>>` for PanSpeed, TiltSpeed
- `From<SpeedLevel>` for PanSpeed, TiltSpeed, ZoomSpeed, FocusSpeed
- `From<Raw<u8>>` for PanSpeed, TiltSpeed

### For Other Types
- Most types have `TryFrom<Percentage<f32>>` implementations
- Most types have `From<Raw<T>>` implementations where T is their inner type
- ColorTemperature has `TryFrom<Kelvin>`
- ShutterSpeed has `TryFrom<Fraction>`

## Current API Usage

### Focus Methods
- `set_focus(position: FocusPosition)`
- Focus speed uses `FocusSpeed` type with `From<SpeedLevel>`

### Zoom Methods
- Zoom commands use `ZoomPosition` directly
- Zoom speed uses `ZoomSpeed` type with `From<SpeedLevel>`

### Pan/Tilt Methods
- Uses degrees (f32) in public API, converts internally
- Speed uses u8 in public API

### Exposure Methods
- `set_iris(level: IrisLevel)`
- `set_brightness(level: BrightnessLevel)`
- `set_gain(gain: Gain)`
- etc.

## Recommendations for API Enhancement

1. **Create generic trait for accepting multiple input types**:
   - Similar to `IntoIrisLevel` trait that exists
   - Could create `IntoZoomPosition`, `IntoFocusPosition`, etc.

2. **Use type wrappers more consistently**:
   - Pan/tilt methods should accept `Degrees<f32>` or similar
   - Speed parameters should use the Speed types or SpeedLevel enum

3. **Add builder patterns** for complex operations:
   - Could accept multiple input formats and convert internally

4. **Document the available conversions** in the public API docs