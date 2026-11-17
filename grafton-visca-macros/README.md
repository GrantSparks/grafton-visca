# grafton-visca-macros

[![Crates.io](https://img.shields.io/crates/v/grafton-visca-macros.svg)](https://crates.io/crates/grafton-visca-macros)
[![Documentation](https://docs.rs/grafton-visca-macros/badge.svg)](https://docs.rs/grafton-visca-macros)
[![License](https://img.shields.io/crates/l/grafton-visca-macros.svg)](https://github.com/GrantSparks/grafton-visca#license)

Procedural macros for checked VISCA values and typed inquiry requests. Use the
entry points re-exported by `grafton-visca`, which also supplies the value,
request, error and optional serialization contracts used by generated code.

## Overview

- **`#[value(...)]`** declares a checked scalar value.
- **`#[value_enum]`** declares a value enum and its wire conversion.
- **`#[derive(ViscaInquiry)]`** implements a typed inquiry request and decoder.

The value attributes generate `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`,
`PartialOrd`, `Ord`, `Hash`, and `Display`. Both use
`grafton_visca::wire::WireValue` for wire conversion. They add serde, schemars,
and ts-rs support according to the features enabled on `grafton-visca`, including
when a downstream crate renames that dependency. When an attribute is reached
only through another crate's re-export, set `crate = provider::visca` to the
provider's re-export of the main crate.

## Checked scalar values

```rust
use grafton_visca::{value, wire::WireValue};

#[value(min = 0, max = 0x4000, display = "Lens position {:#06x}")]
pub struct LensPosition(u16);

# fn main() -> Result<(), grafton_visca::Error> {
let position = LensPosition::new(0x2000)?;
assert_eq!(position.value(), 0x2000);
assert_eq!(LensPosition::MIN.value(), 0);
assert_eq!(LensPosition::MAX.value(), 0x4000);
assert_eq!(LensPosition::from_wire(position.to_wire())?, position);
assert!(LensPosition::new(0x4001).is_err());
# Ok(())
# }
```

The generated API is the same for every bounded scalar:

- `const fn new(value) -> Result<Self>` validates the declared inclusive domain.
- `const fn value(self)` returns the inner value.
- `MIN` and `MAX` are values of the declared type.
- `TryFrom<Inner>` validates construction; `From<Value> for Inner` reads it back.
- `Display` follows the `display` format string.
- With serde enabled, deserialization uses the checked constructor.
- With schemars enabled, named integer schemas describe the same inclusive bounds
  or exact `valid_values` membership as construction and deserialization. Float
  range schemas retain their declared bounds. Integer values remain JSON integers
  and floating-point values remain numbers; titles, descriptions, renames and
  references retain their usual derive behavior.

Integer bounds are integer expressions, and `min` must not exceed `max`.
A sparse integer domain may instead use `valid_values = [ ... ]`. Out-of-range
range values return `Error::ParameterOutOfRange`. The type validates its own domain;
request preparation separately validates the selected camera profile's domain.
Integer schemas likewise describe the value's complete domain, before the camera's
narrower limits are checked. Centered and biased wire mappings do not change
the serialized semantic values or their schema constraints.

### Centered and biased encodings

The default wire representation is the inner value. Declare a transformation
when the protocol encodes a centered or biased value differently:

```rust
use grafton_visca::{value, wire::WireValue};

#[value(min = -7, max = 7, display = "Adjustment {}", wire = centered(7, u8))]
pub struct Adjustment(i8);

#[value(min = -990, max = 990, display = "Gain {}", wire = biased(990, u16))]
pub struct Gain(i16);

# fn main() -> Result<(), grafton_visca::Error> {
assert_eq!(Adjustment::new(0)?.to_wire(), 7);
assert_eq!(Gain::new(0)?.to_wire(), 990);
# Ok(())
# }
```

`WireValue` is the public codec contract:

```rust
# use grafton_visca::Result;
pub trait WireValue: Sized {
    type Wire;
    fn to_wire(self) -> Self::Wire;
    fn from_wire(value: Self::Wire) -> Result<Self>;
}
```

A wire code is not a physical unit. Profile-dependent conversions, such as
Kelvin to a model's color-temperature code, belong to request preparation.

### Finite floating-point values

The same attribute supports `f32` and `f64` physical quantities:

```rust
use grafton_visca::value;

#[value(min = f32::MIN, max = f32::MAX, display = "{} degrees")]
pub struct Heading(f32);

# fn main() -> Result<(), grafton_visca::Error> {
assert_eq!(Heading::new(45.0)?.value(), 45.0);
assert!(Heading::new(f32::NAN).is_err());
assert!(Heading::new(f32::INFINITY).is_err());
assert_eq!(Heading::new(-0.0)?, Heading::new(0.0)?);
# Ok(())
# }
```

Float bounds must be finite and ordered. Construction rejects NaN, infinity
and values outside those bounds, and normalizes negative zero. Equality,
ordering and hashing therefore agree. Float wire values use the identity
representation; centered and biased mappings are integer-only. The selected
profile still owns conversions from a physical quantity to its protocol codes.

## Value enums

```rust
use grafton_visca::{value_enum, wire::WireValue};

#[value_enum]
pub enum ExposureSetting {
    Auto = 0x00,
    Manual = 0x03,
    Shutter = 0x0A,
    Iris = 0x0B,
    Bright = 0x0D,
}

# fn main() -> Result<(), grafton_visca::Error> {
assert_eq!(ExposureSetting::Manual.to_wire(), 0x03);
assert_eq!(ExposureSetting::from_wire(0x03)?, ExposureSetting::Manual);
assert!(ExposureSetting::from_wire(0xFF).is_err());
# Ok(())
# }
```

Declare explicit, unique byte discriminants. `WireValue::from_wire` rejects an
unrecognized code unless the enum declares its supported `Unknown(code)` form.
Use `to_wire` and `from_wire` for protocol bytes; Rust conversions are reserved
for value construction and extraction.

## Typed inquiries

`ViscaInquiry` generates `Request` and `Inquiry` implementations, with an exact
`MAX_SIZE`, zero-allocation `write_into`, and decoding through the canonical
built-in inquiry table.

```rust
use grafton_visca::{CameraId, Request, ViscaInquiry};

#[derive(ViscaInquiry, Debug, Copy, Clone)]
#[visca(opcode = 0x00, response = Power)]
pub struct PowerInquiry;

let mut buffer = [0u8; <PowerInquiry as Request>::MAX_SIZE];
let len = PowerInquiry
    .write_into(CameraId::CAMERA_1, &mut buffer)
    .expect("inquiry should encode");
assert_eq!(
    &buffer[..len],
    &[0x81, 0x09, 0x04, 0x00, grafton_visca::command::VISCA_TERMINATOR]
);
```

The generated `parse_response()` and `Inquiry::decoder()` share the canonical
inquiry decoder. `Inquiry::decoder_for_profile()` uses the same decoder with
the selected profile's framing.

### Decoding and typed responses

Omit `parser` to use the built-in table named by `response`. Its decoder owns
reply widths, value conventions and profile framing. Use `parser = Custom`
with `parse_with` only for an explicit custom payload parser; the parser returns
`Result<InquiryData, Error>`. Checked `Payload` and `Nibbles` helpers are
available for vendor-specific payloads.

| Key | Meaning |
| --- | --- |
| `parser = Custom`, `parse_with = path` | Explicit custom parser; both keys are required together. |
| `data_variant = Variant` | Custom parser's `InquiryData` variant; requires `Custom` and `typed_response`. |
| `typed_response = Type` | Result type, resolved as written in the deriving scope. |
| `typed_field = field` or `(fields, ...)` | Fields projected from the decoded variant. |
| `typed_constructor = new`, `ok_new`, `new_u8`, `ok_struct` | Optional typed construction; `new_u8` checks narrowing. |
| `typed_is_tuple` | Destructures a tuple variant; requires `typed_response`. |

Every attribute key may appear once. Unknown and repeated keys are compile
errors. Inquiry `opcode` and `subcode` are unsuffixed integer literals in
`0..=254`, in any radix; `0xFF` is reserved for the frame terminator. All
`typed_*` keys require `typed_response`.

## Requirements and versions

Rust 1.98 or later is required. The `response` attribute must name an existing
inquiry kind, or `Raw` for a custom inquiry with a manually implemented
`ResponseParser`. Downstream inquiries use the standard five-byte VISCA form.

The macro crate follows the main crate's version. Most applications need only
the main dependency and its re-exported macros. Direct macro-crate users must
use matching versions:

```toml
[dependencies]
grafton-visca = "=2.0.0"
grafton-visca-macros = "=2.0.0"
```

During a release, this macro crate is published and indexed before the
same-version main crate. See `RELEASING.md` for the guarded two-crate sequence.

## License

Licensed under MIT OR Apache-2.0. Both texts are shipped inside this package:
see [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
