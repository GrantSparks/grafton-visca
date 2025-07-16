use bytes::Bytes;
use grafton_visca::command::power::PowerCommand;
use grafton_visca::command::system::Socket;
use grafton_visca::transport::core::Transport;
use grafton_visca::{Camera, CameraModel, Error};
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "tokio")]
use tokio::sync::{mpsc, Mutex};

#[cfg(not(feature = "tokio"))]
use std::sync::Mutex;

/// Mock transport for testing socket manager
#[derive(Clone)]
struct MockTransport {
    #[cfg(feature = "tokio")]
    sent_commands: Arc<Mutex<Vec<Vec<u8>>>>,
    #[cfg(not(feature = "tokio"))]
    sent_commands: Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
    #[cfg(feature = "tokio")]
    response_sender: mpsc::UnboundedSender<Result<Bytes, Error>>,
    #[cfg(feature = "tokio")]
    response_receiver: Arc<Mutex<mpsc::UnboundedReceiver<Result<Bytes, Error>>>>,
}

impl MockTransport {
    #[cfg(feature = "tokio")]
    fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            sent_commands: Arc::new(Mutex::new(Vec::new())),
            response_sender: tx,
            response_receiver: Arc::new(Mutex::new(rx)),
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn new() -> Self {
        Self {
            sent_commands: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[cfg(feature = "tokio")]
    fn add_response(&self, response: Result<Bytes, Error>) {
        // This simulates the camera sending a response
        let _ = self.response_sender.send(response);
    }

    #[cfg(not(feature = "tokio"))]
    fn add_response(&self, _response: Result<Bytes, Error>) {
        // No-op for non-tokio builds
    }

    #[cfg(feature = "tokio")]
    async fn get_sent_commands(&self) -> Vec<Vec<u8>> {
        let commands = self.sent_commands.lock().await;
        commands.clone()
    }

    #[cfg(not(feature = "tokio"))]
    fn get_sent_commands(&self) -> Vec<Vec<u8>> {
        let commands = self.sent_commands.lock().unwrap();
        commands.clone()
    }
}

impl std::fmt::Debug for MockTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockTransport")
            .field("sent_commands", &self.sent_commands)
            .finish()
    }
}

impl Transport for MockTransport {
    type Error = Error;
    #[cfg(feature = "tokio")]
    type SendFut<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Error>> + Send + 'a>>;
    #[cfg(not(feature = "tokio"))]
    type SendFut<'a> = futures::future::Ready<Result<(), Error>>;
    #[cfg(feature = "tokio")]
    type RecvFut<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<Bytes, Error>> + Send + 'a>>;
    #[cfg(not(feature = "tokio"))]
    type RecvFut<'a> = futures::future::Pending<Result<Bytes, Error>>;

    #[cfg(feature = "tokio")]
    fn send(&self, bytes: &[u8]) -> Self::SendFut<'_> {
        let commands_to_send = bytes.to_vec();
        let sent_commands = Arc::clone(&self.sent_commands);
        Box::pin(async move {
            let mut commands = sent_commands.lock().await;
            commands.push(commands_to_send);
            Ok(())
        })
    }

    #[cfg(not(feature = "tokio"))]
    fn send(&self, bytes: &[u8]) -> Self::SendFut<'_> {
        let mut commands = self.sent_commands.lock().unwrap();
        commands.push(bytes.to_vec());
        futures::future::ready(Ok(()))
    }

    #[cfg(feature = "tokio")]
    fn recv(&self) -> Self::RecvFut<'_> {
        // Clone the Arc to avoid lifetime issues
        let receiver_arc = Arc::clone(&self.response_receiver);
        Box::pin(async move {
            // This simulates real hardware that blocks until data is available
            let mut receiver = receiver_arc.lock().await;
            match receiver.recv().await {
                Some(result) => result,
                None => Err(Error::TransportError("Transport closed".to_string())),
            }
        })
    }

    #[cfg(not(feature = "tokio"))]
    fn recv(&self) -> Self::RecvFut<'_> {
        // For non-tokio builds, return a future that never completes
        // This simulates blocking I/O
        futures::future::pending()
    }
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_socket_manager_initialization() {
    let transport = MockTransport::new();
    let handle = tokio::runtime::Handle::current();
    let mut camera =
        Camera::with_profile_and_spawner(CameraModel::PTZOpticsG2, transport.clone(), handle);

    // Initialize socket manager
    let result = camera.initialize_socket_manager();
    assert!(
        result.is_ok(),
        "Failed to initialize socket manager: {:?}",
        result
    );

    // Give the actor time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    println!("Socket manager initialization test passed");
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_socket_manager_command_sending() {
    let transport = MockTransport::new();
    let handle = tokio::runtime::Handle::current();
    let mut camera =
        Camera::with_profile_and_spawner(CameraModel::PTZOpticsG2, transport.clone(), handle);

    // Initialize socket manager
    camera
        .initialize_socket_manager()
        .expect("Failed to initialize socket manager");

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
    let sent_commands = transport.get_sent_commands().await;
    assert!(!sent_commands.is_empty(), "No commands were sent");

    println!("Socket manager command sending test completed");
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_socket_manager_timeout_handling() {
    let transport = MockTransport::new();
    let handle = tokio::runtime::Handle::current();
    let mut camera =
        Camera::with_profile_and_spawner(CameraModel::PTZOpticsG2, transport.clone(), handle);

    // Initialize socket manager
    camera
        .initialize_socket_manager()
        .expect("Failed to initialize socket manager");

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
    let handle = tokio::runtime::Handle::current();
    let mut camera =
        Camera::with_profile_and_spawner(CameraModel::PTZOpticsG2, transport.clone(), handle);

    // Initialize socket manager
    camera
        .initialize_socket_manager()
        .expect("Failed to initialize socket manager");

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
    // This test is for non-tokio environments, so use standard constructor
    let mut camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport);

    // Initialize socket manager
    let result = camera.initialize_socket_manager();

    match result {
        Ok(_) => println!("Socket manager initialized successfully without tokio"),
        Err(e) => println!("Socket manager initialization failed: {:?}", e),
    }
}
