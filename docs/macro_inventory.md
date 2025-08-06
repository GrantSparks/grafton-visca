# VISCA Macro Inventory

## Overview

This document provides a comprehensive inventory of all macros in the grafton-visca codebase, their purpose, usage statistics, and visibility status.

## Macro Locations

1. **`src/macros.rs`** - Main macro module (private module, but macros are `#[macro_export]`)
2. **`src/command/const_encoding/macros.rs`** - Const encoding macros (public submodule)

## Detailed Macro Inventory

### 1. `visca_command!`
- **Location**: `src/macros.rs:33-103`
- **Purpose**: Create simple command enums with byte sequences
- **Visibility**: `#[macro_export]` - Available at crate root
- **Total Usage**: 13 files, ~15 invocations
- **Usage by file**:
  - `exposure.rs`: 2
  - `flip.rs`: 3
  - `image.rs`: 2
  - `power.rs`: 1
  - `tally.rs`: 1
  - `focus.rs`: 1
  - `macros.rs`: 2 (examples)
- **Public API**: YES - Used extensively in command implementations
- **Dependencies**: Uses `EncodeVisca` trait, `Error`, `CameraId`

### 2. `visca_bounded_param!`
- **Location**: `src/macros.rs:122-178`
- **Purpose**: Create validated newtype wrappers for numeric parameters
- **Visibility**: `#[macro_export]` - Available at crate root
- **Total Usage**: 5 files, ~5 invocations
- **Usage by file**:
  - `zoom.rs`: 1
  - `preset.rs`: 1
  - `focus.rs`: 1
  - `macros.rs`: 2 (examples)
- **Public API**: YES - Part of public API, used in examples
- **Dependencies**: Uses `Error` type

### 3. `visca_bool_command!`
- **Location**: `src/macros.rs:209-289`
- **Purpose**: Create boolean on/off commands
- **Visibility**: `#[macro_export]` - Available at crate root
- **Total Usage**: 11 files, ~12 invocations
- **Usage by file**:
  - `image.rs`: 2
  - `streaming.rs`: 1
  - `nd_filter.rs`: 1
  - `menu.rs`: 1
  - `macros.rs`: 6 (tests/examples)
- **Public API**: YES - Used in command implementations
- **Dependencies**: Uses `EncodeVisca` trait, `Error`, `CameraId`, `ResponseType`

### 4. `visca_test!`
- **Location**: `src/macros.rs:292-308`
- **Purpose**: Internal test generation macro
- **Visibility**: `#[macro_export]` with `#[doc(hidden)]`
- **Total Usage**: 20 files, ~170+ invocations
- **Usage by file**:
  - `inquiry_structs.rs`: 33
  - `exposure.rs`: 43
  - `image.rs`: 23
  - `image_adjustment.rs`: 14
  - `menu.rs`: 10
  - `nd_filter.rs`: 8
  - `motion_sync.rs`: 7
  - `white_balance.rs`: 7
  - `focus.rs`: 7
  - `streaming.rs`: 6
  - `system.rs`: 4
  - `pan_tilt.rs`: 4
  - `preset.rs`: 3
  - `gain.rs`: 3
  - `power.rs`: 2
  - `flip.rs`: 2
  - `variable_speed.rs`: 2
  - `const_encoding/constants.rs`: 2
  - `color.rs`: 1
- **Public API**: NO - Internal test utility
- **Dependencies**: Uses `CameraId`, `EncodeVisca`

### 5. `visca_builder!`
- **Location**: `src/macros.rs:330-391`
- **Purpose**: Create builder-style commands with dynamic byte sequences
- **Visibility**: `#[macro_export]` - Available at crate root
- **Total Usage**: 9 files, ~15 invocations
- **Usage by file**:
  - `color.rs`: 4
  - `focus.rs`: 3
  - `menu.rs`: 2
  - `image_adjustment.rs`: 2
  - `gain.rs`: 1
  - `nd_filter.rs`: 1
  - `exposure.rs`: 1
  - `macros.rs`: 1 (example)
- **Public API**: YES - Used in command implementations
- **Dependencies**: Uses `EncodeVisca`, `Error`, `CameraId`, `CommandBuilder`

