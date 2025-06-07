#![cfg(feature = "async-client")]

use grafton_visca::{ViscaError, ViscaInquiryResponse, ViscaResponse, ViscaSession};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_session_concurrent_commands() {
    let session = Arc::new(Mutex::new(ViscaSession::new()));

    // Test assigning two sockets concurrently
    let session1 = Arc::clone(&session);
    let socket1_task = tokio::spawn(async move {
        let mut session = session1.lock().await;
        session.assign_socket(None)
    });

    let session2 = Arc::clone(&session);
    let socket2_task = tokio::spawn(async move {
        // Small delay to ensure first task gets socket 0
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        let mut session = session2.lock().await;
        session.assign_socket(None)
    });

    let (socket1, socket2) = tokio::join!(socket1_task, socket2_task);
    let socket1 = socket1.unwrap().unwrap();
    let socket2 = socket2.unwrap().unwrap();

    assert_eq!(socket1, 0);
    assert_eq!(socket2, 1);

    // Try to assign a third socket (should fail)
    let result = session.lock().await.assign_socket(None);
    assert!(matches!(result, Err(ViscaError::CommandBufferFull)));
}

#[tokio::test]
async fn test_session_ack_completion_flow() {
    let session = Arc::new(Mutex::new(ViscaSession::new()));

    // Assign a socket
    let socket_id = {
        let mut session = session.lock().await;
        session.assign_socket(None).unwrap()
    };

    // Send ACK
    let ack_response = vec![0x90, 0x40 | socket_id, 0xFF];
    {
        let mut session = session.lock().await;
        let result = session.process_response(&ack_response).unwrap();
        assert!(matches!(result, Some((sid, ViscaResponse::Ack)) if sid == socket_id));
        assert!(session.is_acknowledged(socket_id));
        drop(session);
    }

    // Send Completion
    let completion_response = vec![0x90, 0x50 | socket_id, 0xFF];
    {
        let result = session
            .lock()
            .await
            .process_response(&completion_response)
            .unwrap();
        assert!(matches!(result, Some((sid, ViscaResponse::Completion)) if sid == socket_id));
    }

    // Socket should still be allocated after completion (requires explicit release)
    {
        assert_eq!(session.lock().await.pending_count(), 1);
    }

    // Release the socket
    {
        let mut session = session.lock().await;
        session.release_socket(socket_id);
        assert_eq!(session.pending_count(), 0);
        drop(session);
    }
}

#[tokio::test]
async fn test_session_error_handling() {
    let session = Arc::new(Mutex::new(ViscaSession::new()));

    // Assign a socket
    let socket_id = {
        let mut session = session.lock().await;
        session.assign_socket(None).unwrap()
    };

    // Send error response
    let error_response = vec![0x90, 0x60 | socket_id, 0x03, 0xFF]; // Command buffer full
    {
        let result = session
            .lock()
            .await
            .process_response(&error_response)
            .unwrap();
        assert!(matches!(
            result,
            Some((sid, ViscaResponse::Error(ViscaError::CommandBufferFull))) if sid == socket_id
        ));
    }

    // Socket should still be allocated after error (requires explicit release)
    {
        assert_eq!(session.lock().await.pending_count(), 1);
    }

    // Release the socket
    {
        let mut session = session.lock().await;
        session.release_socket(socket_id);
        assert_eq!(session.pending_count(), 0);
        drop(session);
    }
}

#[tokio::test]
async fn test_session_inquiry_response() {
    let session = Arc::new(Mutex::new(ViscaSession::new()));

    // Assign a socket for zoom position inquiry
    let socket_id = {
        let mut session = session.lock().await;
        session
            .assign_socket(Some(grafton_visca::ViscaResponseType::ZoomPosition))
            .unwrap()
    };

    // Send inquiry response
    let inquiry_response = vec![0x90, 0x50 | socket_id, 0x01, 0x02, 0x03, 0x04, 0xFF];
    {
        let result = session
            .lock()
            .await
            .process_response(&inquiry_response)
            .unwrap();
        match result {
            Some((
                sid,
                ViscaResponse::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position }),
            )) => {
                assert_eq!(sid, socket_id);
                assert_eq!(position, 0x1234);
            }
            _ => panic!("Unexpected response type"),
        }
    }
}
