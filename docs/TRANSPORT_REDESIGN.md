# Transport API Redesign Proposal

## Core Design Principles

1. **Blocking-first**: All transports must implement blocking I/O
2. **Async-optional**: Async support via feature flag and additional trait
3. **Simple trait**: Minimal methods for easy implementation
4. **Zero overhead**: No runtime required for blocking usage

## Proposed API

```rust
// Core blocking transport trait - everyone implements this
pub trait Transport: Send + Sync + Debug {
    /// Send raw bytes (blocking)
    fn send(&mut self, data: &[u8]) -> Result<(), Error>;
    
    /// Receive raw bytes (blocking)
    fn receive(&mut self) -> Result<Vec<u8>, Error>;
    
    /// Check if connected
    fn is_connected(&self) -> bool;
    
    /// Get description
    fn description(&self) -> &str;
}

// Optional async extension trait
#[cfg(feature = "async")]
pub trait AsyncTransport: Transport {
    /// Send raw bytes (async)
    fn send_async<'a>(&'a mut self, data: &'a [u8]) 
        -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;
    
    /// Receive raw bytes (async)  
    fn receive_async<'a>(&'a mut self)
        -> Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;
}

// ViscaTransport handles protocol for any Transport
pub struct ViscaTransport<T: Transport> {
    transport: T,
    // ... protocol state
}

impl<T: Transport> ViscaTransport<T> {
    /// Blocking send command
    pub fn send_command(&mut self, cmd: &dyn Command) -> Result<Response, Error> {
        // Handle VISCA protocol using transport.send() and transport.receive()
    }
}

// Async extension for ViscaTransport
#[cfg(feature = "async")]
impl<T: AsyncTransport> ViscaTransport<T> {
    /// Async send command
    pub async fn send_command_async(&mut self, cmd: &dyn Command) -> Result<Response, Error> {
        // Handle VISCA protocol using transport.send_async() and transport.receive_async()
    }
}
```

## Implementation Examples

```rust
// Serial is blocking-only
pub struct SerialTransport {
    port: Box<dyn SerialPort>,
}

impl Transport for SerialTransport {
    fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        self.port.write_all(data)?;
        Ok(())
    }
    
    fn receive(&mut self) -> Result<Vec<u8>, Error> {
        // Read until terminator
    }
}

// TCP supports both blocking and async
pub struct TcpTransport {
    #[cfg(not(feature = "async"))]
    stream: std::net::TcpStream,
    #[cfg(feature = "async")]
    stream: tokio::net::TcpStream,
}

impl Transport for TcpTransport {
    fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        #[cfg(not(feature = "async"))]
        {
            use std::io::Write;
            self.stream.write_all(data)?;
        }
        #[cfg(feature = "async")]
        {
            // Use block_on or error
            todo!("Decide how to handle blocking call on async stream")
        }
        Ok(())
    }
}

#[cfg(feature = "async")]
impl AsyncTransport for TcpTransport {
    fn send_async<'a>(&'a mut self, data: &'a [u8]) -> ... {
        Box::pin(async move {
            self.stream.write_all(data).await?;
            Ok(())
        })
    }
}
```

## Usage Examples

```rust
// Blocking usage (no runtime needed!)
let transport = SerialTransport::new("/dev/ttyUSB0")?;
let mut visca = ViscaTransport::new(transport);
let response = visca.send_command(&PowerOn)?;

// Async usage (with tokio feature)
#[cfg(feature = "async")]
{
    let transport = TcpTransport::connect_async("192.168.1.100:5678").await?;
    let mut visca = ViscaTransport::new(transport);
    let response = visca.send_command_async(&PowerOn).await?;
}
```

## Benefits

1. **Simple blocking usage** - No runtime needed for serial/simple UDP
2. **Clean async support** - When you need it, with proper feature flag
3. **Easy to implement** - Just 4 methods for basic transport
4. **Flexible** - Can support both modes or just blocking
5. **Type safe** - Async methods only available on async transports