# VISCA Testing Guide

This guide explains the comprehensive testing utilities provided by the grafton-visca library and how to use them effectively.

## Overview

The test utilities module (`test_utils`) provides a complete testing framework for VISCA implementations, including:

- **MockTransport**: A highly configurable mock transport for simulating camera behavior
- **ProtocolValidator**: Validates VISCA protocol compliance and timing
- **ResponseBuilder**: Fluent API for building correct VISCA responses
- **ScenarioBuilder**: High-level test scenario creation
- **TestFixtures**: Pre-defined command and response data

## Design Principles

### 1. Transport Abstraction for Testability

The library's transport layer is designed with testing in mind:

```rust
// The transport trait is simple and mockable
pub trait BlockingTransport: Send + Sync + Debug {
    fn send(&mut self, data: &[u8]) -> Result<(), Error>;
    fn receive(&mut self, timeout: Duration) -> Result<Vec<u8>, Error>;
    fn is_connected(&self) -> bool;
    fn description(&self) -> &str;
}
```

This design allows:
- Easy mocking for tests
- Protocol byte inspection
- Network condition simulation
- Error injection

### 2. Dependency Injection

The Camera type accepts any transport implementation:

```rust
let mock = MockTransport::new();
let camera = Camera::<PTZOpticsG2, _>::new(mock);
```

This enables:
- Testing without hardware
- Protocol verification
- Behavior simulation
- Integration testing

## MockTransport

The `MockTransport` is the cornerstone of testing, providing:

### Basic Usage

```rust
use grafton_visca::test_utils::*;

let mut mock = MockTransport::new();

// Set up expectations
mock.expect_command(&[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
    .described_as("power on")
    .will_ack(1)
    .then_complete(1);

// Use with camera
let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());
camera.power_on()?;

// Verify expectations
mock.verify()?;
```

### Advanced Features

```rust
// Simulate network latency
mock.set_latency(Duration::from_millis(50));

// Simulate errors
mock.expect_command(&[0x81, 0x01, 0x04, 0x38, 0x02, 0xFF])
    .will_error(0x41); // Not executable

// Simulate disconnection
mock.disconnect();

// Get command history
let history = mock.sent_history();
assert_eq!(history.len(), 3);
```

### Builder Pattern

```rust
let mock = MockTransportBuilder::new()
    .connected(true)
    .with_latency(Duration::from_millis(10))
    .expect(
        &patterns::power::ON,
        vec![
            MockResponse::Immediate(patterns::responses::ACK_1),
            MockResponse::Delayed(patterns::responses::COMPLETE_1, Duration::from_millis(50)),
        ],
    )
    .build();
```

## Protocol Validation

The `ProtocolValidator` ensures VISCA compliance:

```rust
let mut validator = ProtocolValidator::new(ValidationMode::Strict);

// Validate outgoing command
validator.validate_command(&command)?;

// Validate incoming response
validator.validate_response(&response)?;

// Check protocol state
assert!(validator.all_sockets_free());

// Get summary
let summary = validator.get_summary();
println!("Commands sent: {}", summary.commands_sent);
println!("Responses received: {}", summary.responses_received);
```

### Validation Modes

- **Strict**: Enforces all protocol rules
- **Relaxed**: Allows minor violations
- **Minimal**: Only critical errors

## Response Building

The `ResponseBuilder` provides a fluent API:

```rust
// Simple responses
let ack = ResponseBuilder::ack(1);
let completion = ResponseBuilder::completion(1);
let error = ResponseBuilder::error(0x03); // Buffer full

// Inquiry responses
let zoom_pos = ResponseBuilder::inquiry()
    .add_u16_nibbles(0x4000)
    .build();

// Complex responses
let device_info = ResponseBuilder::inquiry()
    .add_byte(0x00)  // Vendor high
    .add_byte(0x20)  // Vendor low
    .add_byte(0x01)  // Model high
    .add_byte(0x00)  // Model low
    .add_bytes(&[0x01, 0x02, 0x03, 0x04]) // ROM version
    .add_byte(0x02)  // Socket count
    .build();
```

## Test Scenarios

The `ScenarioBuilder` creates complex test scenarios:

```rust
let scenario = ScenarioBuilder::new("Integration Test")
    .description("Test complete camera initialization")
    .expect_power_on()
    .expect_home()
    .expect_preset_recall(1)
    .expect_inquiry(
        vec![0x81, 0x09, 0x04, 0x47, 0xFF],
        patterns::zoom_position_response(0x4000),
    )
    .build();

// Apply to mock transport
let mut mock = MockTransport::new();
scenario.apply_to(&mut mock)?;
```

### Pre-built Scenarios

```rust
// Common scenarios
let init = scenarios::basic_initialization();
let tour = scenarios::preset_tour();
let error = scenarios::error_recovery();
let network = scenarios::network_failure();
```

## Test Fixtures and Generators

### Command Fixtures

