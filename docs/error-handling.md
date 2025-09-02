# VISCA Error Handling & Retry Policy

This document describes the error handling and retry strategies in the grafton-visca library.

## Error Code Mappings

The VISCA protocol defines specific error codes that are returned by cameras when commands fail. These are mapped to the `Error` enum in `src/error.rs`:

### Standard VISCA Error Codes

| VISCA Code | Error Variant | Description | Retryable |
|------------|---------------|-------------|-----------|
| `0x01` | `MessageLengthError` | Message length is incorrect | No |
| `0x02` | `SyntaxError` | Command format is incorrect or parameters are illegal | No |
| `0x03` | `CommandBufferFull` | Two sockets are already in use | **Yes** |
| `0x04` | `CommandCanceled` | Command was canceled in the specified socket | No |
| `0x05` | `NoSocket` | No command is executing in the specified socket | No |
| `0x41` | `CommandNotExecutable` | Command cannot be executed due to current conditions | No |

### Extended Error Conditions

Beyond the standard VISCA error codes, the library defines additional error conditions:

| Error Variant | Description | Retryable | Suggested Delay |
|---------------|-------------|-----------|-----------------|
| `CameraBusy` | Camera is executing another command | **Yes** | 100ms |
| `CommandPending` | Command acknowledged but pending completion | **Yes** | 50ms |
| `CameraMoving` | Camera is performing mechanical movement | **Yes** | 500ms |
| `CommandTimeout` | Command exceeded configured timeout | **Yes** | 1s |
| `Timeout` | Operation timed out | **Yes** | 2s |
| `ConnectionLost` | Connection to camera was lost | No | - |
| `InvalidParameter` | Invalid parameter provided | No | - |
| `FeatureNotSupported` | Feature not supported by camera model | No | - |

## Retry Policy

### Architecture Difference

**Important:** Retry behavior differs between async and blocking modes:
- **Async mode with runtime**: Automatic retry handled by the runtime layer
- **Blocking mode**: No automatic retry - must be implemented by the application

The runtime layer in async mode provides protocol-compliant retry logic with exponential backoff.

### Automatic Retry (Async Mode)

When using the async API, the runtime layer handles automatic retry based on VISCA protocol rules:

```rust
// Retry configuration in the runtime
pub struct RetryConfig {
    max_attempts: 3,              // Maximum retry attempts
    base_delay: Duration::from_millis(100),  // Base delay
    max_delay: Duration::from_secs(5),       // Maximum delay
    jitter: true,                 // Add randomization to delays
}
```

#### Retryable Errors

The runtime automatically retries these VISCA protocol errors:
- `CommandBufferFull` (0x03) - Camera command buffer is full
- `CameraBusy` - Camera is processing another command
- `CommandPending` - Command acknowledged but pending completion
- `CameraMoving` - Camera is performing mechanical movement
- `CommandTimeout` - Command exceeded configured timeout
- `Timeout` - Operation timed out

Retry behavior is command-category aware:
- Movement commands: Always retryable when busy
- Quick commands: Limited retries to avoid blocking
- Inquiry commands: No retry (immediate response expected)

#### Exponential Backoff

Retry delays use exponential backoff with jitter:
- Attempt 0: 100ms
- Attempt 1: 200ms
- Attempt 2: 400ms
- Maximum delay capped at 5 seconds

### Manual Retry (Blocking Mode)

In blocking mode, retry must be handled manually by the application:

```rust
use grafton_visca::Error;
use std::{thread, time::Duration};

fn execute_with_retry<F, T>(mut f: F, max_attempts: u32) -> Result<T, Error>
where
    F: FnMut() -> Result<T, Error>,
{
    let mut attempt = 0;
    loop {
        match f() {
            Ok(result) => return Ok(result),
            Err(e) if e.is_retryable() && attempt < max_attempts => {
                if let Some(delay) = e.suggested_retry_delay() {
                    thread::sleep(delay);
                }
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}
```

