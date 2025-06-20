# API Parity Analysis: Async vs Blocking Camera API

## Current Architecture

The Camera API is split between async and blocking implementations with significant feature disparity:

### Feature Flags
- `async-client`: Full-featured implementation with all methods
- `blocking-client`: Limited subset of methods
- Default feature: `blocking-client`

### Method Availability

#### Async Implementation (`#[cfg(feature = "async-client")]`)
Located in `src/camera/commands.rs` lines 62-664:
- **Total methods**: ~78 public methods
- Includes all command types: power, movement, zoom, focus, exposure, image processing, color, etc.
- Full inquiry support in `src/camera/inquiry.rs`

#### Blocking Implementation (`#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]`)
Located in `src/camera/commands.rs` lines 668-853:
- **Total methods**: 18 public methods
- Only basic commands: power, movement, zoom, focus, presets, exposure mode, white balance mode, gain

#### Missing from Blocking API
1. **All inquiry methods** (get_power_state, get_position, get_zoom_position, etc.)
2. **Advanced positioning**: set_position_units, set_position_normalized
3. **Exposure controls**: set_iris, set_shutter, exposure compensation, brightness, dynamic range
4. **Image processing**: backlight, noise reduction, black/white mode, image flip
5. **Color controls**: saturation, hue, color temperature, red/blue gain tuning
6. **Image quality**: luminance, contrast, sharpness controls

## Root Causes

1. **Conditional Compilation**: Methods are wrapped in `#[cfg]` attributes creating different APIs
2. **Incomplete Implementation**: Blocking version was not kept in sync with async features
3. **No Blocking Inquiry**: All inquiry methods require async, making blocking API unable to query camera state

## Impact on Examples

Examples fail because they assume methods exist in both modes:
- `configurable_timeouts.rs`: References missing inquiry methods (comments note they're unavailable)
- `new_features_demo.rs`: Requires async-client feature
- Other examples likely have similar issues

## Solutions

### Option 1: Full Parity (Recommended)
Add all missing methods to the blocking implementation:
1. Copy method signatures from async impl
2. Remove `async`/`await` keywords
3. Change `&self` to `&mut self` for blocking
4. Implement blocking versions of inquiry methods

### Option 2: Unified API with Runtime Detection
Use a single implementation that works for both:
- Use `cfg_if!` or similar to handle async/blocking differences internally
- Expose same public API regardless of features

### Option 3: Document Limitations
If parity isn't feasible:
- Clearly document which methods are async-only
- Update examples to use conditional compilation
- Consider deprecating blocking mode if it can't be maintained

### Option 4: Async-First with Blocking Adapter
- Make async the primary API
- Provide a blocking wrapper that uses a minimal runtime internally
- This ensures feature parity automatically