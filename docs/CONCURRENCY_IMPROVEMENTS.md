# Camera Concurrency Improvements

This document describes the improvements made to the camera concurrency control system, which uses a semaphore in conjunction with session socket management.

## Overview

The VISCA protocol supports up to 2 concurrent commands through separate "sockets" (command slots). The `Camera` struct uses:
- A `Semaphore` with 2 permits to limit concurrent command execution
- A `Session` that manages the allocation of the 2 VISCA sockets

## New Features

### 1. Pre-check Logging

Before attempting to acquire a semaphore permit, the camera now checks if all sockets are in use and logs an informative message:

```rust
// Check if all sockets are in use before acquiring semaphore
{
    let session = self.session.lock();
    if session.is_full() {
        log::debug!("All VISCA sockets are in use (2/2), command will wait");
    }
}
```

This helps with debugging and understanding when commands are being queued.

### 2. Better Error Handling

The code now includes explicit error handling for the case where `CommandBufferFull` is returned despite having a semaphore permit (which should never happen):

```rust
match session.assign_socket(command.response_type()) {
    Ok(id) => id,
    Err(ViscaError::CommandBufferFull) => {
        log::error!("CommandBufferFull despite having semaphore permit - this should not happen");
        return Err(ViscaError::CommandBufferFull);
    }
    Err(e) => return Err(e),
}
```

### 3. Camera Readiness Methods

Three new public methods have been added to the `Camera` struct:

#### `is_ready() -> bool`
Checks if the camera can accept a new command without blocking.

```rust
if camera.is_ready() {
    // Camera can accept a command immediately
} else {
    // All sockets are in use, next command will wait
}
```

#### `pending_commands() -> usize`
Returns the number of currently pending commands (0-2).

```rust
let pending = camera.pending_commands();
println!("Commands in flight: {}/2", pending);
```

#### `try_reserve() -> Result<(), ViscaError>`
Attempts to check if a command slot is available without actually reserving it.

```rust
match camera.try_reserve() {
    Ok(()) => println!("Camera ready for commands"),
    Err(ViscaError::CommandBufferFull) => println!("Camera busy"),
    Err(e) => println!("Other error: {}", e),
}
```

### 4. Enhanced Session Logging

The `Session::assign_socket` method now provides more detailed logging:

```rust
debug!("Assigned {socket_id} for new command (type: {:?})", response_type);

// When both sockets are full:
debug!("Cannot assign socket - both sockets are in use:");
for (socket_id, cmd) in &self.pending_commands {
    debug!("  {socket_id}: {:?}, acknowledged: {}", cmd.response_type, cmd.acknowledged);
}
```

## Usage Example

```rust
use grafton_visca::{Camera, GenericVisca, transport::UnifiedTransport};

#[tokio::main]
async fn main() -> Result<()> {
    let transport = UnifiedTransport::create_udp("192.168.1.100:52381")?;
    let camera = Camera::<GenericVisca>::new(transport);

    // Check if camera is ready before sending commands
    if camera.is_ready() {
        println!("Camera ready - {} commands pending", camera.pending_commands());
    } else {
        println!("Camera busy - all sockets in use");
    }

    // Send command (will wait if necessary)
    camera.send_raw_async(&SomeCommand).await?;

    Ok(())
}
```

## Implementation Details

### Semaphore and Session Coordination

1. The semaphore ensures no more than 2 commands are processed concurrently
2. The session manages which of the 2 VISCA sockets each command uses
3. Both must be acquired/assigned for a command to proceed

### Thread Safety

- For async code, `try_lock()` is used in the readiness methods to avoid blocking
- For blocking code, regular `lock()` is used since blocking is acceptable
- All shared state is protected by appropriate synchronization primitives

### Clone Support

The `Camera` struct now implements `Clone` when the profile type is `Clone`:

```rust
impl<P: CameraProfile + Clone> Clone for Camera<P> {
    fn clone(&self) -> Self {
        Self {
            profile: self.profile.clone(),
            transport: self.transport.clone(),
            session: self.session.clone(),
            semaphore: self.semaphore.clone(),
        }
    }
}
```

This allows sharing cameras across threads/tasks using `Arc<Camera<P>>` or direct cloning.

## Benefits

1. **Better Debugging**: Clear log messages when commands are queued
2. **Proactive Checking**: Applications can check readiness before attempting commands
3. **Improved Error Messages**: More context when concurrency limits are hit
4. **Thread-Safe Status**: Safe concurrent access to camera state information

## Testing

See `tests/camera_concurrency_tests.rs` for examples of testing the concurrency behavior.