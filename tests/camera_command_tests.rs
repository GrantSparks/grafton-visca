//! Tests for Camera API with command execution using MockTransport.
//!
//! These tests verify that the Camera API correctly sends commands
//! and handles responses through the transport layer.

#[path = "common/mod.rs"]
mod common;

#[cfg(not(feature = "async"))]
mod blocking_tests {
    use super::common::MockTransport;
    use grafton_visca::{
        camera::{profiles::G2PresetId, Camera, PTZOpticsG2},
        Error,
    };

    #[test]
    fn test_camera_power_command() {
        // Create a mock that returns ACK and completion
        let mock = MockTransport::with_ack_completion();
        let commands_sent = mock.commands_sent.clone();
        let transport = mock; // Use raw mock transport, Camera::new will wrap it
        let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

        // Send power on command
        let result = camera.power_on();
        assert!(result.is_ok(), "Power on command should succeed");

        // Verify the command bytes sent
        let commands = commands_sent.lock().unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0],
            vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF],
            "Power on command bytes"
        );
    }

    #[test]
    fn test_camera_home_command() {
        let mock = MockTransport::new();
        mock.add_ack_completion(0);

        let commands_sent = mock.commands_sent.clone();
        let transport = mock; // Use raw mock transport, Camera::new will wrap it
        let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

        // Send home command
        let result = camera.home();
        assert!(result.is_ok(), "Home command should succeed");

        // Verify command was sent
        let commands = commands_sent.lock().unwrap();
        assert_eq!(commands.len(), 1);
        // Home command: 81 01 06 04 FF
        assert_eq!(
            commands[0],
            vec![0x81, 0x01, 0x06, 0x04, 0xFF],
            "Home command bytes"
        );
    }

    #[test]
    fn test_camera_zoom_commands() {
        let mock = MockTransport::new();
        // Add responses for multiple commands
        mock.add_ack_completion(0); // Stop
        mock.add_ack_completion(1); // In
        mock.add_ack_completion(0); // Out

        let commands_sent = mock.commands_sent.clone();
        let transport = mock; // Use raw mock transport, Camera::new will wrap it
        let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

        // Test zoom stop
        assert!(camera.zoom_stop().is_ok());

        // Test zoom in
        assert!(camera.zoom_in().is_ok());

        // Test zoom out
        assert!(camera.zoom_out().is_ok());

        // Verify all commands were sent
        let commands = commands_sent.lock().unwrap();
        assert_eq!(commands.len(), 3);

        // Zoom stop: 81 01 04 07 00 FF
        assert_eq!(commands[0], vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]);
        // Zoom in: 81 01 04 07 02 FF (tele standard)
        assert_eq!(commands[1], vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]);
        // Zoom out: 81 01 04 07 03 FF (wide standard)
        assert_eq!(commands[2], vec![0x81, 0x01, 0x04, 0x07, 0x03, 0xFF]);
    }

    #[test]
    fn test_camera_preset_operations() {
        let mock = MockTransport::new();
        mock.add_ack_completion(0); // Store
        mock.add_ack_completion(1); // Recall

        let commands_sent = mock.commands_sent.clone();
        let transport = mock; // Use raw mock transport, Camera::new will wrap it
        let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

        // Set preset 5
        let preset_id = G2PresetId::new(5).unwrap();
        let result = camera.set_preset(preset_id.into());
        assert!(result.is_ok(), "Set preset should succeed");

        // Recall preset 5
        let result = camera.recall_preset(preset_id.into());
        assert!(result.is_ok(), "Recall preset should succeed");

        let commands = commands_sent.lock().unwrap();
        assert_eq!(commands.len(), 2);

        // Store preset 5: 81 01 04 3F 01 05 FF
        assert_eq!(commands[0], vec![0x81, 0x01, 0x04, 0x3F, 0x01, 0x05, 0xFF]);
        // Recall preset 5: 81 01 04 3F 02 05 FF
        assert_eq!(commands[1], vec![0x81, 0x01, 0x04, 0x3F, 0x02, 0x05, 0xFF]);
    }

    #[test]
    fn test_camera_error_handling() {
        let mock = MockTransport::new();
        // Add syntax error response
        mock.add_response(vec![0x90, 0x60, 0x02, 0xFF]);

        let transport = mock; // Use raw mock transport, Camera::new will wrap it
        let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

        // Send a command that will get an error response
        let result = camera.power_on();
        assert!(result.is_err(), "Should get an error");

        match result {
            Err(Error::SyntaxError) => {
                // Expected error type
            }
            _ => panic!("Expected CommandError, got {:?}", result),
        }
    }

    #[test]
    fn test_camera_timeout() {
        // Create a mock that returns no responses (simulates timeout)
        let mock = MockTransport::with_timeout();
        let transport = mock; // Use raw mock transport, Camera::new will wrap it
        let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

        let result = camera.home();
        assert!(result.is_err(), "Should timeout");

        match result {
            Err(Error::Timeout) => {
                // Expected timeout
            }
            _ => panic!("Expected Timeout error, got {:?}", result),
        }
    }

    #[test]
    fn test_camera_command_sequence() {
        let mock = MockTransport::new();

        // Queue responses for a sequence of commands
        mock.add_ack_completion(0); // Home
        mock.add_ack_completion(1); // Zoom stop
        mock.add_ack_completion(0); // Power off

        let commands_sent = mock.commands_sent.clone();
        let transport = mock; // Use raw mock transport, Camera::new will wrap it
        let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

        // Execute a sequence of commands
        assert!(camera.home().is_ok());
        assert!(camera.zoom_stop().is_ok());
        assert!(camera.power_off().is_ok());

        // Verify all commands were sent in order
        let commands = commands_sent.lock().unwrap();
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0], vec![0x81, 0x01, 0x06, 0x04, 0xFF]); // Home
        assert_eq!(commands[1], vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]); // Zoom stop
        assert_eq!(commands[2], vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]); // Power standby
    }
}

