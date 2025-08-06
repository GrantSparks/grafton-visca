# Macro Classification: Public API vs Internal Utilities

## Public API Macros

These macros are part of the public API and should remain exported:

### 1. `visca_bounded_param!`
- **Rationale**: Documented in module docs, used in public examples
- **Purpose**: Create validated parameter types that users can use
- **Example**: Creating custom bounded numeric parameters

### 2. `forward_facade!`
- **Rationale**: Essential for the facade pattern used by blocking/async APIs
- **Purpose**: Enables trait forwarding in public API types
- **Usage**: Required for extending the camera API

## Internal Implementation Macros

These macros are used internally for implementing commands and should be marked as internal:

### 3. `visca_command!`
- **Rationale**: Only used internally for command implementations
- **Purpose**: Reduce boilerplate in command modules
- **Recommendation**: Mark with `#[doc(hidden)]` or move to private module

### 4. `visca_bool_command!`
- **Rationale**: Internal command implementation detail
- **Purpose**: Simplify boolean command creation
- **Recommendation**: Mark with `#[doc(hidden)]`

### 5. `visca_builder!`
- **Rationale**: Internal command implementation detail
- **Purpose**: Create dynamic command builders
- **Recommendation**: Mark with `#[doc(hidden)]`

### 6. `visca_param_command!`
- **Rationale**: Internal command implementation detail
- **Purpose**: Single-byte parameter commands
- **Recommendation**: Mark with `#[doc(hidden)]`

### 7. `visca_const_command!`
- **Rationale**: Internal command implementation detail
- **Purpose**: Constant byte sequence commands
- **Recommendation**: Mark with `#[doc(hidden)]`

### 8. `visca_test!`
- **Rationale**: Already marked `#[doc(hidden)]`, only for tests
- **Purpose**: Test generation
- **Status**: Already correctly marked

### 9. `visca_bytes!`
- **Rationale**: Documentation says "internal utility macro"
- **Purpose**: Const array creation with terminator
- **Recommendation**: Mark with `#[doc(hidden)]`

### 10. `visca_prefix!`
- **Rationale**: Documentation says "internal utility macro"
- **Purpose**: Const array creation without terminator
- **Recommendation**: Mark with `#[doc(hidden)]`

## Recommendations

1. **Keep as public API**: Only `visca_bounded_param!` and `forward_facade!`
2. **Mark as internal**: All command implementation macros
3. **Consider proc-macro migration**: Command macros would benefit from better error messages as proc-macros