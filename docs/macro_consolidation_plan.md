# Macro Consolidation Plan

## Overview

This document outlines a high-quality approach to consolidating VISCA macros that maintains usability while improving organization.

## Current Problems

1. Macros scattered across two modules (`src/macros.rs` and `src/command/const_encoding/macros.rs`)
2. All macros use `#[macro_export]` making them available at crate root, regardless of intended visibility
3. No clear separation between public API and internal implementation details
4. Potential for symbol duplication and namespace pollution

## Proposed Solution

### 1. Module Reorganization

Create a clear hierarchy in `src/macros/`:

```
src/macros/
├── mod.rs           # Module documentation and re-exports
├── public.rs        # Public API macros
├── internal.rs      # Internal implementation macros
└── test_utils.rs    # Test-only macros
```

### 2. Macro Categories

#### Public API Macros (in `public.rs`)
These remain `#[macro_export]` and form part of the stable API:

- `visca_bounded_param!` - Users need this to create custom parameter types
- `forward_facade!` - Essential for extending the camera API

#### Internal Implementation Macros (in `internal.rs`)
Remove `#[macro_export]` and use `macro_rules!` with `pub(crate)` visibility:

- `visca_command!` - Command enum generator
- `visca_bool_command!` - Boolean command generator
- `visca_builder!` - Builder command generator
- `visca_param_command!` - Parameter command generator
- `visca_const_command!` - Constant command generator
- `visca_bytes!` - Const array with terminator
- `visca_prefix!` - Const array without terminator

#### Test Utilities (in `test_utils.rs`)
Keep in `#[cfg(test)]` module:

- `visca_test!` - Test case generator

### 3. Migration Strategy

1. **Phase 1: Create new module structure**
   - Create `src/macros/mod.rs` with proper documentation
   - Move macros to appropriate submodules
   - Update imports throughout codebase

2. **Phase 2: Adjust visibility**
   - Remove `#[macro_export]` from internal macros
   - Use `pub(crate) use` in `src/macros/mod.rs` for internal macros
   - Keep `#[macro_export]` only for public API macros

3. **Phase 3: Update usage sites**
   - Change from `use crate::macro_name` to `use crate::macros::macro_name`
   - Ensure all command modules can access internal macros

### 4. Benefits

1. **Clear API boundary**: Users know which macros are stable API
2. **No namespace pollution**: Internal macros don't appear at crate root
3. **Better documentation**: Module structure reflects intended usage
4. **Easier maintenance**: All macros in one logical location
5. **Future-proof**: Easy to convert internal macros to proc-macros later

### 5. Alternative Consideration

For Phase 3 of the issue (future work), consider converting command generation macros to derive macros:

```rust
#[derive(ViscaCommand)]
#[visca(category = "Movement")]
enum PanTilt {
    #[visca(bytes = [0x81, 0x01, 0x06, 0x04, 0xFF])]
    Home,
    #[visca(bytes = [0x81, 0x01, 0x06, 0x05, 0xFF])]
    Reset,
}
```

This would provide:
- Better error messages
- IDE support
- Compile-time validation
- Cleaner syntax

## Implementation Checklist

- [ ] Create `src/macros/` directory structure
- [ ] Move macros to appropriate files
- [ ] Adjust macro visibility
- [ ] Update all import statements
- [ ] Add comprehensive module documentation
- [ ] Test that all commands still compile
- [ ] Update CLAUDE.md with macro usage guide