## Command Timeouts

Commands are categorized by expected duration:

| Category | Default Timeout | Example Commands |
|----------|-----------------|------------------|
| `Quick` | 2 seconds | Inquiry, power status |
| `Movement` | 10 seconds | Pan/tilt/zoom operations |
| `Preset` | 60 seconds | Preset recall/save |
| `LongRunning` | 5 minutes | Preset discovery |
| `Network` | 2 seconds | Multicast/NDI settings |
| `Custom` | 30 seconds | User-defined commands |

### Timeout Configuration

```rust
use grafton_visca::timeout::TimeoutConfig;
use std::time::Duration;

// Custom timeout configuration
let config = TimeoutConfig::builder()
    .quick_timeout(Duration::from_secs(1))
    .movement_timeout(Duration::from_secs(15))
    .preset_timeout(Duration::from_secs(90))
    .build();
```

## Error Recovery Strategies

### Connection Errors

For connection-related errors (`ConnectionFailed`, `ConnectionLost`):
1. Attempt to re-establish connection
2. Reset socket manager if using async mode
3. Clear any pending commands

### Command Errors

For command execution errors:
1. Check `is_retryable()` to determine if retry is appropriate
2. Use `suggested_retry_delay()` for optimal retry timing
3. Consider camera state before retrying

### Buffer Full Errors

When encountering `CommandBufferFull`:
1. Wait for current commands to complete
2. Use exponential backoff for retry attempts
3. Consider reducing command rate

## Best Practices

1. **Always check `is_retryable()`** before implementing retry logic
2. **Respect suggested delays** to avoid overwhelming the camera
3. **Limit retry attempts** to prevent infinite loops
4. **Log retry attempts** for debugging and monitoring
5. **Consider camera model** when handling feature-related errors
6. **Use appropriate timeouts** based on command category

## Example: Robust Command Execution

```rust
use grafton_visca::{Error, Result};
use grafton_visca::camera::{Camera, AsyncMode, BlockingMode};
use grafton_visca::camera::controls::pan_tilt::{PanTiltControl, PanTiltControlBlocking};
use grafton_visca::camera::profiles::Profile;
use grafton_visca::transport::{AsyncTransport, BlockingTransport};
use grafton_visca::runtime::executor::Executor;

// Async mode - automatic retry by runtime
async fn execute_command_robust<P, T, E>(
    camera: &Camera<AsyncMode, P, T, E>
) -> Result<()>
where
    P: Profile,
    T: AsyncTransport,
    E: Executor,
{
    // Runtime handles retry automatically based on protocol rules
    camera.pan_tilt_home().await?;
    Ok(())
}

// Blocking mode - manual retry required
fn execute_command_robust_blocking<P, T>(
    camera: &Camera<BlockingMode, P, T, ()>
) -> Result<()>
where
    P: Profile,
    T: BlockingTransport,
{
    let mut attempts = 0;
    const MAX_ATTEMPTS: u32 = 3;

    loop {
        match camera.pan_tilt_home() {
            Ok(()) => return Ok(()),
            Err(e) if e.is_retryable() && attempts < MAX_ATTEMPTS => {
                if let Some(delay) = e.suggested_retry_delay() {
                    std::thread::sleep(delay);
                }
                attempts += 1;
                log::debug!("Retrying command, attempt {}/{}", attempts, MAX_ATTEMPTS);
            }
            Err(e) => return Err(e),
        }
    }
}
```

## Priority-Based Retry

The async runtime uses command priorities to manage retries:

```rust
// Priority levels (lowest to highest value)
pub enum Priority {
    Low = 0,         // Normal operations
    Normal = 1,      // Default priority
    High = 2,        // User-initiated actions
    Critical = 3,    // Emergency/safety operations
}
```

Higher priority commands can preempt retry attempts of lower priority commands when the camera is busy.
