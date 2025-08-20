# Release Notes - v0.7.1 (Unreleased)

## Overview
This patch release focuses on improving the transport layer architecture, enhancing code quality, and streamlining the runtime system for better performance and maintainability.

## Key Improvements

### 🔧 Transport Layer Enhancements
- **Enhanced TransportBuilder**: Comprehensive connection options with DNS resolution, IPv4/IPv6 support, and configurable timeouts
- **AsyncWrapper Pattern**: New abstraction that reduces code duplication between blocking and async transport implementations
- **Improved Buffer Management**: Cleaner abstractions and better separation of concerns between protocol layers

### 🏗️ Runtime & Protocol Improvements
- Streamlined runtime and protocol modules for better performance
- Enhanced scheduler implementation for more efficient command prioritization
- Improved ACK/Completion sequence handling
- Consolidated envelope tests for better maintainability

### 🐛 Bug Fixes
- Fixed TCP test race condition on Windows
- Resolved missing AsyncTransport import in build_async_wrapper doctest
- Fixed various clippy warnings and formatting issues
- Addressed visibility issues in envelope and protocol modules

### 📚 Code Quality
- Removed redundant TODO comments throughout codebase
- Cleaned up imports and module organization
- Consolidated test utilities for better reusability
- Simplified example code for improved clarity

### 🧪 Testing
- Reduced test suite complexity by removing redundant tests
- Improved test organization with better helper utilities
- Streamlined CI/CD checks for faster builds

## Migration Guide
This is a patch release with no breaking changes. Simply update your dependency:

```toml
[dependencies]
grafton-visca = "0.7.1"
```

## What's Next
- Continuing work on serial transport async support
- Further performance optimizations
- Enhanced documentation and examples

## Contributors
Thank you to all contributors who helped make this release possible!

---

For the complete changelog, see [CHANGELOG.md](./CHANGELOG.md)