```rust
// Get all zoom commands
let zoom_cmds = CommandFixtures::zoom_commands();
for (name, cmd) in zoom_cmds {
    println!("Testing {}", name);
    mock.send(&cmd)?;
}

// Edge cases
let edge_cases = CommandFixtures::edge_case_commands();
```

### Data Generators

```rust
// Generate test data
let zoom_positions = generators::zoom_positions();
let pan_tilt_positions = generators::pan_tilt_positions();
let preset_numbers = generators::preset_numbers();

// Use in property-based testing
for zoom in zoom_positions {
    test_zoom_position(zoom)?;
}
```

## Helper Macros

The library provides convenient macros:

```rust
// Command creation
let power_on = cmd!(0x01, 0x04, 0x00, 0x02);
let inquiry = inq!(0x04, 0x47);

// Response creation
let ack = resp!(ack 1);
let complete = resp!(complete 1);
let error = resp!(error 0x03);
let data = resp!(data 0x04, 0x00, 0x00, 0x00);
```

## Protocol Byte Patterns

Pre-defined patterns for common operations:

```rust
use grafton_visca::test_utils::patterns;

// Commands
patterns::power::ON
patterns::power::STANDBY
patterns::zoom::STOP
patterns::pan_tilt::HOME

// Responses
patterns::responses::ACK_1
patterns::responses::COMPLETE_1
patterns::responses::BUFFER_FULL
patterns::responses::NOT_EXECUTABLE
```

## Testing Best Practices

### 1. Use MockTransport for Unit Tests

```rust
#[test]
fn test_power_on() {
    let mut mock = MockTransport::new();
    mock.expect_command(&patterns::power::ON)
        .will_ack(1)
        .then_complete(1);
    
    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());
    camera.power_on().unwrap();
    
    mock.verify().unwrap();
}
```

### 2. Validate Protocol Compliance

```rust
#[test]
fn test_protocol_compliance() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);
    let command = patterns::power::ON;
    
    validator.validate_command(&command).unwrap();
    // ... send command and receive response ...
    validator.validate_response(&response).unwrap();
    
    assert!(validator.all_sockets_free());
}
```

### 3. Test Error Conditions

```rust
#[test]
fn test_buffer_full() {
    let mut mock = MockTransport::new();
    let mut scenario = ScenarioBuilder::new("Buffer Full");
    scenario.simulate_buffer_full();
    scenario.build().apply_to(&mut mock).unwrap();
    
    // Send three commands quickly
    // Third command should get buffer full error
}
```

### 4. Test Timing Constraints

```rust
#[test]
fn test_preset_timing() {
    let mut mock = MockTransport::new();
    
    // Preset recall needs settling time
    mock.expect_command(&[0x81, 0x01, 0x04, 0x3F, 0x02, 0x01, 0xFF])
        .will_ack(1)
        .then_complete(1);
    
    // Camera needs 240ms after preset on some models
    mock.set_latency(Duration::from_millis(240));
}
```

### 5. Use Scenarios for Integration Tests

```rust
#[test]
fn test_camera_initialization_sequence() {
    let scenario = scenarios::basic_initialization();
    let mut mock = MockTransport::new();
    scenario.apply_to(&mut mock).unwrap();
    
    // Run actual initialization
    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());
    camera.power_on().unwrap();
    camera.pan_tilt_home().unwrap();
    
    mock.verify().unwrap();
}
```

## Testing Different Camera Models

The test utilities support testing model-specific behaviors:

```rust
// Test PTZOptics-specific raw UDP
let mock = MockTransportBuilder::new()
    .no_validation() // PTZOptics uses raw VISCA
    .build();

// Test Sony encapsulated protocol
// (Would need custom mock for header validation)

// Test model-specific timing
match camera_model {
    CameraModel::SonyEVI_H100 => {
        mock.set_latency(Duration::from_millis(240)); // Preset settle time
    }
    _ => {}
}
```

## Debugging Test Failures

The test utilities provide detailed information for debugging:

```rust
// Get command history
let history = mock.sent_history();
for (i, cmd) in history.iter().enumerate() {
    println!("Command {}: {:02X?}", i, cmd);
}

// Protocol validation provides detailed errors
match validator.validate_response(&response) {
    Err(e) => println!("Protocol error: {}", e),
    Ok(()) => {}
}

// Expectations show what was expected vs received
mock.verify().unwrap_or_else(|e| {
    panic!("Expectation not met: {}", e);
});
```

## Future Improvements

Potential enhancements to the testing framework:

1. **Property-based testing**: Integration with proptest/quickcheck
2. **Fuzzing support**: For protocol robustness testing  
3. **Performance benchmarks**: For timing-critical operations
4. **Multi-camera scenarios**: For daisy-chain testing
5. **Record/replay**: Capture real camera sessions for testing

## Conclusion

The grafton-visca test utilities provide a comprehensive framework for testing VISCA implementations without hardware. By using MockTransport, ProtocolValidator, and the various builders and fixtures, you can ensure your VISCA code is robust, compliant, and handles edge cases correctly.

The transport abstraction and dependency injection patterns make the library inherently testable, while the rich set of test utilities make writing tests efficient and maintainable.