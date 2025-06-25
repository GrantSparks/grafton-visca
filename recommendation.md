# Recommended Approach: Manual ResponseType with Compile-Time Validation

## Overview

The best approach is to keep the `ResponseType` enum manually defined but add compile-time validation to ensure it stays in sync with the derive macro usage. This follows the pattern used by established crates like `serde`.

## Implementation Strategy

### 1. Keep Manual ResponseType Enum
- Maintain the existing `ResponseType` enum in `src/command/response.rs`
- This provides clear documentation and a single source of truth for response types

### 2. Add Compile-Time Validation in the Derive Macro
- In the `InquiryCommand` derive macro, validate that the specified response type exists
- Generate a compile error if an invalid response type is used

### 3. Consider Adding a Helper Trait
Create a trait that links inquiry commands to their response types:

```rust
pub trait HasResponseType {
    fn response_type() -> ResponseType;
}
```

### 4. Documentation Generation
- Use doc comments in the derive macro to generate comprehensive documentation
- Include the expected response type in the generated docs

## Example Implementation

```rust
// In the derive macro
#[proc_macro_derive(InquiryCommand, attributes(visca))]
pub fn derive_inquiry_command(input: TokenStream) -> TokenStream {
    // ... parsing code ...
    
    // Validate response type exists
    let response_type_check = quote! {
        // This will fail to compile if ResponseType doesn't have the variant
        let _ = ::grafton_visca::command::ResponseType::#response_type;
    };
    
    // Generate the implementation
    quote! {
        #response_type_check
        
        impl ::grafton_visca::command::Command for #name {
            fn response_type(&self) -> Option<::grafton_visca::command::ResponseType> {
                match self {
                    #(#arms)*
                }
            }
            // ... other methods ...
        }
    }
}
```

## Benefits

1. **Compile-Time Safety**: Invalid response types cause compilation errors
2. **Clear Documentation**: ResponseType enum serves as documentation
3. **No Runtime Overhead**: Everything is resolved at compile time
4. **Follows Rust Patterns**: Similar to how serde handles its types
5. **Backward Compatible**: No breaking changes to existing code

## Alternative: Response Type Registry

If you want more automation, consider a response type registry pattern:

```rust
// In a separate module
#[macro_export]
macro_rules! define_response_types {
    ($(
        $variant:ident => $parser:expr
    ),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ResponseType {
            $(
                $variant,
            )*
        }
        
        impl ResponseType {
            pub fn parser(&self) -> fn(&[u8]) -> IResult<&[u8], InquiryResponse> {
                match self {
                    $(
                        Self::$variant => $parser,
                    )*
                }
            }
        }
    };
}

// Usage
define_response_types! {
    Power => parse_power_response,
    PanTiltPosition => parse_pan_tilt_position,
    ZoomPosition => parse_zoom_position,
    // ... etc
}
```

This approach still requires manual maintenance but provides a more structured way to define response types and their associated parsers.

## Conclusion

The manual maintenance approach with compile-time validation provides the best balance of:
- Simplicity
- Type safety  
- Documentation
- Maintainability
- Following established Rust patterns

This is the approach I recommend for the grafton-visca project.