#[cfg(all(feature = "async", feature = "tokio"))]
mod async_tests {
    use super::common::MockAsyncTransport;
    use grafton_visca::{
        camera::{Camera, PTZOpticsG2},
        Error,
    };

    #[tokio::test]
    async fn test_async_camera_power_command() {
        // Create a mock that returns ACK and completion
        let mock = MockAsyncTransport::new();
        mock.add_ack_completion(0).await;

        let sent_commands = mock.sent_commands.clone();
        let transport = mock; // Use the raw mock transport, Camera::new will wrap it
        let camera = Camera::<PTZOpticsG2, _>::new(transport);

        // Send power on command
        let result = camera.power_on().await;
        assert!(result.is_ok(), "Power on command should succeed");

        // Verify the command bytes sent
        let commands = sent_commands.lock().await;
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0],
            vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF],
            "Power on command bytes"
        );
    }

    #[tokio::test]
    async fn test_async_camera_home_command() {
        let mock = MockAsyncTransport::new();
        mock.add_ack_completion(0).await;

        let sent_commands = mock.sent_commands.clone();
        let transport = mock; // Use raw mock transport, Camera::new will wrap it
        let camera = Camera::<PTZOpticsG2, _>::new(transport);

        // Send home command
        let result = camera.home().await;
        assert!(result.is_ok(), "Home command should succeed");

        // Verify command was sent
        let commands = sent_commands.lock().await;
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0],
            vec![0x81, 0x01, 0x06, 0x04, 0xFF],
            "Home command bytes"
        );
    }

    #[tokio::test]
    async fn test_async_camera_timeout() {
        // Create a mock with delay that will cause timeout
        let mock = MockAsyncTransport::new().with_delay(200);
        // Don't add any responses

        let transport = mock; // Use raw mock transport, Camera::new will wrap it
        let camera = Camera::<PTZOpticsG2, _>::new(transport);

        let result = camera.home().await;
        assert!(result.is_err(), "Should timeout");

        match result {
            Err(Error::CommandTimeout { .. }) => {
                // Expected timeout
            }
            _ => panic!("Expected CommandTimeout error, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_async_camera_concurrent_commands() {
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let mock = MockAsyncTransport::new();
        // Add responses for concurrent commands
        // Since commands are serialized by the Mutex, they'll both use socket 0
        mock.add_ack_completion(0).await;
        mock.add_ack_completion(0).await;

        let command_counter = mock.command_counter.clone();
        let transport = mock; // Use raw mock transport, Camera::new will wrap it
        let camera = Arc::new(Mutex::new(Camera::<PTZOpticsG2, _>::new(transport)));

        // Send two commands concurrently
        let cam1 = camera.clone();
        let task1 = tokio::spawn(async move {
            let cam = cam1.lock().await;
            cam.home().await
        });

        let cam2 = camera.clone();
        let task2 = tokio::spawn(async move {
            let cam = cam2.lock().await;
            cam.zoom_stop().await
        });

        // Both should succeed
        assert!(task1.await.unwrap().is_ok());
        assert!(task2.await.unwrap().is_ok());

        // Verify both commands were sent
        let count = *command_counter.lock().await;
        assert_eq!(count, 2);
    }
}
