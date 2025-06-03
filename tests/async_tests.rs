#![cfg(feature = "async")]

use grafton_visca::command::{power::Power, PowerCommand};
use grafton_visca::{
    AsyncViscaTransport, TransportFuture, ViscaCommand, ViscaError, ViscaResponse,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Mock transport for testing that records sent commands and provides canned responses.
struct MockTransport {
    sent_commands: Arc<Mutex<Vec<Vec<u8>>>>,
    responses: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl MockTransport {
    fn new() -> Self {
        Self {
            sent_commands: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn add_response(&self, response: Vec<u8>) {
        let mut responses = self.responses.lock().await;
        responses.push(response);
    }
}

impl AsyncViscaTransport for MockTransport {
    fn send_command<'a>(&'a mut self, command: &'a dyn ViscaCommand) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            let bytes = command.to_bytes()?;
            let mut sent = self.sent_commands.lock().await;
            sent.push(bytes);
            Ok(())
        })
    }

    fn receive_response(&mut self) -> TransportFuture<'_, Vec<Vec<u8>>> {
        Box::pin(async move {
            // Simulate a small delay
            tokio::time::sleep(Duration::from_millis(10)).await;

            let mut responses = self.responses.lock().await;
            if responses.is_empty() {
                Err(ViscaError::Timeout)
            } else {
                Ok(vec![responses.remove(0)])
            }
        })
    }
}

#[tokio::test]
async fn test_async_command_sequence() {
    // Test that commands are sent and acknowledged properly
    let transport = MockTransport::new();

    // Add expected responses (ACK then completion for power on)
    transport.add_response(vec![0x90, 0x40, 0xFF]).await; // ACK socket 0
    transport.add_response(vec![0x90, 0x50, 0xFF]).await; // Completion socket 0

    // This test would require refactoring AsyncViscaClient to accept a custom transport
    // For now, we'll test the transport behavior directly
    let mut transport = transport;

    let cmd = PowerCommand { power: Power::On };
    transport.send_command(&cmd).await.unwrap();

    let responses = transport.receive_response().await.unwrap();
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0], vec![0x90, 0x40, 0xFF]);
}

#[tokio::test]
async fn test_concurrent_commands() {
    // Test that multiple commands can be sent concurrently
    // This would require a real async client test with proper mocking

    // For now, we test that the async runtime works
    let task1 = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        "task1"
    });

    let task2 = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(5)).await;
        "task2"
    });

    let (res1, res2) = tokio::join!(task1, task2);
    assert_eq!(res1.unwrap(), "task1");
    assert_eq!(res2.unwrap(), "task2");
}

#[tokio::test]
async fn test_semaphore_limiting() {
    use tokio::sync::Semaphore;

    // Test that semaphore properly limits to 2 concurrent operations
    let sem = Arc::new(Semaphore::new(2));
    let mut handles = vec![];

    for i in 0..3 {
        let sem_clone = Arc::clone(&sem);
        let handle = tokio::spawn(async move {
            let _permit = sem_clone.acquire().await.unwrap();
            let start = tokio::time::Instant::now();
            tokio::time::sleep(Duration::from_millis(50)).await;
            (i, start)
        });
        handles.push(handle);
        // Give tasks time to start
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let mut results: Vec<_> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    results.sort_by_key(|(i, _)| *i);

    // First two should start almost immediately
    // Third should start after one of the first two finishes (~50ms later)
    let time_diff = results[2].1.duration_since(results[0].1);
    assert!(time_diff >= Duration::from_millis(40)); // Some tolerance for timing
}

#[tokio::test]
async fn test_timeout_handling() {
    use tokio::time::timeout;

    // Test that operations can timeout properly
    let future = async {
        tokio::time::sleep(Duration::from_secs(1)).await;
        Ok::<_, ViscaError>(ViscaResponse::Completion)
    };

    let result = timeout(Duration::from_millis(100), future).await;
    assert!(result.is_err()); // Should timeout
}

#[tokio::test]
async fn test_error_propagation() {
    // Test that errors are properly propagated through the async chain
    async fn failing_operation() -> Result<ViscaResponse, ViscaError> {
        Err(ViscaError::CommandBufferFull)
    }

    let result = failing_operation().await;
    assert!(matches!(result, Err(ViscaError::CommandBufferFull)));
}
