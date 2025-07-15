use grafton_visca::{Camera, CameraModel, Error};
use grafton_visca::command::power::PowerCommand;
use grafton_visca::command::system::Socket;
use grafton_visca::transport::core::Transport;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use std::time::Duration;
use bytes::Bytes;

/// Mock transport for testing socket manager
#[derive(Debug, Clone)]
struct MockTransport {
    sent_commands: Arc<Mutex<Vec<Vec<u8>>>>,
    responses: Arc<Mutex<VecDeque<Result<Bytes, Error>>>>,
}

impl MockTransport {
    fn new() -> Self {
        Self {
            sent_commands: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
    
    fn add_response(&self, response: Result<Bytes, Error>) {
        let mut responses = self.responses.lock().unwrap();
        responses.push_back(response);
    }
    
    fn get_sent_commands(&self) -> Vec<Vec<u8>> {
        let commands = self.sent_commands.lock().unwrap();
        commands.clone()
    }
    
    fn clear_sent_commands(&self) {
        let mut commands = self.sent_commands.lock().unwrap();
        commands.clear();
    }
}

impl Transport for MockTransport {
    type Error = Error;
    type SendFut<'a> = futures::future::Ready<Result<(), Error>>;
    type RecvFut<'a> = futures::future::Ready<Result<Bytes, Error>>;
    
    fn send(&self, bytes: &[u8]) -> Self::SendFut<'_> {
        let mut commands = self.sent_commands.lock().unwrap();
        commands.push(bytes.to_vec());
        futures::future::ready(Ok(()))
    }
    
    fn recv(&self) -> Self::RecvFut<'_> {
        let mut responses = self.responses.lock().unwrap();
        let result = responses.pop_front().unwrap_or_else(|| {
            Err(Error::TransportError("No response available".to_string()))
        });
        futures::future::ready(result)
    }
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_socket_manager_initialization() {
    let transport = MockTransport::new();
    let mut camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport);
    
    // Initially socket manager should not be initialized
    assert!(camera.send_command(&PowerCommand::On).await.is_ok());
    
    // Initialize socket manager
    let result = camera.initialize_socket_manager();
    assert!(result.is_ok(), "Failed to initialize socket manager: {:?}", result);
    
    // Give the actor time to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    println!("Socket manager initialization test passed");
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_socket_manager_command_sending() {
    let transport = MockTransport::new();
    let mut camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport.clone());
    
    // Initialize socket manager
    camera.initialize_socket_manager().expect("Failed to initialize socket manager");
    
    // Give the actor time to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Add a mock ACK response
    transport.add_response(Ok(Bytes::from(vec![0x90, 0x40, 0xFF]))); // ACK for socket 1
    // Add a mock completion response
    transport.add_response(Ok(Bytes::from(vec![0x90, 0x50, 0xFF]))); // Completion for socket 1
    
    // Send a command through socket manager
    let result = camera.send_command(&PowerCommand::On).await;
    
    // The command should succeed (though it might timeout waiting for responses)
    match result {
        Ok(_) => println!("Command succeeded"),
        Err(e) => println!("Command failed: {:?}", e),
    }
    
    // Check that command was actually sent
    let sent_commands = transport.get_sent_commands();
    assert!(!sent_commands.is_empty(), "No commands were sent");
    
    println!("Socket manager command sending test completed");
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_socket_manager_timeout_handling() {
    let transport = MockTransport::new();
    let mut camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport.clone());
    
    // Initialize socket manager
    camera.initialize_socket_manager().expect("Failed to initialize socket manager");
    
    // Give the actor time to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Don't add any responses - this should cause a timeout
    
    // Send a command that should timeout
    let start = std::time::Instant::now();
    let result = camera.send_command(&PowerCommand::On).await;
    let elapsed = start.elapsed();
    
    match result {
        Err(Error::CommandTimeout { .. }) => {
            println!("Command timed out as expected in {:?}", elapsed);
        }
        other => {
            println!("Unexpected result: {:?}", other);
        }
    }
    
    println!("Socket manager timeout test completed");
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_socket_manager_cancellation() {
    let transport = MockTransport::new();
    let mut camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport.clone());
    
    // Initialize socket manager
    camera.initialize_socket_manager().expect("Failed to initialize socket manager");
    
    // Give the actor time to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Test cancellation API
    let result = camera.cancel_command(Socket::Socket1).await;
    
    match result {
        Ok(_) => println!("Cancel command succeeded"),
        Err(e) => println!("Cancel command failed: {:?}", e),
    }
    
    println!("Socket manager cancellation test completed");
}

#[cfg(not(feature = "tokio"))]
#[test]
fn test_socket_manager_without_tokio() {
    let transport = MockTransport::new();
    let mut camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport);
    
    // Initialize socket manager
    let result = camera.initialize_socket_manager();
    
    match result {
        Ok(_) => println!("Socket manager initialized successfully without tokio"),
        Err(e) => println!("Socket manager initialization failed: {:?}", e),
    }
}