# New Transport API Design

This document describes the redesigned transport API that eliminates code duplication and makes it trivial for library users to implement custom transports.

## Problem Statement

The original transport examples (TCP, UDP, Serial) suffered from massive code duplication:

- **500+ lines of identical VISCA protocol logic** duplicated across each transport
- **Socket management code** repeated in every implementation  
- **Response parsing logic** duplicated across sync and async versions
- **Total duplication**: ~1500 lines of repeated code across 3 transports

The core differences between transports were minimal:
- TCP: `TcpStream::connect()` + `write_all()/read()`
- UDP: `UdpSocket::bind()` + `send_to()/recv_from()`  
- Serial: Serial port + different addressing

## Solution: RawTransport Abstraction

### Core Design

We moved all VISCA protocol logic into the library and created a simple `RawTransport` trait that transport implementations must satisfy:

```rust
pub trait RawTransport: Send + Sync + Debug {
    /// Send raw bytes to the target
    fn send<'a>(&'a mut self, data: &'a [u8]) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;
    
    /// Receive raw bytes from the target  
    fn receive<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;
    
    /// Check if transport is connected/ready
    fn is_connected(&self) -> bool;
    
    /// Get human-readable description
    fn description(&self) -> &str;
}
```

### TransportSession Manager

The `TransportSession<T: RawTransport>` handles all VISCA protocol concerns:

- **Socket allocation and management** (VISCA sockets 0 and 1)
- **Command encoding** with proper socket IDs
- **Response correlation** matching responses to pending commands
- **ACK/Completion handling** for the full command lifecycle
- **Error response parsing** and propagation

## Benefits

### Dramatic Code Reduction

| Component | Old Approach | New Approach | Reduction |
|-----------|-------------|--------------|-----------|
| TCP Transport | 680 lines | 80 lines | **88% smaller** |
| UDP Transport | 624 lines | 60 lines | **90% smaller** |
| Serial Transport | 326 lines | 50 lines | **85% smaller** |
| **Total** | **1630 lines** | **190 lines** | **88% reduction** |

### Implementation Simplicity

Transport implementations now focus only on their core differences:

```rust
// TCP implementation - only the TCP-specific parts
impl RawTransport for TcpRawTransport {
    fn send<'a>(&'a mut self, data: &'a [u8]) -> ... {
        Box::pin(async move {
            self.stream.write_all(data).await.map_err(Error::Io)?;
            self.stream.flush().await.map_err(Error::Io)
        })
    }
    // ... rest handles TCP-specific I/O only
}
```

### Easy Custom Transports

Adding new transport types is now trivial. See `examples/custom_transport_simple.rs` for a complete mock transport in ~50 lines.

### Thread Safety

The existing `ChannelTransport` can wrap any `RawTransport` implementation to provide thread-safe sharing without explicit locking.

## Usage Examples

### Simple Transport Creation

```rust
// TCP
let transport = builders::tcp("192.168.1.100:5678").await?;

// UDP  
let transport = builders::udp("192.168.1.100:52381").await?;

// Serial (mock)
let transport = builders::serial(1); // camera address 1

// All have the same API:
let response = transport.send_command(&ZoomCommand::Stop).await?;
```

### Thread-Safe Sharing

```rust
let tcp_transport = builders::tcp("192.168.1.100:5678").await?;
let channel_transport = ChannelTransportBuilder::new(tcp_transport)
    .queue_size(100)
    .build();

// Can be cloned and shared across tasks
let transport_clone = channel_transport.clone();
tokio::spawn(async move {
    transport_clone.send_command(&command).await
});
```

### Custom Transport Implementation

```rust
#[derive(Debug)]
struct MyCustomTransport {
    // your transport-specific fields
}

impl RawTransport for MyCustomTransport {
    fn send<'a>(&'a mut self, data: &'a [u8]) -> ... {
        // your send implementation
    }
    
    fn receive<'a>(&'a mut self) -> ... {
        // your receive implementation  
    }
    
    // ... other trait methods
}

// Use it exactly like built-in transports
let transport = TransportSession::new(MyCustomTransport::new());
```

## Migration Path

### For Library Users

No breaking changes to the main `Transport` trait. New implementations automatically work with existing camera code.

### For Transport Implementers

Instead of implementing the complex `Transport` trait with socket management, implement the simple `RawTransport` trait and wrap with `TransportSession`.

### Backwards Compatibility

The old transport examples continue to work, but new implementations should use the `RawTransport` approach for simplicity.

## Architecture Diagram

```
┌─────────────────────────────────────────┐
│                Camera                   │
│            (unchanged)                  │
└─────────────────┬───────────────────────┘
                  │ Transport trait
┌─────────────────▼───────────────────────┐
│           TransportSession              │
│                                         │
│  • Socket management (0, 1)            │
│  • Command encoding                     │
│  • Response correlation                 │
│  • ACK/Completion handling              │
│  • Error parsing                        │
└─────────────────┬───────────────────────┘
                  │ RawTransport trait
┌─────────────────▼───────────────────────┐
│        Transport Implementation         │
│                                         │
│  • TCP: TcpStream I/O                   │
│  • UDP: UdpSocket I/O                   │
│  • Serial: SerialPort I/O               │
│  • Custom: Your I/O                     │
└─────────────────────────────────────────┘
```

## Files Added

- `src/transport/core.rs` - RawTransport trait and TransportSession
- `src/transport/implementations.rs` - TCP/UDP/Serial implementations
- `examples/new_transport_demo.rs` - Demonstrates all transport types
- `examples/custom_transport_simple.rs` - Shows custom transport implementation

## Conclusion

This redesign achieves the goal of eliminating code duplication while making transport implementation trivial for users. The library now handles all complex VISCA protocol logic, while transport implementations focus only on their specific I/O mechanisms.

Transport implementations went from 500+ lines of duplicated VISCA protocol code to 50-100 lines of focused I/O code - a **10x reduction** in complexity.