### 6. `visca_param_command!`
- **Location**: `src/macros.rs:423-581`
- **Purpose**: Create single-byte parameter commands
- **Visibility**: `#[macro_export]` - Available at crate root
- **Total Usage**: 14 files, ~16 invocations
- **Usage by file**:
  - `macros.rs`: 6 (tests/examples)
  - `image.rs`: 2
  - `nd_filter.rs`: 2
  - `white_balance.rs`: 2
  - `menu.rs`: 1
  - `streaming.rs`: 1
  - `system.rs`: 1
  - `exposure.rs`: 1
- **Public API**: YES - Used in command implementations
- **Dependencies**: Uses `EncodeVisca`, `Error`, `CameraId`, `ResponseType`

### 7. `forward_facade!`
- **Location**: `src/macros.rs:601-648`
- **Purpose**: Forward method calls from wrapper types to inner camera instance
- **Visibility**: `#[macro_export]` - Available at crate root
- **Total Usage**: 3 files, 5 invocations
- **Usage by file**:
  - `blocking.rs`: 2
  - `macros.rs`: 3 (examples)
- **Public API**: YES - Essential for facade pattern in blocking/async APIs
- **Dependencies**: Uses profile/transport traits

### 8. `visca_const_command!`
- **Location**: `src/macros.rs:674-759`
- **Purpose**: Create simple constant byte commands
- **Visibility**: `#[macro_export]` - Available at crate root
- **Total Usage**: 6 files, ~9 invocations
- **Usage by file**:
  - `macros.rs`: 3 (examples)
  - `const_encoding/constants.rs`: 3
  - `system.rs`: 2
  - `color.rs`: 1
- **Public API**: YES - Used in command implementations
- **Dependencies**: Uses `EncodeVisca`, `Error`, `CameraId`, `ResponseType`

### 9. `visca_bytes!`
- **Location**: `src/command/const_encoding/macros.rs:21-30`
- **Purpose**: Create compile-time VISCA arrays with terminator
- **Visibility**: `#[macro_export]` - Available at crate root
- **Total Usage**: 2 files, 44 invocations
- **Usage by file**:
  - `const_encoding/constants.rs`: 43
  - `const_encoding/macros.rs`: 1 (example)
- **Public API**: NO - Internal utility (marked as internal in docs)
- **Dependencies**: None (pure const evaluation)

### 10. `visca_prefix!`
- **Location**: `src/command/const_encoding/macros.rs:46-51`
- **Purpose**: Create VISCA prefixes without terminator
- **Visibility**: `#[macro_export]` - Available at crate root
- **Total Usage**: 2 files, 74 invocations
- **Usage by file**:
  - `const_encoding/constants.rs`: 73
  - `const_encoding/macros.rs`: 1 (example)
- **Public API**: NO - Internal utility (marked as internal in docs)
- **Dependencies**: None (pure const evaluation)

## Macro Dependencies

### Direct Dependencies:
- `visca_command!` → `EncodeVisca`, `Error`, `CameraId`, `ResponseType`, `CommandCategory`
- `visca_bounded_param!` → `Error`
- `visca_bool_command!` → `EncodeVisca`, `Error`, `CameraId`, `ResponseType`, `CommandCategory`
- `visca_test!` → `CameraId`, `EncodeVisca`
- `visca_builder!` → `EncodeVisca`, `Error`, `CameraId`, `CommandBuilder`, `CommandCategory`
- `visca_param_command!` → `EncodeVisca`, `Error`, `CameraId`, `ResponseType`, `CommandCategory`
- `forward_facade!` → Profile/Transport traits
- `visca_const_command!` → `EncodeVisca`, `Error`, `CameraId`, `ResponseType`, `CommandCategory`
- `visca_bytes!` → None
- `visca_prefix!` → None

### Macro Interdependencies:
- Some macros call other macros internally (e.g., `visca_bool_command!` and `visca_param_command!` have multiple forms)
- No circular dependencies detected

## Summary Statistics

- **Total macros**: 10
- **Public API macros**: 8
- **Internal utility macros**: 2 (`visca_test!`, though technically both `visca_bytes!` and `visca_prefix!` are documented as internal)
- **Most used macro**: `visca_test!` (170+ invocations)
- **Least used macros**: `visca_bounded_param!` (5 invocations), `forward_facade!` (5 invocations)

## Issues Identified

1. **Scattered definitions**: Macros in two separate modules
2. **Inconsistent visibility documentation**: Some macros marked internal in docs but still exported
3. **`visca_test!` is `#[doc(hidden)]`** but still `#[macro_export]`
4. **No clear separation** between public API and internal utilities
5. **Module structure**: Main macros in private module but exported, const macros in public module