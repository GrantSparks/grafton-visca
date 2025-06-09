use grafton_visca::{Error, Response, Session, SocketId};

#[test]
fn test_session_socket_management() {
    let mut session = Session::new();

    // Should be able to send two commands
    let socket1 = session.assign_socket(None).unwrap();
    let socket2 = session.assign_socket(None).unwrap();

    assert_ne!(socket1, socket2);
    assert_eq!(session.pending_count(), 2);

    // Third command should fail
    let result = session.assign_socket(None);
    assert!(matches!(result, Err(Error::CommandBufferFull)));

    // After releasing one socket, should be able to send another
    session.release_socket(socket1);
    assert_eq!(session.pending_count(), 1);

    let socket3 = session.assign_socket(None).unwrap();
    assert_eq!(socket3, socket1); // Should reuse the freed socket
}

#[test]
fn test_interleaved_responses() {
    let mut session = Session::new();

    // Assign two sockets
    let socket_a = session.assign_socket(None).unwrap();
    let socket_b = session.assign_socket(None).unwrap();

    // Simulate interleaved ACKs and completions
    let responses = [
        vec![0x90, 0x40 | socket_b.value(), 0xFF], // ACK for B
        vec![0x90, 0x40 | socket_a.value(), 0xFF], // ACK for A
        vec![0x90, 0x50 | socket_b.value(), 0xFF], // Completion for B
        vec![0x90, 0x50 | socket_a.value(), 0xFF], // Completion for A
    ];

    // Process all responses and verify correct socket assignment
    for (i, response) in responses.iter().enumerate() {
        let result = session.process_response(response).unwrap();
        assert!(result.is_some());

        let (socket_id, parsed) = result.unwrap();

        match i {
            0 => {
                assert_eq!(socket_id, socket_b);
                assert!(matches!(parsed, Response::Ack));
                // Socket B should be acknowledged
            }
            1 => {
                assert_eq!(socket_id, socket_a);
                assert!(matches!(parsed, Response::Ack));
                // Socket A should be acknowledged
            }
            2 => {
                assert_eq!(socket_id, socket_b);
                assert!(matches!(parsed, Response::Completion));
            }
            3 => {
                assert_eq!(socket_id, socket_a);
                assert!(matches!(parsed, Response::Completion));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn test_error_on_one_socket_doesnt_affect_other() {
    let mut session = Session::new();

    // Assign two sockets
    let socket_a = session.assign_socket(None).unwrap();
    let socket_b = session.assign_socket(None).unwrap();

    // Socket A gets an error, socket B completes successfully
    let responses = vec![
        vec![0x90, 0x40 | socket_a.value(), 0xFF], // ACK for A
        vec![0x90, 0x40 | socket_b.value(), 0xFF], // ACK for B
        vec![0x90, 0x60 | socket_a.value(), 0x41, 0xFF], // Error for A (Not Executable)
        vec![0x90, 0x50 | socket_b.value(), 0xFF], // Completion for B
    ];

    let mut results = Vec::new();
    for response in responses {
        if let Ok(Some(result)) = session.process_response(&response) {
            results.push(result);
        }
    }

    assert_eq!(results.len(), 4);

    // Verify socket A got error
    let (socket, response) = &results[2];
    assert_eq!(*socket, socket_a);
    assert!(matches!(
        response,
        Response::Error(Error::CommandNotExecutable)
    ));

    // Verify socket B completed successfully
    let (socket, response) = &results[3];
    assert_eq!(*socket, socket_b);
    assert!(matches!(response, Response::Completion));
}

#[test]
fn test_response_for_unknown_socket_ignored() {
    let mut session = Session::new();

    // Only assign socket 0
    session.assign_socket(None).unwrap();

    // Response for socket 1 (not assigned) should be ignored
    let response = vec![0x90, 0x41, 0xFF]; // ACK for socket 1
    let result = session.process_response(&response).unwrap();

    // Should return None (ignored)
    assert!(result.is_none());
}

#[test]
fn test_multiple_errors_in_sequence() {
    let mut session = Session::new();
    let socket = session.assign_socket(None).unwrap();

    // Multiple errors can occur if camera state changes
    let responses = vec![
        vec![0x90, 0x40 | socket.value(), 0xFF],       // ACK
        vec![0x90, 0x60 | socket.value(), 0x02, 0xFF], // Syntax Error
    ];

    for response in responses {
        let result = session.process_response(&response).unwrap();
        assert!(result.is_some());
    }
}

#[test]
fn test_socket_reuse_after_completion() {
    let mut session = Session::new();

    // First command uses socket 0
    let socket1 = session.assign_socket(None).unwrap();
    assert_eq!(socket1, SocketId::SOCKET_0);

    // Process ACK and completion
    session.process_response(&[0x90, 0x40, 0xFF]).unwrap();
    session.process_response(&[0x90, 0x50, 0xFF]).unwrap();

    // Release socket 0
    session.release_socket(SocketId::SOCKET_0);

    // Next command should reuse socket 0
    let socket2 = session.assign_socket(None).unwrap();
    assert_eq!(socket2, SocketId::SOCKET_0);
}
