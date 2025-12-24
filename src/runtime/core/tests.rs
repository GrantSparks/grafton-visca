//! Tests for the scheduler core state machine.

#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]

use super::*;
use crate::command::bytes::VISCA_TERMINATOR;
use crate::command::encode::EncodedCommand;
use crate::transport::{BackoffStrategy, RetryAttempt, RetryConfig};
use crate::CameraId;
use crate::Error;
use smallvec::SmallVec;
use std::sync::Arc;
use std::time::Duration;

/// Helper function to create CommandId from u32 in tests.
/// Panics if value is 0 (invalid for CommandId).
fn cmd_id(value: u32) -> CommandId {
    CommandId::from_raw(value).expect("test command ID must be non-zero")
}

// Helper structs for different test command categories
#[derive(Debug, Clone)]
struct TestCommandQuick {
    bytes: Vec<u8>,
    response_type: Option<InquiryKind>,
}

impl crate::command::encode::ViscaCommand for TestCommandQuick {
    type Response = ();
    const MAX_SIZE: usize = 16;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        let len = self.bytes.len();
        buffer[..len].copy_from_slice(&self.bytes);
        Ok(len)
    }

    fn response_kind(&self) -> Option<InquiryKind> {
        self.response_type
    }
}

#[derive(Debug, Clone)]
struct TestCommandMovement {
    bytes: Vec<u8>,
    response_type: Option<InquiryKind>,
}

impl crate::command::encode::ViscaCommand for TestCommandMovement {
    type Response = ();
    const MAX_SIZE: usize = 16;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Movement;

    fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        let len = self.bytes.len();
        buffer[..len].copy_from_slice(&self.bytes);
        Ok(len)
    }

    fn response_kind(&self) -> Option<InquiryKind> {
        self.response_type
    }
}

// Helper function to create test commands from byte patterns
fn create_test_command(
    bytes: Vec<u8>,
    response_type: Option<InquiryKind>,
    category: CommandCategory,
    camera_id: CameraId,
) -> Arc<EncodedCommand> {
    let command = match category {
        CommandCategory::Quick => {
            let cmd = TestCommandQuick {
                bytes,
                response_type,
            };
            EncodedCommand::new(&cmd, camera_id).unwrap()
        }
        CommandCategory::Movement => {
            let cmd = TestCommandMovement {
                bytes,
                response_type,
            };
            EncodedCommand::new(&cmd, camera_id).unwrap()
        }
        _ => {
            // Default to Quick for other categories in tests
            let cmd = TestCommandQuick {
                bytes,
                response_type,
            };
            EncodedCommand::new(&cmd, camera_id).unwrap()
        }
    };
    Arc::new(command)
}

// Helper struct for test inquiries
#[derive(Debug, Clone)]
struct TestInquiryHelper;

impl crate::command::encode::ViscaCommand for TestInquiryHelper {
    type Response = ();
    const MAX_SIZE: usize = 5;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        // Standard inquiry bytes: 81 09 00 02 FF (Power inquiry)
        buffer[0] = 0x81;
        buffer[1] = 0x09;
        buffer[2] = 0x00;
        buffer[3] = 0x02;
        buffer[4] = VISCA_TERMINATOR;
        Ok(5)
    }

    fn response_kind(&self) -> Option<InquiryKind> {
        Some(InquiryKind::Power)
    }
}

/// Helper function to create test inquiries.
fn create_test_inquiry(camera_id: CameraId) -> Arc<EncodedCommand> {
    Arc::new(EncodedCommand::new(&TestInquiryHelper, camera_id).unwrap())
}

#[test]
fn test_retry_budget_from_base() {
    // Test with base of 3 (default)
    let budget = RetryBudget::from_base(3);
    assert_eq!(budget.quick, 5); // 3 + 2
    assert_eq!(budget.movement, 3);
    assert_eq!(budget.preset, 3);
    assert_eq!(budget.network, 2); // 3 - 1, min 1
    assert_eq!(budget.long_running, 1);
    assert_eq!(budget.custom, 3);

    // Test with base of 0
    let budget = RetryBudget::from_base(0);
    assert_eq!(budget.quick, 2); // 0 + 2
    assert_eq!(budget.movement, 0);
    assert_eq!(budget.preset, 0);
    assert_eq!(budget.network, 1); // min 1
    assert_eq!(budget.long_running, 1);
    assert_eq!(budget.custom, 0);

    // Test with base of 1
    let budget = RetryBudget::from_base(1);
    assert_eq!(budget.quick, 3); // 1 + 2
    assert_eq!(budget.movement, 1);
    assert_eq!(budget.preset, 1);
    assert_eq!(budget.network, 1); // 1 - 1 = 0, but min 1
    assert_eq!(budget.long_running, 1);
    assert_eq!(budget.custom, 1);

    // Test with large base
    let budget = RetryBudget::from_base(10);
    assert_eq!(budget.quick, 12); // 10 + 2
    assert_eq!(budget.movement, 10);
    assert_eq!(budget.preset, 10);
    assert_eq!(budget.network, 9); // 10 - 1
    assert_eq!(budget.long_running, 1);
    assert_eq!(budget.custom, 10);
}

#[test]
fn test_retry_budget_for_category() {
    let budget = RetryBudget::from_base(3);

    assert_eq!(budget.for_category(CommandCategory::Quick), 5);
    assert_eq!(budget.for_category(CommandCategory::Movement), 3);
    assert_eq!(budget.for_category(CommandCategory::Preset), 3);
    assert_eq!(budget.for_category(CommandCategory::Network), 2);
    assert_eq!(budget.for_category(CommandCategory::LongRunning), 1);
    assert_eq!(budget.for_category(CommandCategory::Custom), 3);
}

#[test]
fn test_ack_backoff_parity() {
    // Test that the new ACK backoff calculation matches the legacy behavior
    let retry_config = RetryConfig {
        max_retries: 3,
        base_retry_delay: Duration::from_millis(100),
        max_retry_duration: Duration::from_secs(10),
        backoff_strategy: BackoffStrategy::Exponential,
    };

    // Legacy calculation: base_delay * 2^attempts.min(5)
    // New calculation: retry_config.calculate_delay((attempts + 1).min(6), None)

    // Test cases matching the legacy behavior
    let test_cases = vec![
        (0, 100),  // 2^0 = 1, 100ms * 1 = 100ms
        (1, 200),  // 2^1 = 2, 100ms * 2 = 200ms
        (2, 400),  // 2^2 = 4, 100ms * 4 = 400ms
        (3, 800),  // 2^3 = 8, 100ms * 8 = 800ms
        (4, 1600), // 2^4 = 16, 100ms * 16 = 1600ms
        (5, 3200), // 2^5 = 32, 100ms * 32 = 3200ms (capped)
        (6, 3200), // Still capped at 2^5
        (7, 3200), // Still capped at 2^5
    ];

    for (attempts, expected_ms) in test_cases {
        // New calculation used in the code
        let capped_attempt_num = (attempts + 1).min(6);
        let retry_attempt = RetryAttempt::new(capped_attempt_num).unwrap();
        let actual_delay = retry_config.calculate_delay(retry_attempt, None);

        assert_eq!(
            actual_delay,
            Duration::from_millis(expected_ms),
            "Mismatch for attempt {}: expected {}ms, got {}ms",
            attempts,
            expected_ms,
            actual_delay.as_millis()
        );
    }
}

#[test]
fn test_inquiry_does_not_consume_sockets() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let priority = Priority::Normal;
    let _category = CommandCategory::Quick;
    let camera_id = CameraId::CAMERA_1;

    // Create a test inquiry
    #[derive(Clone)]
    struct TestInquiry;
    impl crate::command::encode::ViscaCommand for TestInquiry {
        type Response = ();
        const MAX_SIZE: usize = 5;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;
        fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = 0x81;
            buffer[1] = 0x09;
            buffer[2] = 0x00;
            buffer[3] = 0x02;
            buffer[4] = VISCA_TERMINATOR;
            Ok(5)
        }
        fn response_kind(&self) -> Option<InquiryKind> {
            Some(InquiryKind::Power)
        }
    }
    impl std::fmt::Debug for TestInquiry {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("TestInquiry").finish()
        }
    }

    let inquiry_cmd = Arc::new(EncodedCommand::new(&TestInquiry, camera_id).unwrap());

    // Helper to create test commands
    #[derive(Clone)]
    struct TestCmd1;
    impl crate::command::encode::ViscaCommand for TestCmd1 {
        type Response = ();
        const MAX_SIZE: usize = 6;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Movement;
        fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = 0x81;
            buffer[1] = 0x01;
            buffer[2] = 0x04;
            buffer[3] = 0x00;
            buffer[4] = 0x02;
            buffer[5] = VISCA_TERMINATOR;
            Ok(6)
        }
        fn response_kind(&self) -> Option<InquiryKind> {
            None
        }
    }
    impl std::fmt::Debug for TestCmd1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("TestCmd1").finish()
        }
    }

    #[derive(Clone)]
    struct TestCmd2;
    impl crate::command::encode::ViscaCommand for TestCmd2 {
        type Response = ();
        const MAX_SIZE: usize = 6;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Movement;
        fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = 0x81;
            buffer[1] = 0x01;
            buffer[2] = 0x04;
            buffer[3] = 0x00;
            buffer[4] = 0x03;
            buffer[5] = VISCA_TERMINATOR;
            Ok(6)
        }
        fn response_kind(&self) -> Option<InquiryKind> {
            None
        }
    }
    impl std::fmt::Debug for TestCmd2 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("TestCmd2").finish()
        }
    }

    // Start two commands to occupy both sockets
    let cmd1 = Arc::new(EncodedCommand::new(&TestCmd1, camera_id).unwrap());
    let cmd2 = Arc::new(EncodedCommand::new(&TestCmd2, camera_id).unwrap());

    // Register first command on socket 1
    core.register_pending_ack(cmd_id(1), cmd1.clone(), priority, camera_id, now);
    // Manually allocate socket 1 (simulating ACK received)
    // When ACK is received, command is removed from pending_ack_ids
    core.pending_ack_ids.remove(&cmd_id(1));
    core.sockets[0] = SocketState::Busy {
        command_id: cmd_id(1),
        started_at: now,
        category: CommandCategory::Movement,
    };

    // Register second command on socket 2
    core.register_pending_ack(cmd_id(2), cmd2.clone(), priority, camera_id, now);
    // Manually allocate socket 2 (simulating ACK received)
    // When ACK is received, command is removed from pending_ack_ids
    core.pending_ack_ids.remove(&cmd_id(2));
    core.sockets[1] = SocketState::Busy {
        command_id: cmd_id(2),
        started_at: now,
        category: CommandCategory::Movement,
    };

    // Both sockets are now occupied, but inquiry should still be sendable
    assert!(!core.can_send_command()); // Cannot send more commands

    // Start an inquiry - should not need a socket
    core.start_inquiry(cmd_id(3), inquiry_cmd.clone(), priority, camera_id, now);

    // Verify inquiry is tracked
    assert!(core.inflight_inquiry_ids.contains(&cmd_id(3)));
    assert!(core.inquiries_order.contains(&cmd_id(3)));

    // Sockets should still be occupied by commands
    let state1 = core.socket_state(ViscaSocket::S1);
    assert!(!state1.0); // Socket 1 still occupied
    assert_eq!(state1.1, Some(cmd_id(1))); // By command 1

    let state2 = core.socket_state(ViscaSocket::S2);
    assert!(!state2.0); // Socket 2 still occupied
    assert_eq!(state2.1, Some(cmd_id(2))); // By command 2
}

#[test]
fn test_inquiry_reply_handling() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();

    // Create a test inquiry command
    #[derive(Debug, Clone)]
    struct TestInquiryCmd;
    impl crate::command::encode::ViscaCommand for TestInquiryCmd {
        type Response = ();
        const MAX_SIZE: usize = 5;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;
        fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = 0x81;
            buffer[1] = 0x09;
            buffer[2] = 0x00;
            buffer[3] = 0x02;
            buffer[4] = VISCA_TERMINATOR;
            Ok(5)
        }
        fn response_kind(&self) -> Option<InquiryKind> {
            Some(InquiryKind::Power)
        }
    }

    let priority = Priority::Normal;
    let _category = CommandCategory::Quick;
    let camera_id = CameraId::CAMERA_1;
    let command = Arc::new(EncodedCommand::new(&TestInquiryCmd, camera_id).unwrap());

    // Start an inquiry
    core.start_inquiry(cmd_id(1), command.clone(), priority, camera_id, now);

    // Verify inquiry is tracked
    assert!(core.inflight_inquiry_ids.contains(&cmd_id(1)));
    assert!(core.inquiries_order.contains(&cmd_id(1)));

    // Process InquiryReply event
    let response = Response::Inquiry(crate::command::InquiryData::Power { on: true });
    let source = ReplySource::from_fields(Some(cmd_id(1)), None, None);
    let event = SchedulerEvent::InquiryReply { source, response };

    let actions = core.process_event(event, now);

    // Should get CommandComplete action
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        SchedulerAction::CommandComplete {
            id, response: resp, ..
        } => {
            assert_eq!(*id, cmd_id(1));
            match resp {
                Response::Inquiry(crate::command::InquiryData::Power { on }) => {
                    assert!(*on);
                }
                _ => panic!("Expected Power inquiry response"),
            }
        }
        _ => panic!("Expected CommandComplete action"),
    }

    // Inquiry should be removed from tracking
    assert!(!core.inflight_inquiry_ids.contains(&cmd_id(1)));
    assert!(!core.inquiries_order.contains(&cmd_id(1)));
}

#[test]
fn test_raw_visca_inquiry_ordering() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let priority = Priority::Normal;
    let _category = CommandCategory::Quick;
    let camera_id = CameraId::CAMERA_1;

    // Create test inquiry commands
    #[derive(Debug, Clone)]
    struct TestInquiry1;
    impl crate::command::encode::ViscaCommand for TestInquiry1 {
        type Response = ();
        const MAX_SIZE: usize = 5;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;
        fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = 0x81;
            buffer[1] = 0x09;
            buffer[2] = 0x00;
            buffer[3] = 0x02;
            buffer[4] = VISCA_TERMINATOR;
            Ok(5)
        }
        fn response_kind(&self) -> Option<InquiryKind> {
            Some(InquiryKind::Power)
        }
    }

    #[derive(Debug, Clone)]
    struct TestInquiry2;
    impl crate::command::encode::ViscaCommand for TestInquiry2 {
        type Response = ();
        const MAX_SIZE: usize = 5;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;
        fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = 0x81;
            buffer[1] = 0x09;
            buffer[2] = 0x04;
            buffer[3] = 0x00;
            buffer[4] = VISCA_TERMINATOR;
            Ok(5)
        }
        fn response_kind(&self) -> Option<InquiryKind> {
            Some(InquiryKind::ZoomPosition)
        }
    }

    #[derive(Debug, Clone)]
    struct TestInquiry3;
    impl crate::command::encode::ViscaCommand for TestInquiry3 {
        type Response = ();
        const MAX_SIZE: usize = 5;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;
        fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = 0x81;
            buffer[1] = 0x09;
            buffer[2] = 0x06;
            buffer[3] = 0x12;
            buffer[4] = VISCA_TERMINATOR;
            Ok(5)
        }
        fn response_kind(&self) -> Option<InquiryKind> {
            Some(InquiryKind::Power)
        }
    }

    let cmd1 = Arc::new(EncodedCommand::new(&TestInquiry1, camera_id).unwrap());
    let cmd2 = Arc::new(EncodedCommand::new(&TestInquiry2, camera_id).unwrap());
    let cmd3 = Arc::new(EncodedCommand::new(&TestInquiry3, camera_id).unwrap());

    core.start_inquiry(cmd_id(1), cmd1, priority, camera_id, now);
    core.start_inquiry(cmd_id(2), cmd2, priority, camera_id, now);
    core.start_inquiry(cmd_id(3), cmd3, priority, camera_id, now);

    // Verify all inquiries are tracked in order
    assert_eq!(core.inquiries_order.len(), 3);
    assert_eq!(core.inquiries_order[0], cmd_id(1));
    assert_eq!(core.inquiries_order[1], cmd_id(2));
    assert_eq!(core.inquiries_order[2], cmd_id(3));

    // Process InquiryReply events without cmd_id (raw VISCA)
    // First reply should match first inquiry
    let response1 = Response::Inquiry(crate::command::InquiryData::Power { on: true });
    let source1 = ReplySource::Unknown; // No sequence in raw VISCA
    let event1 = SchedulerEvent::InquiryReply {
        source: source1,
        response: response1,
    };

    let actions = core.process_event(event1, now);
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        SchedulerAction::CommandComplete { id, .. } => {
            assert_eq!(*id, cmd_id(1)); // First inquiry completed
        }
        _ => panic!("Expected CommandComplete action"),
    }

    // Order should have inquiry 1 removed
    assert_eq!(core.inquiries_order.len(), 2);
    assert_eq!(core.inquiries_order[0], cmd_id(2));
    assert_eq!(core.inquiries_order[1], cmd_id(3));
}

#[test]
fn test_sony_sequence_attribution() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let priority = Priority::Normal;
    let camera_id = CameraId::CAMERA_1;

    // Create test commands
    #[derive(Debug, Clone)]
    struct TestCmd1;
    impl crate::command::encode::ViscaCommand for TestCmd1 {
        type Response = ();
        const MAX_SIZE: usize = 6;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Movement;
        fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = 0x81;
            buffer[1] = 0x01;
            buffer[2] = 0x04;
            buffer[3] = 0x00;
            buffer[4] = 0x02;
            buffer[5] = VISCA_TERMINATOR;
            Ok(6)
        }
        fn response_kind(&self) -> Option<InquiryKind> {
            None
        }
    }

    #[derive(Debug, Clone)]
    struct TestCmd2;
    impl crate::command::encode::ViscaCommand for TestCmd2 {
        type Response = ();
        const MAX_SIZE: usize = 6;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Movement;
        fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = 0x81;
            buffer[1] = 0x01;
            buffer[2] = 0x04;
            buffer[3] = 0x00;
            buffer[4] = 0x03;
            buffer[5] = VISCA_TERMINATOR;
            Ok(6)
        }
        fn response_kind(&self) -> Option<InquiryKind> {
            None
        }
    }

    let cmd1 = Arc::new(EncodedCommand::new(&TestCmd1, camera_id).unwrap());
    let cmd2 = Arc::new(EncodedCommand::new(&TestCmd2, camera_id).unwrap());

    core.register_pending_ack(cmd_id(1), cmd1, priority, camera_id, now);
    core.register_sequence(cmd_id(1), 100); // Command 1 has sequence 100

    core.register_pending_ack(cmd_id(2), cmd2, priority, camera_id, now);
    core.register_sequence(cmd_id(2), 101); // Command 2 has sequence 101

    // Process ACK for command 2 first (out of order)
    let cmd_id_2 = core.get_command_by_sequence(101);
    let source = ReplySource::from_fields(cmd_id_2, Some(101), Some(ViscaSocket::S2));
    let event = SchedulerEvent::Ack { source };

    let actions = core.process_event(event, now);

    // ACK processing should be silent (no action returned)
    assert!(
        actions.is_empty(),
        "Expected no actions from ACK processing"
    );

    // Verify command 2 got socket 2
    let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S2);
    assert!(!free); // Socket occupied
    assert_eq!(socket_cmd_id, Some(cmd_id(2))); // By command 2

    // Command 1 should still be pending
    assert!(core.pending_ack_ids.contains(&cmd_id(1)));

    // Now process ACK for command 1
    let cmd_id_1 = core.get_command_by_sequence(100);
    let source = ReplySource::from_fields(cmd_id_1, Some(100), Some(ViscaSocket::S1));
    let event = SchedulerEvent::Ack { source };

    core.process_event(event, now);

    // Verify command 1 got socket 1
    let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
    assert!(!free); // Socket occupied
    assert_eq!(socket_cmd_id, Some(cmd_id(1))); // By command 1
}

#[test]
fn test_inquiry_timeout_handling() {
    let timeout_config = TimeoutConfig {
        quick_timeout: Duration::from_millis(100),
        ..Default::default()
    };
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let priority = Priority::Normal;
    let _category = CommandCategory::Quick;
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR],
        Some(InquiryKind::Power),
        CommandCategory::Quick,
        camera_id,
    );

    // Start an inquiry
    core.start_inquiry(cmd_id(1), command.clone(), priority, camera_id, now);
    assert!(core.inflight_inquiry_ids.contains(&cmd_id(1)));

    // Check timeout immediately - should not timeout
    let actions = core.check_timeouts(now);
    assert!(actions.is_empty());

    // Check timeout after the timeout period
    let later = now + Duration::from_millis(200);
    let actions = core.check_timeouts(later);

    // Quick commands get retries, so first timeout triggers a retry
    assert!(!actions.is_empty(), "Expected timeout action but got none");
    assert_eq!(
        actions.len(),
        1,
        "Expected exactly one action, got: {:?}",
        actions
    );
    match &actions[0] {
        SchedulerAction::RetryCommand { id, .. } => {
            assert_eq!(*id, cmd_id(1));
        }
        other => panic!("Expected RetryCommand action, got: {:?}", other),
    }

    // Inquiry should be removed from tracking after timeout
    assert!(!core.inflight_inquiry_ids.contains(&cmd_id(1)));

    // Start inquiry again for the retry
    core.start_inquiry(cmd_id(1), command.clone(), priority, camera_id, later);
    assert!(core.inflight_inquiry_ids.contains(&cmd_id(1)));

    // Exhaust retries by timing out again (simulate max retries reached)
    // For Quick category, we get extra retries, so we need to exhaust them
    // Set retry attempts to max to force failure on next timeout
    // Must be done AFTER start_inquiry since it creates a new CommandState
    if let Some(state) = core.commands.get_mut(&cmd_id(1)) {
        state.attempt = 10; // Force max retries exceeded
    }

    // Now timeout should fail
    let later2 = later + Duration::from_millis(200);
    let actions2 = core.check_timeouts(later2);

    assert_eq!(actions2.len(), 1, "Expected exactly one action");
    match &actions2[0] {
        SchedulerAction::CommandFailed { id, error } => {
            assert_eq!(*id, cmd_id(1));
            assert!(matches!(error, Error::Timeout));
        }
        other => panic!(
            "Expected CommandFailed action after max retries, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_sequence_tracking_with_retries() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let priority = Priority::Normal;
    let _category = CommandCategory::Movement;
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );

    // Register command with initial sequence
    core.register_pending_ack(cmd_id(1), command.clone(), priority, camera_id, now);
    core.register_sequence(cmd_id(1), 100);

    // Verify initial sequence is tracked
    assert_eq!(core.get_command_by_sequence(100), Some(cmd_id(1)));
    assert_eq!(core.seq_to_cmd.len(), 1);
    assert_eq!(core.cmd_to_seqs.len(), 1);

    // Simulate retry - register new sequence for same command
    core.register_sequence(cmd_id(1), 101);

    // Both sequences should now map to command 1
    assert_eq!(core.get_command_by_sequence(100), Some(cmd_id(1)));
    assert_eq!(core.get_command_by_sequence(101), Some(cmd_id(1)));
    assert_eq!(core.seq_to_cmd.len(), 2);
    assert_eq!(core.cmd_to_seqs.len(), 1); // Still one command

    // Simulate another retry
    core.register_sequence(cmd_id(1), 102);

    // All three sequences should map to command 1
    assert_eq!(core.get_command_by_sequence(100), Some(cmd_id(1)));
    assert_eq!(core.get_command_by_sequence(101), Some(cmd_id(1)));
    assert_eq!(core.get_command_by_sequence(102), Some(cmd_id(1)));
    assert_eq!(core.seq_to_cmd.len(), 3);

    // Finish the command - all sequences should be cleaned up
    core.finish_sequence(cmd_id(1));
    core.commands.remove(&cmd_id(1));

    // No sequences should remain
    assert_eq!(core.get_command_by_sequence(100), None);
    assert_eq!(core.get_command_by_sequence(101), None);
    assert_eq!(core.get_command_by_sequence(102), None);
    assert_eq!(core.seq_to_cmd.len(), 0);
    assert_eq!(core.cmd_to_seqs.len(), 0);
}

#[test]
fn test_late_reply_after_completion_ignored() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let priority = Priority::Normal;
    let _category = CommandCategory::Movement;
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );

    // Register command with sequences from multiple retries
    core.register_pending_ack(cmd_id(1), command.clone(), priority, camera_id, now);
    core.register_sequence(cmd_id(1), 100);
    core.register_sequence(cmd_id(1), 101); // Retry 1
    core.register_sequence(cmd_id(1), 102); // Retry 2

    // Verify all sequences are active
    assert_eq!(core.get_command_by_sequence(100), Some(cmd_id(1)));
    assert_eq!(core.get_command_by_sequence(101), Some(cmd_id(1)));
    assert_eq!(core.get_command_by_sequence(102), Some(cmd_id(1)));

    // Complete the command (simulating success on the third attempt)
    let response = Response::Completion {
        socket: Some(ViscaSocket::S1),
    };
    let source = ReplySource::from_fields(Some(cmd_id(1)), Some(102), Some(ViscaSocket::S1));
    let event = SchedulerEvent::Completion { source, response };

    let actions = core.process_event(event, now);
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        SchedulerAction::CommandComplete { id, .. } => {
            assert_eq!(*id, cmd_id(1));
        }
        _ => panic!("Expected CommandComplete"),
    }

    // Now simulate a late reply from an earlier sequence
    // This should be ignored since the command is already completed
    assert_eq!(core.get_command_by_sequence(100), None);
    assert_eq!(core.get_command_by_sequence(101), None);
    assert_eq!(core.get_command_by_sequence(102), None);

    // Verify all mappings are cleaned up
    assert_eq!(core.seq_to_cmd.len(), 0);
    assert_eq!(core.cmd_to_seqs.len(), 0);
}

#[test]
fn test_sequence_cap_at_max() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let priority = Priority::Normal;
    let _category = CommandCategory::Movement;
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );

    // Register command
    core.register_pending_ack(cmd_id(1), command.clone(), priority, camera_id, now);

    // Register more than MAX_SEQUENCES_PER_CMD (8) sequences
    for seq in 100..110 {
        core.register_sequence(cmd_id(1), seq);
    }

    // Only the last 8 sequences should be active (102-109)
    // 100 and 101 should have been dropped
    assert_eq!(core.get_command_by_sequence(100), None); // Dropped
    assert_eq!(core.get_command_by_sequence(101), None); // Dropped

    for seq in 102..110 {
        assert_eq!(core.get_command_by_sequence(seq), Some(cmd_id(1)));
    }

    // Verify we have exactly 8 sequence mappings
    assert!(core.seq_to_cmd.len() <= 8);
}

#[test]
fn test_multiple_commands_with_sequences() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let priority = Priority::Normal;
    let _category = CommandCategory::Movement;
    let camera_id = CameraId::CAMERA_1;

    // Register two different commands
    let cmd1 = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    let cmd2 = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );

    core.register_pending_ack(cmd_id(1), cmd1.clone(), priority, camera_id, now);
    core.register_pending_ack(cmd_id(2), cmd2.clone(), priority, camera_id, now);

    // Command 1 has sequences 100, 101 (retry)
    core.register_sequence(cmd_id(1), 100);
    core.register_sequence(cmd_id(1), 101);

    // Command 2 has sequences 200, 201, 202 (two retries)
    core.register_sequence(cmd_id(2), 200);
    core.register_sequence(cmd_id(2), 201);
    core.register_sequence(cmd_id(2), 202);

    // Verify all sequences map correctly
    assert_eq!(core.get_command_by_sequence(100), Some(cmd_id(1)));
    assert_eq!(core.get_command_by_sequence(101), Some(cmd_id(1)));
    assert_eq!(core.get_command_by_sequence(200), Some(cmd_id(2)));
    assert_eq!(core.get_command_by_sequence(201), Some(cmd_id(2)));
    assert_eq!(core.get_command_by_sequence(202), Some(cmd_id(2)));

    // Complete command 1
    core.finish_sequence(cmd_id(1));
    core.commands.remove(&cmd_id(1));

    // Command 1's sequences should be gone, command 2's should remain
    assert_eq!(core.get_command_by_sequence(100), None);
    assert_eq!(core.get_command_by_sequence(101), None);
    assert_eq!(core.get_command_by_sequence(200), Some(cmd_id(2)));
    assert_eq!(core.get_command_by_sequence(201), Some(cmd_id(2)));
    assert_eq!(core.get_command_by_sequence(202), Some(cmd_id(2)));

    // Complete command 2
    core.finish_sequence(cmd_id(2));
    core.commands.remove(&cmd_id(2));

    // All sequences should be cleaned up
    assert_eq!(core.get_command_by_sequence(200), None);
    assert_eq!(core.get_command_by_sequence(201), None);
    assert_eq!(core.get_command_by_sequence(202), None);
    assert_eq!(core.seq_to_cmd.len(), 0);
    assert_eq!(core.cmd_to_seqs.len(), 0);
}

#[test]
fn test_16_bit_sequence_fallback() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let priority = Priority::Normal;
    let _category = CommandCategory::Movement;
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );

    // Register a command with a 32-bit sequence that has non-zero high 16 bits
    let full_sequence = 0x12345678u32; // High 16 bits: 0x1234, Low 16 bits: 0x5678
    core.register_pending_ack(cmd_id(1), command.clone(), priority, camera_id, now);
    core.register_sequence(cmd_id(1), full_sequence);

    // Verify that both 32-bit and 16-bit lookups work
    assert_eq!(core.get_command_by_sequence(full_sequence), Some(cmd_id(1)));
    assert_eq!(core.get_command_by_sequence(0x5678), Some(cmd_id(1))); // Should find by lower 16 bits

    // Verify that a non-matching 16-bit value doesn't work
    assert_eq!(core.get_command_by_sequence(0x1234), None);
}

#[test]
fn test_16_bit_sequence_ambiguity() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let priority = Priority::Normal;
    let _category = CommandCategory::Movement;
    let camera_id = CameraId::CAMERA_1;

    // Register two commands with different 32-bit sequences but same lower 16 bits
    let seq1 = 0x12345678u32;
    let seq2 = 0xABCD5678u32; // Same lower 16 bits (0x5678) but different high bits

    let cmd1 = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    let cmd2 = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );

    core.register_pending_ack(cmd_id(1), cmd1, priority, camera_id, now);
    core.register_sequence(cmd_id(1), seq1);

    core.register_pending_ack(cmd_id(2), cmd2, priority, camera_id, now);
    core.register_sequence(cmd_id(2), seq2);

    // Both 32-bit sequences should work
    assert_eq!(core.get_command_by_sequence(seq1), Some(cmd_id(1)));
    assert_eq!(core.get_command_by_sequence(seq2), Some(cmd_id(2)));

    // 16-bit lookup should be ambiguous and return None
    assert_eq!(core.get_command_by_sequence(0x5678), None);
}

#[test]
fn test_16_bit_sequence_cleanup() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let priority = Priority::Normal;
    let _category = CommandCategory::Movement;
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );

    // Register a command with sequence
    let sequence = 0x12345678u32;
    core.register_pending_ack(cmd_id(1), command, priority, camera_id, now);
    core.register_sequence(cmd_id(1), sequence);

    // Verify both mappings exist
    assert_eq!(core.get_command_by_sequence(sequence), Some(cmd_id(1)));
    assert_eq!(core.get_command_by_sequence(0x5678), Some(cmd_id(1)));

    // Finish the sequence
    core.finish_sequence(cmd_id(1));
    core.commands.remove(&cmd_id(1));

    // Verify both mappings are cleaned up
    assert_eq!(core.get_command_by_sequence(sequence), None);
    assert_eq!(core.get_command_by_sequence(0x5678), None);
}

#[test]
fn test_16_bit_sequence_collision_recovers_after_finish() {
    // This test verifies the key fix from issue #457:
    // When two commands collide on a 16-bit sequence and one finishes,
    // the remaining command should become uniquely resolvable.
    //
    // Previously, this would fail because finish_sequence would delete the
    // seq16_to_cmd entry entirely, making the remaining command unreachable.
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let priority = Priority::Normal;
    let _category = CommandCategory::Movement;
    let camera_id = CameraId::CAMERA_1;

    // Create two commands with different 32-bit sequences but same lower 16 bits
    let seq1 = 0x12345678u32; // Lower 16 bits: 0x5678
    let seq2 = 0xABCD5678u32; // Lower 16 bits: 0x5678 (collision!)

    let cmd1 = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    let cmd2 = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );

    // Register both commands
    core.register_pending_ack(cmd_id(1), cmd1, priority, camera_id, now);
    core.register_sequence(cmd_id(1), seq1);

    core.register_pending_ack(cmd_id(2), cmd2, priority, camera_id, now);
    core.register_sequence(cmd_id(2), seq2);

    // Both 32-bit sequences should work
    assert_eq!(core.get_command_by_sequence(seq1), Some(cmd_id(1)));
    assert_eq!(core.get_command_by_sequence(seq2), Some(cmd_id(2)));

    // 16-bit lookup should be ambiguous and return None (both commands active)
    assert_eq!(
        core.get_command_by_sequence(0x5678),
        None,
        "16-bit lookup should be ambiguous while both commands are active"
    );

    // Now finish command 1
    core.finish_sequence(cmd_id(1));
    core.commands.remove(&cmd_id(1));

    // Command 2's 32-bit sequence should still work
    assert_eq!(core.get_command_by_sequence(seq2), Some(cmd_id(2)));

    // KEY FIX: 16-bit lookup should NOW return command 2 (no longer ambiguous!)
    // This works for both explicit 16-bit values and full 32-bit values that
    // share the same lower 16 bits
    assert_eq!(
        core.get_command_by_sequence(0x5678),
        Some(cmd_id(2)),
        "After finishing command 1, 16-bit lookup should uniquely resolve to command 2"
    );

    // Note: Looking up seq1 (0x12345678) will also find command 2 via 16-bit fallback
    // because the 32-bit exact match fails and the 16-bit (0x5678) uniquely matches
    // command 2. This is expected behavior for truncated sequence resolution.
    assert_eq!(
        core.get_command_by_sequence(seq1),
        Some(cmd_id(2)),
        "seq1's lower 16 bits match seq2, so 16-bit fallback finds command 2"
    );
}

#[test]
fn test_16_bit_sequence_collision_recovers_finish_order_reversed() {
    // Same as above, but finish command 2 first to ensure symmetry
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let priority = Priority::Normal;
    let _category = CommandCategory::Movement;
    let camera_id = CameraId::CAMERA_1;

    let seq1 = 0x12345678u32;
    let seq2 = 0xABCD5678u32;

    let cmd1 = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    let cmd2 = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );

    core.register_pending_ack(cmd_id(1), cmd1, priority, camera_id, now);
    core.register_sequence(cmd_id(1), seq1);

    core.register_pending_ack(cmd_id(2), cmd2, priority, camera_id, now);
    core.register_sequence(cmd_id(2), seq2);

    // Both active: 16-bit should be ambiguous
    assert_eq!(core.get_command_by_sequence(0x5678), None);

    // Finish command 2 first (reversed order from previous test)
    core.finish_sequence(cmd_id(2));
    core.commands.remove(&cmd_id(2));

    // Command 1's sequence should still work
    assert_eq!(core.get_command_by_sequence(seq1), Some(cmd_id(1)));

    // 16-bit lookup should now return command 1
    assert_eq!(
        core.get_command_by_sequence(0x5678),
        Some(cmd_id(1)),
        "After finishing command 2, 16-bit lookup should uniquely resolve to command 1"
    );

    // Note: seq2 (0xABCD5678) will also find command 1 via 16-bit fallback
    // since its 32-bit exact match fails but 16-bit (0x5678) uniquely matches command 1
    assert_eq!(
        core.get_command_by_sequence(seq2),
        Some(cmd_id(1)),
        "seq2's lower 16 bits match seq1, so 16-bit fallback finds command 1"
    );
}

#[test]
fn test_16_bit_eviction_under_collision() {
    // This test verifies that when a command's 16-bit sequence is evicted due to
    // the 8-sequence cap, it doesn't break another command's ownership of that sequence.
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let priority = Priority::Normal;
    let _category = CommandCategory::Movement;
    let camera_id = CameraId::CAMERA_1;

    // Command 1: starts with seq16 = 0x5678
    let seq1_initial = 0x12345678u32;

    // Command 2: also uses seq16 = 0x5678 (collision)
    let seq2 = 0xABCD5678u32;

    let cmd1 = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    let cmd2 = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );

    // Register command 1 with its initial sequence
    core.register_pending_ack(cmd_id(1), cmd1, priority, camera_id, now);
    core.register_sequence(cmd_id(1), seq1_initial);

    // Register command 2 (collision on 0x5678)
    core.register_pending_ack(cmd_id(2), cmd2, priority, camera_id, now);
    core.register_sequence(cmd_id(2), seq2);

    // Both commands own 0x5678 - ambiguous
    assert_eq!(core.get_command_by_sequence(0x5678), None);

    // Now simulate retries for command 1 that eventually evict its 0x5678 entry
    // Add 8 more sequences with DIFFERENT 16-bit values to command 1
    for i in 0u32..8 {
        // Generate sequences with different lower 16 bits
        let retry_seq = 0x1234_0000 + i + 1; // 0x12340001, 0x12340002, ..., 0x12340008
        core.register_sequence(cmd_id(1), retry_seq);
    }

    // Command 1's original 0x5678 should be evicted from its history
    // But command 2 should still own 0x5678!
    assert_eq!(
        core.get_command_by_sequence(0x5678),
        Some(cmd_id(2)),
        "After command 1's 0x5678 is evicted, command 2 should uniquely own it"
    );

    // Command 2's full sequence should still work
    assert_eq!(core.get_command_by_sequence(seq2), Some(cmd_id(2)));
}

#[test]
fn test_raw_visca_content_based_matching() {
    // This test verifies that the scheduler core correctly handles
    // out-of-order inquiry replies in raw VISCA mode.
    // The actual content-based matching happens in the adapter layer,
    // but the core should correctly process the events with proper cmd_id.

    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);
    let now = Instant::now();
    let priority = Priority::Normal;
    let _category = CommandCategory::Quick;
    let camera_id = CameraId::CAMERA_1;

    // Start two different inquiries in raw VISCA mode
    let power_cmd = create_test_command(
        vec![0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR],
        Some(InquiryKind::Power),
        CommandCategory::Quick,
        camera_id,
    );
    let zoom_cmd = create_test_command(
        vec![0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR],
        Some(InquiryKind::ZoomPosition),
        CommandCategory::Quick,
        camera_id,
    );

    core.start_inquiry(cmd_id(1), power_cmd, priority, camera_id, now);
    core.start_inquiry(cmd_id(2), zoom_cmd, priority, camera_id, now);

    // Verify both are tracked
    assert_eq!(core.inquiries_order.len(), 2);
    assert!(core.inflight_inquiry_ids.contains(&cmd_id(1)));
    assert!(core.inflight_inquiry_ids.contains(&cmd_id(2)));

    // Process replies out of order
    // Second inquiry (zoom) reply arrives first - with explicit cmd_id
    // (This simulates the adapter layer doing content-based matching)
    let zoom_response =
        Response::Inquiry(crate::command::InquiryData::ZoomPosition { position: 0x1234 });
    // Content-based matching identified this as inquiry 2, raw VISCA (no sequence)
    let source2 = ReplySource::from_fields(Some(cmd_id(2)), None, None);
    let event2 = SchedulerEvent::InquiryReply {
        source: source2,
        response: zoom_response,
    };

    let actions = core.process_event(event2, now);
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        SchedulerAction::CommandComplete { id, response, .. } => {
            assert_eq!(*id, cmd_id(2)); // Second inquiry completed
                                        // Verify it's a zoom response
            match response {
                Response::Inquiry(crate::command::InquiryData::ZoomPosition { position }) => {
                    assert_eq!(*position, 0x1234);
                }
                _ => panic!("Expected ZoomPosition response"),
            }
        }
        _ => panic!("Expected CommandComplete action"),
    }

    // First inquiry (power) reply arrives second
    let power_response = Response::Inquiry(crate::command::InquiryData::Power { on: true });
    // Content-based matching identified this as inquiry 1, raw VISCA (no sequence)
    let source1 = ReplySource::from_fields(Some(cmd_id(1)), None, None);
    let event1 = SchedulerEvent::InquiryReply {
        source: source1,
        response: power_response,
    };

    let actions = core.process_event(event1, now);
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        SchedulerAction::CommandComplete { id, response, .. } => {
            assert_eq!(*id, cmd_id(1)); // First inquiry completed
                                        // Verify it's a power response
            match response {
                Response::Inquiry(crate::command::InquiryData::Power { on }) => {
                    assert!(*on);
                }
                _ => panic!("Expected Power response"),
            }
        }
        _ => panic!("Expected CommandComplete action"),
    }

    // All inquiries should be completed
    assert_eq!(core.inquiries_order.len(), 0);
    assert!(!core.inflight_inquiry_ids.contains(&cmd_id(1)));
    assert!(!core.inflight_inquiry_ids.contains(&cmd_id(2)));
}

// Tests for issue #362: send-failure retry behavior

#[test]
fn test_send_failure_retry_with_transport_error_flag() {
    let mut core = SchedulerCore::with_retry_config(
        TimeoutConfig::default(),
        RetryConfig {
            max_retries: 2,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Constant,
        },
    );

    let cmd_id = cmd_id(1);
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    let now = Instant::now();

    // Register command metadata (simulating a command that was sent but failed)
    core.register_pending_ack(
        cmd_id,
        command.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // Mark as transport error (simulating send failure)
    core.mark_retry_as_transport_error(cmd_id);

    // Queue retry - should succeed
    let action = core.queue_retry_for_command(cmd_id, now);

    // Should get a retry action
    match action {
        Some(SchedulerAction::RetryCommand { id, delay, .. }) => {
            assert_eq!(id, cmd_id);
            assert!(delay > Duration::ZERO);
        }
        _ => panic!("Expected RetryCommand action, got: {:?}", action),
    }

    // Verify retry is queued
    let retries = core.get_ready_retries(now + Duration::from_millis(200));
    assert_eq!(retries.len(), 1);
    assert_eq!(retries[0].id, cmd_id);
    assert_eq!(retries[0].attempt, 1);
}

#[test]
fn test_transport_error_classification_after_exhausted_retries() {
    let mut core = SchedulerCore::with_retry_config(
        TimeoutConfig::default(),
        RetryConfig {
            max_retries: 1, // Only 1 retry allowed
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Constant,
        },
    );

    let cmd_id = cmd_id(1);
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    let now = Instant::now();

    // Register command metadata
    core.register_pending_ack(
        cmd_id,
        command.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // First retry - should succeed
    core.mark_retry_as_transport_error(cmd_id);
    let action = core.queue_retry_for_command(cmd_id, now);
    assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));

    // Second retry - should fail with TransportError
    core.mark_retry_as_transport_error(cmd_id);
    let action = core.queue_retry_for_command(cmd_id, now);

    match action {
        Some(SchedulerAction::CommandFailed { id, error }) => {
            assert_eq!(id, cmd_id);
            match error {
                Error::TransportError(msg) => {
                    assert!(
                        msg.contains("Network error after max retries"),
                        "Expected 'Network error after max retries', got: {}",
                        msg
                    );
                }
                _ => panic!("Expected TransportError, got: {:?}", error),
            }
        }
        _ => panic!("Expected CommandFailed action, got: {:?}", action),
    }
}

#[test]
fn test_timeout_classification_without_transport_error_flag() {
    let mut core = SchedulerCore::with_retry_config(
        TimeoutConfig::default(),
        RetryConfig {
            max_retries: 1,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Constant,
        },
    );

    let cmd_id = cmd_id(1);
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    let now = Instant::now();

    // Register command metadata
    core.register_pending_ack(
        cmd_id,
        command.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // First retry WITHOUT marking as transport error
    let action = core.queue_retry_for_command(cmd_id, now);
    assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));

    // Second retry - should fail with Timeout (not TransportError)
    let action = core.queue_retry_for_command(cmd_id, now);

    match action {
        Some(SchedulerAction::CommandFailed { id, error }) => {
            assert_eq!(id, cmd_id);
            match error {
                Error::Timeout => {
                    // Expected - timeout when not marked as transport error
                }
                _ => panic!("Expected Timeout error, got: {:?}", error),
            }
        }
        _ => panic!("Expected CommandFailed action, got: {:?}", action),
    }
}

#[test]
fn test_multiple_commands_with_transport_errors() {
    let mut core = SchedulerCore::with_retry_config(
        TimeoutConfig::default(),
        RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(50),
            max_retry_duration: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Exponential,
        },
    );

    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;

    // Register multiple commands
    for i in 1..=3 {
        let command = create_test_command(
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
            None,
            CommandCategory::Movement,
            camera_id,
        );
        core.register_pending_ack(
            cmd_id(i),
            command,
            Priority::Normal,
            CameraId::CAMERA_1,
            now,
        );

        // Mark as transport error and queue retry
        core.mark_retry_as_transport_error(cmd_id(i));
        let action = core.queue_retry_for_command(cmd_id(i), now);
        assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));
    }

    // Get all ready retries
    let retries = core.get_ready_retries(now + Duration::from_secs(1));
    assert_eq!(retries.len(), 3, "Should have 3 retries queued");

    // Verify all have attempt = 1
    for retry in retries {
        assert_eq!(retry.attempt, 1);
        assert!([cmd_id(1), cmd_id(2), cmd_id(3)].contains(&retry.id));
    }
}

#[test]
fn test_ack_without_socket_nibble_s1_free() {
    // Test: ACK with socket: None, S1 free => assign S1
    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let now = Instant::now();

    // Register a command
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    core.register_pending_ack(
        cmd_id(1),
        command,
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // Send ACK without socket nibble
    let source = ReplySource::from_fields(Some(cmd_id(1)), None, None);
    let event = SchedulerEvent::Ack { source };
    core.process_event(event, now);

    // Verify S1 was assigned
    let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
    assert!(!free, "S1 should be occupied");
    assert_eq!(socket_cmd_id, Some(cmd_id(1)), "Command 1 should be on S1");

    // Verify S2 is still free
    let (free, _, _) = core.socket_state(ViscaSocket::S2);
    assert!(free, "S2 should still be free");
}

#[test]
fn test_ack_without_socket_nibble_s1_busy_s2_free() {
    // Test: ACK with socket: None, S1 busy, S2 free => assign S2
    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;

    // Register two commands
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    core.register_pending_ack(
        cmd_id(1),
        command.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );
    core.register_pending_ack(
        cmd_id(2),
        command,
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // First ACK assigns S1
    let source = ReplySource::from_fields(Some(cmd_id(1)), None, Some(ViscaSocket::S1));
    let event = SchedulerEvent::Ack { source };
    core.process_event(event, now);

    // Second ACK without socket nibble should get S2
    let source = ReplySource::from_fields(Some(cmd_id(2)), None, None);
    let event = SchedulerEvent::Ack { source };
    core.process_event(event, now);

    // Verify S1 has command 1
    let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
    assert!(!free, "S1 should be occupied");
    assert_eq!(socket_cmd_id, Some(cmd_id(1)), "Command 1 should be on S1");

    // Verify S2 has command 2
    let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S2);
    assert!(!free, "S2 should be occupied");
    assert_eq!(socket_cmd_id, Some(cmd_id(2)), "Command 2 should be on S2");
}

#[test]
fn test_ack_without_socket_nibble_both_busy() {
    // Test: ACK with socket: None, both busy => no assignment
    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;

    // Register three commands
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    for i in 1..=3 {
        core.register_pending_ack(
            cmd_id(i),
            command.clone(),
            Priority::Normal,
            CameraId::CAMERA_1,
            now,
        );
    }

    // First two ACKs occupy both sockets
    let source = ReplySource::from_fields(Some(cmd_id(1)), None, Some(ViscaSocket::S1));
    let event = SchedulerEvent::Ack { source };
    core.process_event(event, now);

    let source = ReplySource::from_fields(Some(cmd_id(2)), None, Some(ViscaSocket::S2));
    let event = SchedulerEvent::Ack { source };
    core.process_event(event, now);

    // Third ACK without socket nibble should fail
    let source = ReplySource::from_fields(Some(cmd_id(3)), None, None);
    let event = SchedulerEvent::Ack { source };
    core.process_event(event, now);

    // Verify command 3 is still pending
    assert!(
        core.pending_ack_ids.contains(&cmd_id(3)),
        "Command 3 should still be pending"
    );

    // Verify sockets are still occupied by commands 1 and 2
    let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
    assert!(!free, "S1 should be occupied");
    assert_eq!(socket_cmd_id, Some(cmd_id(1)), "Command 1 should be on S1");

    let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S2);
    assert!(!free, "S2 should be occupied");
    assert_eq!(socket_cmd_id, Some(cmd_id(2)), "Command 2 should be on S2");
}

#[test]
fn test_ack_with_busy_socket_fallback() {
    // Test: ACK requests busy socket, fallback to free socket
    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;

    // Register two commands
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    core.register_pending_ack(
        cmd_id(1),
        command.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );
    core.register_pending_ack(
        cmd_id(2),
        command,
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // First ACK assigns S1
    let source = ReplySource::from_fields(Some(cmd_id(1)), None, Some(ViscaSocket::S1));
    let event = SchedulerEvent::Ack { source };
    core.process_event(event, now);

    // Second ACK requests S1 (busy), should fallback to S2
    let source = ReplySource::from_fields(Some(cmd_id(2)), None, Some(ViscaSocket::S1));
    let event = SchedulerEvent::Ack { source };
    core.process_event(event, now);

    // Verify S1 still has command 1
    let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
    assert!(!free, "S1 should be occupied");
    assert_eq!(socket_cmd_id, Some(cmd_id(1)), "Command 1 should be on S1");

    // Verify S2 has command 2 (fallback allocation)
    let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S2);
    assert!(!free, "S2 should be occupied");
    assert_eq!(
        socket_cmd_id,
        Some(cmd_id(2)),
        "Command 2 should be on S2 (fallback)"
    );
}

#[test]
fn test_command_kind_preserved_through_retries() {
    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let now = Instant::now();

    // Test case: A Command with bytes[1] == 0x09 (synthetic vendor command)
    // This would have been misclassified as Inquiry by the old heuristic

    // Create a dummy command for testing
    #[derive(Debug, Clone)]
    struct TestCommand;
    impl crate::command::encode::ViscaCommand for TestCommand {
        type Response = ();
        const MAX_SIZE: usize = 6;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Movement;

        fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            // Encode a command with bytes[1] == 0x09 that would be misclassified
            buffer[0] = 0x81;
            buffer[1] = 0x09;
            buffer[2] = 0x04;
            buffer[3] = 0x00;
            buffer[4] = 0x01;
            buffer[5] = VISCA_TERMINATOR;
            Ok(6)
        }

        fn response_kind(&self) -> Option<InquiryKind> {
            None // This is a command, not an inquiry
        }
    }

    let test_command = Arc::new(EncodedCommand::new(&TestCommand, CameraId::CAMERA_1).unwrap());
    let test_cmd_id = cmd_id(1);

    // Register as Command explicitly
    core.register_pending_ack(
        test_cmd_id,
        test_command.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // Mark as transport error and queue retry
    core.mark_retry_as_transport_error(test_cmd_id);
    let action = core.queue_retry_for_command(test_cmd_id, now);

    // Should get a retry action
    assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));

    // Get the retry and verify the kind is preserved
    let retries = core.get_ready_retries(now + Duration::from_millis(200));
    assert_eq!(retries.len(), 1);
    let retry = &retries[0];

    // The key assertion: kind should be Command, not Inquiry
    // This verifies that we no longer use the bytes[1] == 0x09 heuristic
    assert_eq!(retry.kind(), CommandKind::Command);
    assert_eq!(retry.id, test_cmd_id);
    // Verify the command is preserved (same Arc)
    assert!(Arc::ptr_eq(&retry.command, &test_command));

    // Test case 2: A normal Inquiry to ensure it also preserves correctly
    let mut core2 = SchedulerCore::new(TimeoutConfig::default());

    // Create a dummy inquiry for testing
    #[derive(Debug, Clone)]
    struct TestInquiry;
    impl crate::command::encode::ViscaCommand for TestInquiry {
        type Response = ();
        const MAX_SIZE: usize = 6;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

        fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = 0x81;
            buffer[1] = 0x09;
            buffer[2] = 0x04;
            buffer[3] = 0x00;
            buffer[4] = 0x02;
            buffer[5] = VISCA_TERMINATOR;
            Ok(6)
        }

        fn response_kind(&self) -> Option<InquiryKind> {
            Some(InquiryKind::Power) // This is an inquiry
        }
    }

    let test_inquiry = Arc::new(EncodedCommand::new(&TestInquiry, CameraId::CAMERA_1).unwrap());
    let inquiry_id = cmd_id(2);

    // Start as inquiry
    core2.start_inquiry(
        inquiry_id,
        test_inquiry.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // Mark as transport error and queue retry
    core2.mark_retry_as_transport_error(inquiry_id);
    let inquiry_action = core2.queue_retry_for_command(inquiry_id, now);

    // Should get a retry action
    assert!(matches!(
        inquiry_action,
        Some(SchedulerAction::RetryCommand { .. })
    ));

    // Get the retry and verify inquiry kind is preserved
    let inquiry_retries = core2.get_ready_retries(now + Duration::from_millis(200));
    assert_eq!(inquiry_retries.len(), 1);
    let inquiry_retry = &inquiry_retries[0];

    // Verify Inquiry kind is preserved
    assert_eq!(inquiry_retry.kind(), CommandKind::Inquiry);
    assert_eq!(inquiry_retry.id, inquiry_id);
    // Verify the inquiry is preserved (same Arc)
    assert!(Arc::ptr_eq(&inquiry_retry.command, &test_inquiry));
}

#[test]
fn test_inquiry_bypasses_socket_gate() {
    // This test verifies that inquiries can be sent even when both command sockets are occupied
    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let now = Instant::now();

    // Create two commands and one inquiry
    let command1 = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]),
        kind: CommandKind::Command,
        category: CommandCategory::Movement,
        response_type: None,
    });
    let command2 = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR]),
        kind: CommandKind::Command,
        category: CommandCategory::Movement,
        response_type: None,
    });
    let inquiry = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
        kind: CommandKind::Inquiry,
        category: CommandCategory::Quick,
        response_type: Some(InquiryKind::ZoomPosition),
    });

    // Queue both commands
    core.queue_command(PendingCommand {
        id: cmd_id(1),
        command: command1.clone(),
        priority: Priority::Normal,
        camera_id: CameraId::CAMERA_1,
        submitted_at: now,
    });
    core.queue_command(PendingCommand {
        id: cmd_id(2),
        command: command2.clone(),
        priority: Priority::Normal,
        camera_id: CameraId::CAMERA_1,
        submitted_at: now,
    });

    // Send and register both commands as pending ACK
    let cmd1 = core.next_item_to_send(now).unwrap();
    assert_eq!(cmd1.id, cmd_id(1));
    core.register_pending_ack(
        cmd_id(1),
        command1.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    let cmd2 = core.next_item_to_send(now).unwrap();
    assert_eq!(cmd2.id, cmd_id(2));
    core.register_pending_ack(
        cmd_id(2),
        command2.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // Now both commands are pending ACK - sockets are at capacity
    assert!(!core.can_send_command());
    assert_eq!(core.next_item_to_send(now), None); // No commands can be sent

    // Queue an inquiry
    core.queue_command(PendingCommand {
        id: cmd_id(3),
        command: inquiry.clone(),
        priority: Priority::Normal,
        camera_id: CameraId::CAMERA_1,
        submitted_at: now,
    });

    // The inquiry should be sendable even though command sockets are full
    let inq = core.next_item_to_send(now);
    assert!(inq.is_some());
    let inq = inq.unwrap();
    assert_eq!(inq.id, cmd_id(3));
    assert_eq!(inq.kind(), CommandKind::Inquiry);

    // Start the inquiry (track it in flight)
    core.start_inquiry(
        cmd_id(3),
        inquiry.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // Queue another command - should not be sendable
    core.queue_command(PendingCommand {
        id: cmd_id(4),
        command: command1.clone(),
        priority: Priority::Normal,
        camera_id: CameraId::CAMERA_1,
        submitted_at: now,
    });

    // No more commands should be sendable (sockets still full)
    assert_eq!(core.next_item_to_send(now), None);

    // Queue another inquiry - should be sendable
    core.queue_command(PendingCommand {
        id: cmd_id(5),
        command: inquiry.clone(),
        priority: Priority::Normal,
        camera_id: CameraId::CAMERA_1,
        submitted_at: now,
    });

    let inq2 = core.next_item_to_send(now);
    assert!(inq2.is_some());
    assert_eq!(inq2.unwrap().id, cmd_id(5));
}

#[test]
fn test_inquiry_pipeline_limit() {
    // Test that inquiries respect the max_inquiries_inflight limit
    let mut core = SchedulerCore::new(TimeoutConfig::default());
    core.set_max_inquiries_inflight(2); // Set a low limit for testing
    let now = Instant::now();

    let inquiry = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
        kind: CommandKind::Inquiry,
        category: CommandCategory::Quick,
        response_type: Some(InquiryKind::ZoomPosition),
    });

    // Queue 3 inquiries
    for id in 1..=3 {
        core.queue_command(PendingCommand {
            id: cmd_id(id),
            command: inquiry.clone(),
            priority: Priority::Normal,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
        });
    }

    // First inquiry should be sendable
    let inq1 = core.next_item_to_send(now);
    assert!(inq1.is_some());
    assert_eq!(inq1.unwrap().id, cmd_id(1));
    core.start_inquiry(
        cmd_id(1),
        inquiry.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // Second inquiry should be sendable
    let inq2 = core.next_item_to_send(now);
    assert!(inq2.is_some());
    assert_eq!(inq2.unwrap().id, cmd_id(2));
    core.start_inquiry(
        cmd_id(2),
        inquiry.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // Third inquiry should NOT be sendable (limit reached)
    assert!(!core.can_send_inquiry(now));
    let inq3 = core.next_item_to_send(now);
    assert!(inq3.is_none());

    // Complete one inquiry by removing it from inflight
    core.inflight_inquiry_ids.remove(&cmd_id(1));
    core.commands.remove(&cmd_id(1));

    // Now the third inquiry should be sendable
    assert!(core.can_send_inquiry(now));
    let inq3 = core.next_item_to_send(now);
    assert!(inq3.is_some());
    assert_eq!(inq3.unwrap().id, cmd_id(3));
}

#[test]
fn test_mixed_priority_queue_ordering() {
    // Test that priority-aware selection works correctly across both queues.
    // Higher priority commands should preempt lower priority inquiries,
    // preventing command starvation from background polling.
    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let now = Instant::now();

    let command = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]),
        kind: CommandKind::Command,
        category: CommandCategory::Movement,
        response_type: None,
    });
    let inquiry = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
        kind: CommandKind::Inquiry,
        category: CommandCategory::Quick,
        response_type: Some(InquiryKind::ZoomPosition),
    });

    // Queue items with different priorities
    core.queue_command(PendingCommand {
        id: cmd_id(1),
        command: command.clone(),
        priority: Priority::Low,
        camera_id: CameraId::CAMERA_1,
        submitted_at: now,
    });

    core.queue_command(PendingCommand {
        id: cmd_id(2),
        command: inquiry.clone(),
        priority: Priority::High,
        camera_id: CameraId::CAMERA_1,
        submitted_at: now,
    });

    core.queue_command(PendingCommand {
        id: cmd_id(3),
        command: command.clone(),
        priority: Priority::Critical,
        camera_id: CameraId::CAMERA_1,
        submitted_at: now,
    });

    core.queue_command(PendingCommand {
        id: cmd_id(4),
        command: inquiry.clone(),
        priority: Priority::Normal,
        camera_id: CameraId::CAMERA_1,
        submitted_at: now,
    });

    // Critical priority command should come first (Critical > High > Normal > Low)
    let item1 = core.next_item_to_send(now).unwrap();
    assert_eq!(item1.id, cmd_id(3));
    assert_eq!(item1.priority, Priority::Critical);

    // High priority inquiry second (preempts Normal priority inquiry)
    let item2 = core.next_item_to_send(now).unwrap();
    assert_eq!(item2.id, cmd_id(2));
    assert_eq!(item2.priority, Priority::High);

    // Normal priority inquiry (no higher priority items remaining)
    let item3 = core.next_item_to_send(now).unwrap();
    assert_eq!(item3.id, cmd_id(4));
    assert_eq!(item3.priority, Priority::Normal);

    // Low priority command last
    let item4 = core.next_item_to_send(now).unwrap();
    assert_eq!(item4.id, cmd_id(1));
    assert_eq!(item4.priority, Priority::Low);
}

#[test]
fn test_high_priority_command_not_starved_by_normal_inquiries() {
    // Regression test for command starvation issue (GitHub issue #381):
    // When background polling generates many Normal-priority inquiries,
    // a High-priority command (like preset save) should not be blocked.
    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let now = Instant::now();

    let command = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x01, 0x04, 0x3F, 0x01, 0x05, VISCA_TERMINATOR]),
        kind: CommandKind::Command,
        category: CommandCategory::Movement,
        response_type: None,
    });
    let inquiry = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
        kind: CommandKind::Inquiry,
        category: CommandCategory::Quick,
        response_type: Some(InquiryKind::ZoomPosition),
    });

    // Simulate a burst of Normal-priority polling inquiries (typical background load)
    for i in 1..=10 {
        core.queue_command(PendingCommand {
            id: cmd_id(i),
            command: inquiry.clone(),
            priority: Priority::Normal,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
        });
    }

    // High-priority preset save command arrives (user action)
    core.queue_command(PendingCommand {
        id: cmd_id(100),
        command: command.clone(),
        priority: Priority::High,
        camera_id: CameraId::CAMERA_1,
        submitted_at: now,
    });

    // The High-priority command should be returned BEFORE the Normal-priority inquiries
    // This prevents command starvation from background polling
    let item = core.next_item_to_send(now).unwrap();
    assert_eq!(
        item.id,
        cmd_id(100),
        "High-priority command should not be starved by Normal-priority inquiries"
    );
    assert_eq!(item.priority, Priority::High);
    assert_eq!(item.kind(), CommandKind::Command);

    // Subsequent calls should return the Normal-priority inquiries
    let item2 = core.next_item_to_send(now).unwrap();
    assert_eq!(item2.priority, Priority::Normal);
    assert_eq!(item2.kind(), CommandKind::Inquiry);
}

#[test]
fn test_equal_priority_prefers_inquiry_for_backwards_compat() {
    // When command and inquiry have equal priority, prefer inquiry for backwards
    // compatibility (inquiries don't hold sockets and are typically faster).
    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let now = Instant::now();

    let command = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]),
        kind: CommandKind::Command,
        category: CommandCategory::Movement,
        response_type: None,
    });
    let inquiry = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
        kind: CommandKind::Inquiry,
        category: CommandCategory::Quick,
        response_type: Some(InquiryKind::ZoomPosition),
    });

    // Queue command first, then inquiry (both Normal priority)
    core.queue_command(PendingCommand {
        id: cmd_id(1),
        command: command.clone(),
        priority: Priority::Normal,
        camera_id: CameraId::CAMERA_1,
        submitted_at: now,
    });
    core.queue_command(PendingCommand {
        id: cmd_id(2),
        command: inquiry.clone(),
        priority: Priority::Normal,
        camera_id: CameraId::CAMERA_1,
        submitted_at: now,
    });

    // Inquiry should be preferred at equal priority
    let item = core.next_item_to_send(now).unwrap();
    assert_eq!(
        item.id,
        cmd_id(2),
        "Inquiry should be preferred at equal priority"
    );
    assert_eq!(item.kind(), CommandKind::Inquiry);

    // Then the command
    let item2 = core.next_item_to_send(now).unwrap();
    assert_eq!(item2.id, cmd_id(1));
    assert_eq!(item2.kind(), CommandKind::Command);
}

#[test]
fn test_immediate_error_without_ack_maps_to_most_recent_pending_command() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);
    let now = Instant::now();

    // Create two pending ACK commands; the second one is the most recent
    // Use Quick category so 0x41 error is NOT retryable (0x41 is only retryable for Movement/Preset)
    let camera_id = CameraId::CAMERA_1;
    let cmd1 = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]),
        kind: CommandKind::Command,
        category: CommandCategory::Quick,
        response_type: None,
    });
    let cmd2 = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x10, 0x05, VISCA_TERMINATOR]), // One Push Trigger
        kind: CommandKind::Command,
        category: CommandCategory::Quick,
        response_type: None,
    });

    core.register_pending_ack(cmd_id(1), cmd1, Priority::Normal, camera_id, now);
    core.register_pending_ack(
        cmd_id(2),
        cmd2,
        Priority::Normal,
        camera_id,
        now + Duration::from_millis(1),
    );

    // Simulate: camera returns 90 6y 41 FF (Not Executable) without a prior ACK
    // Raw VISCA (no sequence), so heuristic fallback is allowed
    let source = ReplySource::BySocket {
        socket: ViscaSocket::S2,
    };
    let event = SchedulerEvent::Error { source, code: 0x41 };
    let actions = core.process_event(event, now + Duration::from_millis(2));

    // The *most recent* pending command (id=2) should be failed immediately with 0x41
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        SchedulerAction::CommandFailed { id, error } => {
            assert_eq!(*id, cmd_id(2), "Newest pending command must be attributed");
            assert!(matches!(error, Error::CommandNotExecutable));
        }
        _ => panic!("Expected CommandFailed for id=2"),
    }
    // And it must be removed from pending_ack
    assert!(!core.is_command_pending(cmd_id(2)));
    assert!(core.is_command_pending(cmd_id(1)));
}

#[test]
fn test_error_without_socket_prefers_inflight_inquiry() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;

    // Start an inquiry (front of FIFO)
    let inq = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x81, 0x09, 0x04, 0x35, VISCA_TERMINATOR]), // WB Mode Inquiry
        kind: CommandKind::Inquiry,
        category: CommandCategory::Quick,
        response_type: Some(InquiryKind::Power), // any kind
    });
    core.start_inquiry(cmd_id(42), inq, Priority::Normal, camera_id, now);

    // Also have a pending ACK command in the background
    let cmd = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR]),
        kind: CommandKind::Command,
        category: CommandCategory::Movement,
        response_type: None,
    });
    core.register_pending_ack(cmd_id(99), cmd, Priority::Normal, camera_id, now);

    // Simulate an inquiry-style error: 90 60 EE FF (y=0 -> no socket field)
    // Raw VISCA (no sequence), so heuristic fallback is allowed
    let source = ReplySource::Unknown; // No sequence, no socket
    let event = SchedulerEvent::Error { source, code: 0x41 };
    let actions = core.process_event(event, now + Duration::from_millis(1));

    // It must fail the inflight inquiry (id=42), not the command
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        SchedulerAction::CommandFailed { id, error } => {
            assert_eq!(*id, cmd_id(42));
            assert!(matches!(error, Error::CommandNotExecutable));
        }
        _ => panic!("Expected CommandFailed for inquiry id=42"),
    }
    // Confirm the command is still pending
    assert!(core.is_command_pending(cmd_id(99)));
}

// =========================================================================
// Tests for issue #434: max_retry_duration enforcement in SchedulerCore
// =========================================================================

// Helper to create a simple test command for duration tests
fn make_duration_test_cmd(category: CommandCategory) -> Arc<EncodedCommand> {
    create_test_command(
        vec![0x81, 0x01, 0x00, VISCA_TERMINATOR],
        None,
        category,
        CameraId::CAMERA_1,
    )
}

#[test]
fn test_max_retry_duration_ack_timeout() {
    // Test that ACK timeout retries respect max_retry_duration
    // Use short timeouts for testing
    let timeout_config = TimeoutConfig::uniform(Duration::from_millis(50));
    let retry_config = RetryConfig {
        max_retries: 10, // High retry count to ensure duration is the limiting factor
        base_retry_delay: Duration::from_millis(10),
        max_retry_duration: Duration::from_millis(200), // Short duration for testing
        backoff_strategy: BackoffStrategy::Constant,
    };
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let cmd = make_duration_test_cmd(CommandCategory::Quick);
    let start = Instant::now();

    // Register a command
    core.register_pending_ack(cmd_id(1), cmd, Priority::Normal, CameraId::CAMERA_1, start);

    // Simulate ACK timeout at t+60ms (ack_timeout=50ms + 10ms margin, within duration 200ms)
    let actions = core.check_timeouts(start + Duration::from_millis(60));

    // Should retry because we're within max_retry_duration (200ms)
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, SchedulerAction::RetryCommand { id, .. } if *id == cmd_id(1))),
        "Expected retry within max_retry_duration"
    );

    // Re-register for next retry simulation with the original start time
    let cmd = make_duration_test_cmd(CommandCategory::Quick);
    core.register_pending_ack(cmd_id(1), cmd, Priority::Normal, CameraId::CAMERA_1, start);

    // Simulate ACK timeout at t+250ms (exceeds duration of 200ms)
    let actions = core.check_timeouts(start + Duration::from_millis(250));

    // Should NOT retry because max_retry_duration (200ms) exceeded
    // Instead, should fail the command
    let has_retry = actions
        .iter()
        .any(|a| matches!(a, SchedulerAction::RetryCommand { id, .. } if *id == cmd_id(1)));
    let has_timeout_no_retry = actions.iter().any(|a| {
        matches!(
            a,
            SchedulerAction::Timeout {
                id,
                will_retry: false,
                ..
            } if *id == cmd_id(1)
        )
    });
    let has_failed = actions
        .iter()
        .any(|a| matches!(a, SchedulerAction::CommandFailed { id, .. } if *id == cmd_id(1)));

    assert!(
        !has_retry || has_timeout_no_retry || has_failed,
        "Expected no retry or failure after max_retry_duration exceeded, got: {:?}",
        actions
    );
}

#[test]
fn test_max_retry_duration_inquiry_timeout() {
    // Test that inquiry timeout retries respect max_retry_duration
    // Use short timeouts for testing
    let timeout_config = TimeoutConfig::uniform(Duration::from_millis(50));
    let retry_config = RetryConfig {
        max_retries: 10,
        base_retry_delay: Duration::from_millis(10),
        max_retry_duration: Duration::from_millis(200),
        backoff_strategy: BackoffStrategy::Constant,
    };
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    // Use create_test_inquiry which creates an actual Inquiry-type command
    let inquiry = create_test_inquiry(CameraId::CAMERA_1);
    let start = Instant::now();

    // Start an inquiry
    core.start_inquiry(
        cmd_id(1),
        inquiry,
        Priority::Normal,
        CameraId::CAMERA_1,
        start,
    );

    // Check timeout at t+60ms (quick_timeout=50ms + 10ms margin, within duration 200ms)
    let actions = core.check_timeouts(start + Duration::from_millis(60));

    // Should have retry action within duration
    let has_retry = actions
        .iter()
        .any(|a| matches!(a, SchedulerAction::RetryCommand { id, .. } if *id == cmd_id(1)));
    assert!(
        has_retry,
        "Expected retry within max_retry_duration for inquiry"
    );
}

#[test]
fn test_max_retry_duration_queue_retry_for_command() {
    // Test that queue_retry_for_command respects max_retry_duration
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig {
        max_retries: 10,
        base_retry_delay: Duration::from_millis(10),
        max_retry_duration: Duration::from_millis(100),
        backoff_strategy: BackoffStrategy::Constant,
    };
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let cmd = make_duration_test_cmd(CommandCategory::Quick);
    let start = Instant::now();

    // Register a command
    core.register_pending_ack(cmd_id(1), cmd, Priority::Normal, CameraId::CAMERA_1, start);

    // Queue retry within duration - should succeed
    let action = core.queue_retry_for_command(cmd_id(1), start + Duration::from_millis(50));
    assert!(
        matches!(action, Some(SchedulerAction::RetryCommand { id, .. }) if id == cmd_id(1)),
        "Expected retry command within duration"
    );

    // Clear retry state for next test
    if let Some(state) = core.commands.get_mut(&cmd_id(1)) {
        state.attempt = 0;
    }

    // Queue retry after duration exceeded - should fail
    let action = core.queue_retry_for_command(cmd_id(1), start + Duration::from_millis(150));
    assert!(
        matches!(
            action,
            Some(SchedulerAction::CommandFailed {
                id,
                error: Error::Timeout
            }) if id == cmd_id(1)
        ),
        "Expected failure after duration exceeded"
    );
}

#[test]
fn test_max_retry_duration_should_retry_command() {
    // Test that should_retry_command respects max_retry_duration
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig {
        max_retries: 10,
        base_retry_delay: Duration::from_millis(10),
        max_retry_duration: Duration::from_millis(100),
        backoff_strategy: BackoffStrategy::Constant,
    };
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let cmd = make_duration_test_cmd(CommandCategory::Movement);
    let start = Instant::now();

    // Register a command
    core.register_pending_ack(cmd_id(1), cmd, Priority::Normal, CameraId::CAMERA_1, start);

    // Create a retryable error (0x41 = CommandNotExecutable, retryable for Movement)
    let error = ViscaError::from_byte(0x41);

    // Should retry within duration
    assert!(
        core.should_retry_command(cmd_id(1), &error, start + Duration::from_millis(50)),
        "Expected should_retry_command=true within duration"
    );

    // Should NOT retry after duration exceeded
    assert!(
        !core.should_retry_command(cmd_id(1), &error, start + Duration::from_millis(150)),
        "Expected should_retry_command=false after duration exceeded"
    );
}

#[test]
fn test_max_retry_duration_socket_timeout() {
    // Test that socket timeout retries respect max_retry_duration
    // Use short timeouts for testing
    let timeout_config = TimeoutConfig::uniform(Duration::from_millis(50));
    let retry_config = RetryConfig {
        max_retries: 10,
        base_retry_delay: Duration::from_millis(10),
        max_retry_duration: Duration::from_millis(200),
        backoff_strategy: BackoffStrategy::Constant,
    };
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let cmd = make_duration_test_cmd(CommandCategory::Quick);
    let start = Instant::now();

    // Register and assign to socket
    core.register_pending_ack(
        cmd_id(1),
        cmd.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        start,
    );

    // Simulate ACK received, assign to socket
    core.handle_ack_with_id(Some(ViscaSocket::S1), Some(cmd_id(1)), start);

    // Check socket timeout at t+60ms (quick_timeout=50ms + 10ms margin, within duration 200ms)
    let actions = core.check_timeouts(start + Duration::from_millis(60));

    // Should have retry action
    let has_retry = actions
        .iter()
        .any(|a| matches!(a, SchedulerAction::RetryCommand { id, .. } if *id == cmd_id(1)));
    assert!(
        has_retry,
        "Expected retry for socket timeout within duration"
    );
}

#[test]
fn test_max_retry_duration_preserved_across_retries() {
    // Test that submitted_at is preserved when metadata is re-inserted during retries
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig {
        max_retries: 10,
        base_retry_delay: Duration::from_millis(10),
        max_retry_duration: Duration::from_millis(500),
        backoff_strategy: BackoffStrategy::Constant,
    };
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let cmd = make_duration_test_cmd(CommandCategory::Quick);
    let start = Instant::now();

    // Register initial command
    core.register_pending_ack(
        cmd_id(1),
        cmd.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        start,
    );

    // Get the initial submitted_at
    let initial_submitted_at = core
        .commands
        .get(&cmd_id(1))
        .map(|state| state.submitted_at);
    assert!(initial_submitted_at.is_some());

    // Simulate ACK received and socket assignment
    core.handle_ack_with_id(
        Some(ViscaSocket::S1),
        Some(cmd_id(1)),
        start + Duration::from_millis(50),
    );

    // Verify submitted_at is preserved after socket assignment
    let after_ack_submitted_at = core
        .commands
        .get(&cmd_id(1))
        .map(|state| state.submitted_at);
    assert_eq!(
        initial_submitted_at, after_ack_submitted_at,
        "submitted_at should be preserved after ACK"
    );

    // Queue a retry
    core.queue_retry_for_command(cmd_id(1), start + Duration::from_millis(100));

    // The command should still be trackable with original submitted_at
    // Note: After retry, the command may be in retry_queue not commands
    // But if it's still in commands, the timestamp should match
    if let Some(state) = core.commands.get(&cmd_id(1)) {
        assert_eq!(
            state.submitted_at,
            initial_submitted_at.unwrap(),
            "submitted_at should be preserved across retries"
        );
    }
}

#[test]
fn test_max_retry_duration_with_high_retry_budget() {
    // Ensure that even with high retry budget, duration limit is enforced
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig {
        max_retries: 100, // Very high retry count
        base_retry_delay: Duration::from_millis(1),
        max_retry_duration: Duration::from_millis(50), // Very short duration
        backoff_strategy: BackoffStrategy::Constant,
    };
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let cmd = make_duration_test_cmd(CommandCategory::Quick);
    let start = Instant::now();

    core.register_pending_ack(cmd_id(1), cmd, Priority::Normal, CameraId::CAMERA_1, start);

    // Even with 100 max_retries, should fail after 50ms duration
    let action = core.queue_retry_for_command(cmd_id(1), start + Duration::from_millis(60));

    assert!(
        matches!(
            action,
            Some(SchedulerAction::CommandFailed {
                id,
                error: Error::Timeout
            }) if id == cmd_id(1)
        ),
        "Duration limit should override high retry budget"
    );
}

#[test]
fn test_retry_config_should_retry_method_parity() {
    // Verify that our implementation matches RetryConfig::should_retry semantics
    let config = RetryConfig {
        max_retries: 3,
        base_retry_delay: Duration::from_millis(100),
        max_retry_duration: Duration::from_millis(500),
        backoff_strategy: BackoffStrategy::Exponential,
    };

    let start = Instant::now();

    // Test RetryConfig::should_retry directly
    assert!(config.should_retry(0, start), "Attempt 0 should retry");
    assert!(config.should_retry(1, start), "Attempt 1 should retry");
    assert!(config.should_retry(2, start), "Attempt 2 should retry");
    assert!(
        !config.should_retry(3, start),
        "Attempt 3 should NOT retry (max reached)"
    );

    // Verify SchedulerCore enforces the same semantics
    let timeout_config = TimeoutConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, config);

    let cmd = make_duration_test_cmd(CommandCategory::Movement);
    core.register_pending_ack(cmd_id(1), cmd, Priority::Normal, CameraId::CAMERA_1, start);

    let error = ViscaError::from_byte(0x41); // Retryable for Movement

    // Note: SchedulerCore uses category-based budgets which differ from raw max_retries
    // Movement category uses base max_retries (3), so behavior should align
    assert!(
        core.should_retry_command(cmd_id(1), &error, start),
        "SchedulerCore should align with RetryConfig at start"
    );
}

#[test]
fn test_inquiry_spacing_blocks_too_fast_inquiries() {
    // Test that inquiries respect min_inquiry_spacing
    let mut core = SchedulerCore::new(TimeoutConfig::default());
    core.set_min_inquiry_spacing(Duration::from_millis(100));
    let now = Instant::now();

    let inquiry = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
        kind: CommandKind::Inquiry,
        category: CommandCategory::Quick,
        response_type: Some(InquiryKind::ZoomPosition),
    });

    // Queue 2 inquiries
    for id in 1..=2 {
        core.queue_command(PendingCommand {
            id: cmd_id(id),
            command: inquiry.clone(),
            priority: Priority::Normal,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
        });
    }

    // First inquiry should be sendable
    let inq1 = core.next_item_to_send(now);
    assert!(inq1.is_some(), "First inquiry should be sendable");
    assert_eq!(inq1.unwrap().id, cmd_id(1));
    core.start_inquiry(
        cmd_id(1),
        inquiry.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // Second inquiry should NOT be sendable immediately (spacing not satisfied)
    assert!(
        !core.can_send_inquiry(now),
        "Second inquiry should be blocked by spacing"
    );
    let inq2 = core.next_item_to_send(now);
    assert!(
        inq2.is_none(),
        "No inquiry should be returned when spacing not satisfied"
    );

    // After 50ms (less than spacing), still should not be sendable
    let too_soon = now + Duration::from_millis(50);
    assert!(
        !core.can_send_inquiry(too_soon),
        "Inquiry should still be blocked before spacing expires"
    );

    // After 100ms+ (spacing satisfied), should be sendable
    let after_spacing = now + Duration::from_millis(100);
    assert!(
        core.can_send_inquiry(after_spacing),
        "Inquiry should be sendable after spacing expires"
    );
    let inq2_delayed = core.next_item_to_send(after_spacing);
    assert!(
        inq2_delayed.is_some(),
        "Second inquiry should be returned after spacing"
    );
    assert_eq!(inq2_delayed.unwrap().id, cmd_id(2));
}

#[test]
fn test_inquiry_spacing_allows_commands_while_blocking() {
    // Test that commands can still be sent even when inquiry spacing blocks inquiries
    let mut core = SchedulerCore::new(TimeoutConfig::default());
    core.set_min_inquiry_spacing(Duration::from_millis(150));
    let now = Instant::now();

    let inquiry = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
        kind: CommandKind::Inquiry,
        category: CommandCategory::Quick,
        response_type: Some(InquiryKind::ZoomPosition),
    });

    let command = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]),
        kind: CommandKind::Command,
        category: CommandCategory::Movement,
        response_type: None,
    });

    // Send first inquiry
    core.queue_command(PendingCommand {
        id: cmd_id(1),
        command: inquiry.clone(),
        priority: Priority::Normal,
        camera_id: CameraId::CAMERA_1,
        submitted_at: now,
    });

    let _inq1 = core.next_item_to_send(now).unwrap();
    core.start_inquiry(
        cmd_id(1),
        inquiry.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // Queue another inquiry and a command
    core.queue_command(PendingCommand {
        id: cmd_id(2),
        command: inquiry.clone(),
        priority: Priority::Normal,
        camera_id: CameraId::CAMERA_1,
        submitted_at: now,
    });

    core.queue_command(PendingCommand {
        id: cmd_id(3),
        command: command.clone(),
        priority: Priority::Normal,
        camera_id: CameraId::CAMERA_1,
        submitted_at: now,
    });

    // Inquiry should be blocked, but command should be sendable
    let next = core.next_item_to_send(now);
    assert!(next.is_some(), "Command should be sendable");
    let cmd = next.unwrap();
    assert_eq!(
        cmd.id,
        cmd_id(3),
        "Command should be returned, not blocked inquiry"
    );
    assert_eq!(cmd.kind(), CommandKind::Command);
}

#[test]
fn test_inquiry_spacing_zero_means_no_delay() {
    // Test that spacing of zero (default) doesn't block inquiries
    let mut core = SchedulerCore::new(TimeoutConfig::default());
    // Default spacing is Duration::ZERO - no artificial delay
    let now = Instant::now();

    let inquiry = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x01, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
        kind: CommandKind::Inquiry,
        category: CommandCategory::Quick,
        response_type: Some(InquiryKind::ZoomPosition),
    });

    // Queue 3 inquiries
    for id in 1..=3 {
        core.queue_command(PendingCommand {
            id: cmd_id(id),
            command: inquiry.clone(),
            priority: Priority::Normal,
            camera_id: CameraId::CAMERA_1,
            submitted_at: now,
        });
    }

    // With default max_inquiries_inflight (8), all should be sendable immediately
    for expected_id in 1..=3 {
        let inq = core.next_item_to_send(now);
        assert!(
            inq.is_some(),
            "Inquiry {expected_id} should be sendable with zero spacing"
        );
        let inq = inq.unwrap();
        assert_eq!(inq.id, cmd_id(expected_id));
        core.start_inquiry(
            cmd_id(expected_id),
            inquiry.clone(),
            Priority::Normal,
            CameraId::CAMERA_1,
            now,
        );
    }
}

// ==================== Stale Retry Filtering Tests ====================
// These tests verify that get_ready_retries correctly filters out stale
// retries for commands that have already completed or failed.

#[test]
fn test_stale_retry_dropped_after_command_completion() {
    // Arrange: register a command, schedule a retry, then complete the command
    // before retry_at. Verify the retry is dropped.
    let mut core = SchedulerCore::with_retry_config(
        TimeoutConfig::default(),
        RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Constant,
        },
    );

    let cmd_id = cmd_id(1);
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    let now = Instant::now();

    // Register command
    core.register_pending_ack(
        cmd_id,
        command.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // Queue a retry
    let action = core.queue_retry_for_command(cmd_id, now);
    assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));

    // Verify retry is queued
    assert_eq!(core.retry_queue_depth(), 1);

    // Complete the command (simulating successful response before retry_at)
    core.complete_command(cmd_id);

    // Advance time past retry_at and try to get retries
    let retries = core.get_ready_retries(now + Duration::from_millis(200));

    // Assert: no retries should be returned since command is no longer active
    assert!(
        retries.is_empty(),
        "Stale retry should be dropped for completed command"
    );

    // The retry queue should now be empty (stale entry was popped and discarded)
    assert_eq!(core.retry_queue_depth(), 0);
}

#[test]
fn test_stale_retry_dropped_after_inquiry_completion() {
    // Same as above but for inquiries
    let mut core = SchedulerCore::with_retry_config(
        TimeoutConfig::default(),
        RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Constant,
        },
    );

    let cmd_id = cmd_id(1);
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR],
        Some(InquiryKind::ZoomPosition),
        CommandCategory::Quick,
        camera_id,
    );
    let now = Instant::now();

    // Start inquiry (this registers it in inquiries_inflight)
    core.start_inquiry(
        cmd_id,
        command.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // Queue a retry
    let action = core.queue_retry_for_command(cmd_id, now);
    assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));

    // Complete the inquiry
    core.complete_inquiry(cmd_id);

    // Advance time past retry_at and try to get retries
    let retries = core.get_ready_retries(now + Duration::from_millis(200));

    // Assert: no retries should be returned
    assert!(
        retries.is_empty(),
        "Stale retry should be dropped for completed inquiry"
    );
}

#[test]
fn test_stale_retry_dropped_after_command_failure() {
    // Arrange: schedule retry, then simulate a failure that removes metadata
    let mut core = SchedulerCore::with_retry_config(
        TimeoutConfig::default(),
        RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Constant,
        },
    );

    let cmd_id = cmd_id(1);
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    let now = Instant::now();

    // Register command
    core.register_pending_ack(
        cmd_id,
        command.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // Queue a retry
    let action = core.queue_retry_for_command(cmd_id, now);
    assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));

    // Cancel the command (simulating failure cleanup)
    core.cancel_command(cmd_id);

    // Note: cancel_command already removes the retry from the queue,
    // but if there were a race condition where a retry was added after
    // cancel started, it would be filtered by get_ready_retries.

    // Advance time and verify no retries
    let retries = core.get_ready_retries(now + Duration::from_millis(200));
    assert!(
        retries.is_empty(),
        "No retries should be returned after cancel"
    );
}

#[test]
fn test_superseded_retry_entries_are_ignored() {
    // Arrange: force two retries for the same cmd_id with different attempt values
    // This simulates overlapping timeout/error paths that could schedule multiple retries
    let mut core = SchedulerCore::with_retry_config(
        TimeoutConfig::default(),
        RetryConfig {
            max_retries: 5,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Constant,
        },
    );

    let cmd_id = cmd_id(1);
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    let now = Instant::now();

    // Register command
    core.register_pending_ack(
        cmd_id,
        command.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // Queue first retry (attempt 1)
    let action1 = core.queue_retry_for_command(cmd_id, now);
    assert!(matches!(
        action1,
        Some(SchedulerAction::RetryCommand { .. })
    ));

    // Simulate another overlapping path queueing a second retry
    // This increments retry_attempts to 2 and queues another retry
    let action2 = core.queue_retry_for_command(cmd_id, now);
    assert!(matches!(
        action2,
        Some(SchedulerAction::RetryCommand { .. })
    ));

    // Now retry_attempts[cmd_id] == 2, but we have two entries in queue:
    // - One with attempt=1 (stale)
    // - One with attempt=2 (current)

    // Advance time past both retry_at values
    let retries = core.get_ready_retries(now + Duration::from_millis(300));

    // Assert: only one retry should be returned (the one with attempt=2)
    assert_eq!(
        retries.len(),
        1,
        "Only the current attempt should be returned, stale entry should be dropped"
    );
    assert_eq!(
        retries[0].attempt, 2,
        "The returned retry should be attempt 2"
    );

    // Both entries should have been popped from the queue
    assert_eq!(core.retry_queue_depth(), 0);
}

#[test]
fn test_valid_retry_still_works() {
    // Sanity test: ensure valid retries are still returned properly
    let mut core = SchedulerCore::with_retry_config(
        TimeoutConfig::default(),
        RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Constant,
        },
    );

    let cmd_id = cmd_id(1);
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    let now = Instant::now();

    // Register command
    core.register_pending_ack(
        cmd_id,
        command.clone(),
        Priority::Normal,
        CameraId::CAMERA_1,
        now,
    );

    // Queue a retry
    let action = core.queue_retry_for_command(cmd_id, now);
    assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));

    // Command is still active (not completed/cancelled)
    // Advance time past retry_at
    let retries = core.get_ready_retries(now + Duration::from_millis(200));

    // Assert: retry should be returned
    assert_eq!(retries.len(), 1, "Valid retry should be returned");
    assert_eq!(retries[0].id, cmd_id);
    assert_eq!(retries[0].attempt, 1);
}

#[test]
fn test_multiple_commands_with_valid_and_stale_retries() {
    // Test with multiple commands: some completed, some still active
    let mut core = SchedulerCore::with_retry_config(
        TimeoutConfig::default(),
        RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Constant,
        },
    );

    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );
    let now = Instant::now();

    // Register 3 commands
    for i in 1..=3 {
        core.register_pending_ack(
            cmd_id(i),
            command.clone(),
            Priority::Normal,
            CameraId::CAMERA_1,
            now,
        );
        // Queue a retry for each
        let action = core.queue_retry_for_command(cmd_id(i), now);
        assert!(matches!(action, Some(SchedulerAction::RetryCommand { .. })));
    }

    // Complete command 1 and 3, leave 2 active
    core.complete_command(cmd_id(1));
    core.complete_command(cmd_id(3));

    // Advance time and get retries
    let retries = core.get_ready_retries(now + Duration::from_millis(200));

    // Only command 2's retry should be returned
    assert_eq!(
        retries.len(),
        1,
        "Only active command's retry should return"
    );
    assert_eq!(
        retries[0].id,
        cmd_id(2),
        "Command 2's retry should be returned"
    );
}

// =============================================================================
// Sequence Correlation Safety Tests (Issue #475)
//
// These tests verify that sequenced replies with unmatched sequences are
// ignored to prevent stale/duplicate UDP packets from completing or failing
// the wrong command.
// =============================================================================

#[test]
fn test_late_completion_after_socket_reuse_sequenced() {
    // Test: Late completion for a completed command (sequenced) must NOT
    // complete a new command that has reused the same socket.
    //
    // Scenario:
    // 1. Register command A, assign socket 1, register sequence seqA
    // 2. Complete A and ensure socket 1 becomes free + finish_sequence runs
    // 3. Start command B on socket 1 (simulate ACK)
    // 4. Inject a Completion event for socket 1 with sequence = Some(seqA) but cmd_id unresolved
    // Expected: B is not completed/freed; no actions emitted; ignored counter increments

    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;

    // Create two commands
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );

    // Step 1: Register command A
    core.register_pending_ack(cmd_id(1), command.clone(), Priority::Normal, camera_id, now);
    core.register_sequence(cmd_id(1), 100); // seqA = 100

    // ACK for command A
    let source_a = ReplySource::from_fields(Some(cmd_id(1)), Some(100), Some(ViscaSocket::S1));
    let ack_a = SchedulerEvent::Ack { source: source_a };
    core.process_event(ack_a, now);

    // Verify A is on socket 1
    let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
    assert!(!free);
    assert_eq!(socket_cmd_id, Some(cmd_id(1)));

    // Step 2: Complete command A
    let source_complete_a =
        ReplySource::from_fields(Some(cmd_id(1)), Some(100), Some(ViscaSocket::S1));
    let complete_a = SchedulerEvent::Completion {
        source: source_complete_a,
        response: Response::Completion {
            socket: Some(ViscaSocket::S1),
        },
    };
    let actions = core.process_event(complete_a, now);
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        SchedulerAction::CommandComplete { id, .. } => assert_eq!(*id, cmd_id(1)),
        _ => panic!("Expected CommandComplete for A"),
    }

    // Socket 1 is now free
    let (free, _, _) = core.socket_state(ViscaSocket::S1);
    assert!(free);

    // Step 3: Start command B on socket 1
    core.register_pending_ack(cmd_id(2), command, Priority::Normal, camera_id, now);
    core.register_sequence(cmd_id(2), 101); // seqB = 101

    // ACK for command B (gets socket 1)
    let source_b = ReplySource::from_fields(Some(cmd_id(2)), Some(101), Some(ViscaSocket::S1));
    let ack_b = SchedulerEvent::Ack { source: source_b };
    core.process_event(ack_b, now);

    // Verify B is on socket 1
    let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
    assert!(!free);
    assert_eq!(socket_cmd_id, Some(cmd_id(2)));

    // Record the counter before the stale event
    let counter_before = core.ignored_unmatched_sequenced_replies();

    // Step 4: Inject a late completion with sequence=100 (A's sequence)
    // The sequence won't resolve because A is already completed and finish_sequence was called
    // Sequence lookup failed (A is gone), so this is Sequenced with no cmd_id
    let late_source = ReplySource::Sequenced {
        sequence: 100,
        socket: Some(ViscaSocket::S1),
    };
    let late_complete = SchedulerEvent::Completion {
        source: late_source,
        response: Response::Completion {
            socket: Some(ViscaSocket::S1),
        },
    };
    let actions = core.process_event(late_complete, now);

    // Assert: NO actions should be emitted
    assert!(
        actions.is_empty(),
        "Late sequenced completion must not produce any actions"
    );

    // Assert: Counter should increment
    assert_eq!(
        core.ignored_unmatched_sequenced_replies(),
        counter_before + 1,
        "Ignored counter should increment for unmatched sequenced reply"
    );

    // Assert: B is still active on socket 1
    let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
    assert!(!free, "Socket 1 should still be occupied by B");
    assert_eq!(
        socket_cmd_id,
        Some(cmd_id(2)),
        "B should still own socket 1"
    );
    assert!(
        core.is_command_pending(cmd_id(2)),
        "B should still be pending"
    );
}

#[test]
fn test_late_error_without_socket_sequenced() {
    // Test: Late error with unmatched sequence (and socket=None) must NOT
    // attribute to the most-recent pending-ACK command.
    //
    // Scenario:
    // 1. Create two pending ACK commands
    // 2. Inject an Error event with sequence=Some(unknown) and socket=None
    // Expected: The scheduler does NOT attribute it to pending_ack.max_by_key(sent_time)

    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;

    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );

    // Register two pending-ACK commands
    core.register_pending_ack(cmd_id(1), command.clone(), Priority::Normal, camera_id, now);
    core.register_pending_ack(
        cmd_id(2),
        command,
        Priority::Normal,
        camera_id,
        now + Duration::from_millis(1), // Slightly later
    );

    // Both are pending
    assert!(core.is_command_pending(cmd_id(1)));
    assert!(core.is_command_pending(cmd_id(2)));

    let counter_before = core.ignored_unmatched_sequenced_replies();

    // Inject error with unknown sequence (simulates stale packet)
    // Sequence lookup failed, so this is Sequenced with no cmd_id
    let stale_source = ReplySource::Sequenced {
        sequence: 999,
        socket: None,
    };
    let stale_error = SchedulerEvent::Error {
        source: stale_source,
        code: 0x41,
    };
    let actions = core.process_event(stale_error, now);

    // Assert: NO actions (especially no CommandFailed)
    assert!(
        actions.is_empty(),
        "Stale sequenced error must not produce any actions"
    );

    // Assert: Counter incremented
    assert_eq!(
        core.ignored_unmatched_sequenced_replies(),
        counter_before + 1,
        "Ignored counter should increment"
    );

    // Assert: Both commands are still pending
    assert!(
        core.is_command_pending(cmd_id(1)),
        "Command 1 should still be pending"
    );
    assert!(
        core.is_command_pending(cmd_id(2)),
        "Command 2 should still be pending"
    );
}

#[test]
fn test_late_inquiry_reply_sequenced() {
    // Test: Late inquiry reply with unmatched sequence must NOT
    // pop from the FIFO order queue.

    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;

    let inquiry = Arc::new(EncodedCommand {
        payload: SmallVec::from_slice(&[0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR]),
        kind: CommandKind::Inquiry,
        category: CommandCategory::Quick,
        response_type: Some(InquiryKind::ZoomPosition),
    });

    // Start an inquiry (adds to FIFO)
    core.start_inquiry(cmd_id(1), inquiry, Priority::Normal, camera_id, now);

    // Verify inquiry is in queue
    assert_eq!(core.inquiries_order.len(), 1);
    assert!(core.inflight_inquiry_ids.contains(&cmd_id(1)));

    let counter_before = core.ignored_unmatched_sequenced_replies();

    // Inject a stale inquiry reply with unknown sequence
    // Sequence lookup failed, so this is Sequenced with no cmd_id
    let stale_source = ReplySource::Sequenced {
        sequence: 9999,
        socket: None,
    };
    let stale_reply = SchedulerEvent::InquiryReply {
        source: stale_source,
        response: Response::Inquiry(crate::command::InquiryData::ZoomPosition { position: 0x1234 }),
    };
    let actions = core.process_event(stale_reply, now);

    // Assert: NO actions
    assert!(
        actions.is_empty(),
        "Stale sequenced inquiry reply must not produce actions"
    );

    // Assert: Counter incremented
    assert_eq!(
        core.ignored_unmatched_sequenced_replies(),
        counter_before + 1,
        "Ignored counter should increment"
    );

    // Assert: The inquiry is still in the queue (FIFO not popped)
    assert_eq!(
        core.inquiries_order.len(),
        1,
        "FIFO queue should not be popped"
    );
    assert!(
        core.inflight_inquiry_ids.contains(&cmd_id(1)),
        "Inquiry should still be inflight"
    );
}

#[test]
fn test_unsequenced_completion_still_uses_socket_fallback() {
    // Test: Raw VISCA (no sequence) completions can still use socket fallback.
    // This ensures the change doesn't break raw VISCA behavior.

    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;

    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );

    // Register command
    core.register_pending_ack(cmd_id(1), command, Priority::Normal, camera_id, now);

    // ACK assigns socket (raw VISCA - no sequence)
    let source = ReplySource::from_fields(Some(cmd_id(1)), None, Some(ViscaSocket::S1));
    let ack = SchedulerEvent::Ack { source };
    core.process_event(ack, now);

    // Completion with no cmd_id but matching socket (raw VISCA)
    // Raw VISCA - heuristic fallback allowed
    let source = ReplySource::BySocket {
        socket: ViscaSocket::S1,
    };
    let complete = SchedulerEvent::Completion {
        source,
        response: Response::Completion {
            socket: Some(ViscaSocket::S1),
        },
    };
    let actions = core.process_event(complete, now);

    // Assert: Command should complete via socket fallback
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        SchedulerAction::CommandComplete { id, .. } => {
            assert_eq!(
                *id,
                cmd_id(1),
                "Raw VISCA should complete via socket fallback"
            );
        }
        _ => panic!("Expected CommandComplete for raw VISCA"),
    }
}

#[test]
fn test_unsequenced_error_still_uses_temporal_fallback() {
    // Test: Raw VISCA (no sequence) errors can still use temporal fallback.

    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;

    // Use Quick category so 0x41 is NOT retryable (only Movement/Preset are retried)
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Quick,
        camera_id,
    );

    // Register a pending command
    core.register_pending_ack(cmd_id(1), command, Priority::Normal, camera_id, now);

    // Error with no socket nibble (raw VISCA)
    // Raw VISCA - temporal fallback allowed
    let source = ReplySource::Unknown;
    let error = SchedulerEvent::Error { source, code: 0x41 };
    let actions = core.process_event(error, now);

    // Assert: Error should be attributed via temporal fallback
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        SchedulerAction::CommandFailed { id, .. } => {
            assert_eq!(
                *id,
                cmd_id(1),
                "Raw VISCA error should attribute via temporal fallback"
            );
        }
        _ => panic!("Expected CommandFailed for raw VISCA error"),
    }
}

#[test]
fn test_late_ack_with_unmatched_sequence() {
    // Test: Late ACK with unmatched sequence should be ignored.

    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;

    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );

    // Register and complete a command
    core.register_pending_ack(cmd_id(1), command.clone(), Priority::Normal, camera_id, now);
    core.register_sequence(cmd_id(1), 100);

    // ACK and complete the command
    let source = ReplySource::from_fields(Some(cmd_id(1)), Some(100), Some(ViscaSocket::S1));
    let ack = SchedulerEvent::Ack { source };
    core.process_event(ack, now);

    let source = ReplySource::from_fields(Some(cmd_id(1)), Some(100), Some(ViscaSocket::S1));
    let complete = SchedulerEvent::Completion {
        source,
        response: Response::Completion {
            socket: Some(ViscaSocket::S1),
        },
    };
    core.process_event(complete, now);

    // Start a new command
    core.register_pending_ack(cmd_id(2), command, Priority::Normal, camera_id, now);

    let counter_before = core.ignored_unmatched_sequenced_replies();

    // Late ACK for old sequence (sequence lookup failed)
    let late_source = ReplySource::Sequenced {
        sequence: 100,
        socket: Some(ViscaSocket::S1),
    };
    let late_ack = SchedulerEvent::Ack {
        source: late_source,
    };
    let actions = core.process_event(late_ack, now);

    // Assert: No actions, counter incremented
    assert!(actions.is_empty(), "Late sequenced ACK must be ignored");
    assert_eq!(
        core.ignored_unmatched_sequenced_replies(),
        counter_before + 1,
        "Counter should increment for ignored ACK"
    );

    // Command 2 should still be pending (not affected)
    assert!(
        core.is_command_pending(cmd_id(2)),
        "Command 2 should still be pending"
    );
}

#[test]
fn test_fail_after_receive_error_frees_socket() {
    // Test: fail_after_receive_error should free any socket allocated to the command.
    //
    // This ensures that receive-side errors (decode failures, protocol errors)
    // immediately release socket resources so subsequent commands can proceed.
    //
    // Scenario:
    // 1. Start a command and assign it a socket via ACK
    // 2. Call fail_after_receive_error for that command
    // 3. Verify the socket is freed

    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;

    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        None,
        CommandCategory::Movement,
        camera_id,
    );

    // Register command and assign socket via ACK
    core.register_pending_ack(cmd_id(1), command, Priority::Normal, camera_id, now);

    // Process ACK to assign socket
    let source = ReplySource::from_fields(Some(cmd_id(1)), None, Some(ViscaSocket::S1));
    let ack = SchedulerEvent::Ack { source };
    core.process_event(ack, now);

    // Verify socket is occupied
    let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
    assert!(
        !free,
        "S1 should be occupied before fail_after_receive_error"
    );
    assert_eq!(
        socket_cmd_id,
        Some(cmd_id(1)),
        "S1 should be assigned to command 1"
    );

    // Fail the command with a receive error (simulates decode failure)
    let error = Error::invalid_response_length(1, &[0x01, 0x02, 0x03]);
    let action = core.fail_after_receive_error(cmd_id(1), error);

    // Verify action is CommandFailed
    assert!(action.is_some(), "Should produce CommandFailed action");
    match action.unwrap() {
        SchedulerAction::CommandFailed { id, error } => {
            assert_eq!(id, cmd_id(1));
            assert!(
                matches!(error, Error::InvalidResponseLength { .. }),
                "Error should be InvalidResponseLength"
            );
        }
        other => panic!("Expected CommandFailed, got {:?}", other),
    }

    // Verify socket is now free
    let (free, socket_cmd_id, _) = core.socket_state(ViscaSocket::S1);
    assert!(free, "S1 should be free after fail_after_receive_error");
    assert_eq!(
        socket_cmd_id, None,
        "S1 should not be assigned to any command"
    );

    // Verify command is no longer pending
    assert!(
        !core.is_command_pending(cmd_id(1)),
        "Command should be removed from pending"
    );
}

// ============================================================================
// Cancel lifecycle tests (issue #487)
// ============================================================================

/// Test: Cancel requested before ACK results in SendCancel action on ACK.
///
/// Verifies that when a cancel is requested for a command before a socket
/// is assigned (awaiting ACK), the cancel is deferred and emitted as a
/// `SchedulerAction::SendCancel` when the ACK arrives.
#[test]
fn test_cancel_requested_before_ack_emits_send_cancel_on_ack() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR],
        None,
        CommandCategory::Quick,
        camera_id,
    );

    // Register a command as pending ACK
    core.register_pending_ack(cmd_id(1), command, Priority::Normal, camera_id, now);

    // Request cancel before ACK (no socket assigned yet)
    let result = core.request_cancel_by_id(cmd_id(1));

    // Should return None (cancel deferred, not immediate)
    assert!(
        result.is_none(),
        "request_cancel_by_id should return None for command awaiting ACK"
    );

    // Verify cancel_requested is set
    let state = core.commands.get(&cmd_id(1)).expect("Command should exist");
    assert!(
        state.cancel_requested,
        "cancel_requested flag should be set"
    );

    // Now process an ACK to assign a socket
    let actions = core.process_event(
        SchedulerEvent::Ack {
            source: ReplySource::BySocket {
                socket: ViscaSocket::S1,
            },
        },
        now,
    );

    // Should emit a SendCancel action
    assert_eq!(
        actions.len(),
        1,
        "Should emit exactly one action (SendCancel)"
    );
    match &actions[0] {
        SchedulerAction::SendCancel {
            camera_id: cam,
            socket,
        } => {
            assert_eq!(*cam, camera_id, "Camera ID should match");
            assert_eq!(*socket, ViscaSocket::S1, "Socket should be S1");
        }
        other => panic!("Expected SendCancel, got {:?}", other),
    }

    // Verify cancel_requested is cleared
    let state = core.commands.get(&cmd_id(1)).expect("Command should exist");
    assert!(
        !state.cancel_requested,
        "cancel_requested flag should be cleared after SendCancel emitted"
    );
}

/// Test: Cancel requested after socket assignment returns immediately.
///
/// Verifies that when a cancel is requested for a command that already has
/// a socket assigned, `request_cancel_by_id` returns the camera_id and socket
/// immediately so the caller can send the cancel command.
#[test]
fn test_cancel_requested_after_ack_returns_immediately() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let camera_id = CameraId::CAMERA_2;
    let command = create_test_command(
        vec![0x82, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR],
        None,
        CommandCategory::Quick,
        camera_id,
    );

    // Register and send command, then process ACK to assign socket
    core.register_pending_ack(cmd_id(2), command, Priority::Normal, camera_id, now);
    core.process_event(
        SchedulerEvent::Ack {
            source: ReplySource::BySocket {
                socket: ViscaSocket::S2,
            },
        },
        now,
    );

    // Request cancel after socket is assigned
    let result = core.request_cancel_by_id(cmd_id(2));

    // Should return Some with camera_id and socket
    assert!(
        result.is_some(),
        "request_cancel_by_id should return Some for command with socket"
    );
    let (cam, socket) = result.unwrap();
    assert_eq!(cam, camera_id, "Camera ID should match");
    assert_eq!(socket, ViscaSocket::S2, "Socket should be S2");

    // cancel_requested should NOT be set (cancel is immediate)
    let state = core.commands.get(&cmd_id(2)).expect("Command should exist");
    assert!(
        !state.cancel_requested,
        "cancel_requested should not be set for immediate cancel"
    );
}

/// Test: Cancel requested for inactive/unknown command is a no-op.
///
/// Verifies that requesting a cancel for a command that doesn't exist
/// (completed, timed out, or never existed) returns None and does not
/// retain any state.
#[test]
fn test_cancel_requested_for_inactive_command_is_noop() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    // Request cancel for non-existent command
    let result = core.request_cancel_by_id(cmd_id(999));

    assert!(
        result.is_none(),
        "request_cancel_by_id should return None for inactive command"
    );

    // Verify no state was created
    assert!(
        core.commands.is_empty(),
        "No commands should be created for inactive cancel"
    );
}

/// Test: Command removal clears pending cancel.
///
/// Verifies that when a command is cancelled or completed after having
/// `cancel_requested` set, the flag is properly cleaned up.
#[test]
fn test_cancel_command_clears_cancel_requested() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR],
        None,
        CommandCategory::Quick,
        camera_id,
    );

    // Register command and request cancel
    core.register_pending_ack(cmd_id(3), command, Priority::Normal, camera_id, now);
    let _ = core.request_cancel_by_id(cmd_id(3));

    // Verify cancel_requested is set
    assert!(
        core.commands.get(&cmd_id(3)).unwrap().cancel_requested,
        "cancel_requested should be set"
    );

    // Cancel the command (removes all state)
    core.cancel_command(cmd_id(3));

    // Verify command is fully removed
    assert!(
        !core.commands.contains_key(&cmd_id(3)),
        "Command should be removed after cancel_command"
    );
}

/// Test: Clearing all state clears pending cancels.
///
/// Verifies that `clear_all()` properly clears any pending cancels.
#[test]
fn test_clear_all_clears_cancel_requested() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR],
        None,
        CommandCategory::Quick,
        camera_id,
    );

    // Register command and request cancel
    core.register_pending_ack(cmd_id(4), command, Priority::Normal, camera_id, now);
    let _ = core.request_cancel_by_id(cmd_id(4));

    // Clear all state
    core.clear_all();

    // Verify all commands are gone
    assert!(core.commands.is_empty(), "All commands should be cleared");
}

/// Test: Multiple cancels for the same command only emit once.
///
/// Verifies that calling `request_cancel_by_id` multiple times for the
/// same command before ACK only results in one `SendCancel` action.
#[test]
fn test_multiple_cancel_requests_emit_once() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);

    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR],
        None,
        CommandCategory::Quick,
        camera_id,
    );

    // Register command
    core.register_pending_ack(cmd_id(5), command, Priority::Normal, camera_id, now);

    // Request cancel multiple times
    let _ = core.request_cancel_by_id(cmd_id(5));
    let _ = core.request_cancel_by_id(cmd_id(5));
    let _ = core.request_cancel_by_id(cmd_id(5));

    // Process ACK
    let actions = core.process_event(
        SchedulerEvent::Ack {
            source: ReplySource::BySocket {
                socket: ViscaSocket::S1,
            },
        },
        now,
    );

    // Should emit exactly one SendCancel
    assert_eq!(
        actions.len(),
        1,
        "Should emit exactly one SendCancel despite multiple requests"
    );
    assert!(
        matches!(actions[0], SchedulerAction::SendCancel { .. }),
        "Action should be SendCancel"
    );
}

// ============================================================================
// Issue #490: Fix inquiry retry state to honor RetryConfig budgets
// ============================================================================

#[test]
fn test_inquiry_retry_preserves_attempt_count() {
    // Test: start_inquiry on resend should NOT reset attempt count.
    // Configure max_retries = 1 (Quick budget = 1+2 = 3) to verify behavior.
    let mut core = SchedulerCore::with_retry_config(
        TimeoutConfig {
            quick_timeout: Duration::from_millis(100), // Short timeout for testing
            ..TimeoutConfig::default()
        },
        RetryConfig {
            max_retries: 1,
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_secs(30),
            backoff_strategy: BackoffStrategy::Constant,
        },
    );

    let camera_id = CameraId::CAMERA_1;
    let now = Instant::now();

    // Create an inquiry
    let inquiry = create_test_inquiry(camera_id);

    // Start initial inquiry
    core.start_inquiry(cmd_id(1), inquiry.clone(), Priority::Normal, camera_id, now);

    // Verify initial state
    {
        let state = core.commands.get(&cmd_id(1)).expect("should exist");
        assert_eq!(state.attempt, 0, "Initial attempt should be 0");
        assert_eq!(state.submitted_at, now, "Initial submitted_at should match");
    }

    // Simulate timeout triggering a retry via check_timeouts
    // Must exceed quick_timeout (100ms)
    let timeout_time = now + Duration::from_millis(150);
    let actions = core.check_timeouts(timeout_time);

    // Should get a retry action
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        SchedulerAction::RetryCommand { id, .. } => {
            assert_eq!(*id, cmd_id(1));
        }
        _ => panic!("Expected RetryCommand action, got: {:?}", actions[0]),
    }

    // Verify attempt was incremented to 1
    {
        let state = core.commands.get(&cmd_id(1)).expect("should exist");
        assert_eq!(state.attempt, 1, "Attempt should be incremented to 1");
    }

    // Now simulate resending the inquiry (what happens in the runtime loop)
    let resend_time = now + Duration::from_secs(3);
    core.start_inquiry(
        cmd_id(1),
        inquiry.clone(),
        Priority::Normal,
        camera_id,
        resend_time,
    );

    // CRITICAL: attempt should still be 1 (not reset to 0)
    {
        let state = core.commands.get(&cmd_id(1)).expect("should exist");
        assert_eq!(
            state.attempt, 1,
            "Attempt count should be PRESERVED (1), not reset to 0"
        );
        // submitted_at should be preserved from initial submission
        assert_eq!(
            state.submitted_at, now,
            "submitted_at should be preserved from initial submission"
        );
        // sent_at should be updated to resend time
        assert_eq!(
            state.sent_at,
            Some(resend_time),
            "sent_at should be updated"
        );
    }
}

#[test]
fn test_inquiry_retry_preserves_submitted_at() {
    // Test: start_inquiry on resend should NOT reset submitted_at timestamp.
    // This ensures max_retry_duration is measured from original submission.
    let mut core = SchedulerCore::with_retry_config(
        TimeoutConfig::default(),
        RetryConfig {
            max_retries: 5,
            base_retry_delay: Duration::from_millis(50),
            max_retry_duration: Duration::from_millis(500), // Short duration
            backoff_strategy: BackoffStrategy::Constant,
        },
    );

    let camera_id = CameraId::CAMERA_1;
    let start_time = Instant::now();

    let inquiry = create_test_inquiry(camera_id);

    // Start initial inquiry
    core.start_inquiry(
        cmd_id(1),
        inquiry.clone(),
        Priority::Normal,
        camera_id,
        start_time,
    );

    let original_submitted_at = core.commands.get(&cmd_id(1)).unwrap().submitted_at;

    // Simulate multiple resends at different times
    for i in 1..=3 {
        let resend_time = start_time + Duration::from_millis(100 * i);
        core.start_inquiry(
            cmd_id(1),
            inquiry.clone(),
            Priority::Normal,
            camera_id,
            resend_time,
        );

        let state = core.commands.get(&cmd_id(1)).expect("should exist");
        assert_eq!(
            state.submitted_at, original_submitted_at,
            "submitted_at should be preserved across resends (iteration {})",
            i
        );
        assert_eq!(
            state.sent_at,
            Some(resend_time),
            "sent_at should be updated on resend"
        );
    }
}

#[test]
fn test_inquiry_retry_preserves_transport_error_flag() {
    // Test: start_inquiry on resend should NOT reset transport_error flag.
    // When max retries exhausted, error should be TransportError (not Timeout).
    let mut core = SchedulerCore::with_retry_config(
        TimeoutConfig::default(),
        RetryConfig {
            max_retries: 1,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Constant,
        },
    );

    let camera_id = CameraId::CAMERA_1;
    let now = Instant::now();

    let inquiry = create_test_inquiry(camera_id);

    // Start initial inquiry
    core.start_inquiry(cmd_id(1), inquiry.clone(), Priority::Normal, camera_id, now);

    // Mark as transport error (e.g., network failure)
    core.mark_retry_as_transport_error(cmd_id(1));

    // Verify flag is set
    assert!(
        core.commands.get(&cmd_id(1)).unwrap().transport_error,
        "transport_error should be set"
    );

    // Simulate resend
    let resend_time = now + Duration::from_millis(200);
    core.start_inquiry(
        cmd_id(1),
        inquiry.clone(),
        Priority::Normal,
        camera_id,
        resend_time,
    );

    // CRITICAL: transport_error should still be true
    assert!(
        core.commands.get(&cmd_id(1)).unwrap().transport_error,
        "transport_error flag should be PRESERVED after resend"
    );
}

#[test]
fn test_inquiry_retries_terminate_after_max_retries() {
    // Test: Inquiry retries should terminate after max_retries with correct error.
    // max_retries: 0 => Quick budget = 0 + 2 = 2, so allow 2 retries before failure.
    // Timeline: attempt 0 (initial) -> timeout -> attempt 1 -> timeout -> attempt 2 -> timeout -> FAIL
    let mut core = SchedulerCore::with_retry_config(
        TimeoutConfig {
            quick_timeout: Duration::from_millis(100),
            ..TimeoutConfig::default()
        },
        RetryConfig {
            max_retries: 0, // Quick budget = 0 + 2 = 2
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_secs(30),
            backoff_strategy: BackoffStrategy::Constant,
        },
    );

    let camera_id = CameraId::CAMERA_1;
    let start = Instant::now();

    let inquiry = create_test_inquiry(camera_id);

    // Start initial inquiry (attempt = 0)
    core.start_inquiry(
        cmd_id(1),
        inquiry.clone(),
        Priority::Normal,
        camera_id,
        start,
    );

    // First timeout (attempt 0 -> 1)
    let t1 = start + Duration::from_millis(150);
    let actions1 = core.check_timeouts(t1);
    assert_eq!(actions1.len(), 1);
    assert!(
        matches!(actions1[0], SchedulerAction::RetryCommand { .. }),
        "First timeout should trigger retry"
    );
    assert_eq!(
        core.commands.get(&cmd_id(1)).unwrap().attempt,
        1,
        "Attempt should be 1"
    );

    // Simulate resend (runtime would call start_inquiry again)
    core.start_inquiry(cmd_id(1), inquiry.clone(), Priority::Normal, camera_id, t1);
    core.inflight_inquiry_ids.insert(cmd_id(1));

    // Second timeout (attempt 1 -> 2)
    let t2 = t1 + Duration::from_millis(150);
    let actions2 = core.check_timeouts(t2);
    assert_eq!(actions2.len(), 1);
    assert!(
        matches!(actions2[0], SchedulerAction::RetryCommand { .. }),
        "Second timeout should trigger retry"
    );
    assert_eq!(
        core.commands.get(&cmd_id(1)).unwrap().attempt,
        2,
        "Attempt should be 2"
    );

    // Simulate resend
    core.start_inquiry(cmd_id(1), inquiry.clone(), Priority::Normal, camera_id, t2);
    core.inflight_inquiry_ids.insert(cmd_id(1));

    // Third timeout (attempt 2 >= max_retries=2) -> should FAIL, not retry
    let t3 = t2 + Duration::from_millis(150);
    let actions3 = core.check_timeouts(t3);
    assert_eq!(actions3.len(), 1);
    match &actions3[0] {
        SchedulerAction::CommandFailed { id, error } => {
            assert_eq!(*id, cmd_id(1));
            assert!(
                matches!(error, Error::Timeout),
                "Should fail with Timeout error, got: {:?}",
                error
            );
        }
        _ => panic!("Expected CommandFailed, got: {:?}", actions3[0]),
    }

    // Command should be removed
    assert!(
        !core.commands.contains_key(&cmd_id(1)),
        "Command should be removed after terminal failure"
    );
}

#[test]
fn test_inquiry_retries_terminate_after_max_duration() {
    // Test: Inquiry retries should terminate after max_retry_duration.
    let mut core = SchedulerCore::with_retry_config(
        TimeoutConfig {
            quick_timeout: Duration::from_millis(100),
            ..TimeoutConfig::default()
        },
        RetryConfig {
            max_retries: 100, // High value to ensure duration limit is hit first
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_millis(300), // Short duration
            backoff_strategy: BackoffStrategy::Constant,
        },
    );

    let camera_id = CameraId::CAMERA_1;
    let start = Instant::now();

    let inquiry = create_test_inquiry(camera_id);

    // Start initial inquiry
    core.start_inquiry(
        cmd_id(1),
        inquiry.clone(),
        Priority::Normal,
        camera_id,
        start,
    );

    // First timeout - well within duration
    let t1 = start + Duration::from_millis(150);
    let actions1 = core.check_timeouts(t1);
    assert!(
        matches!(actions1[0], SchedulerAction::RetryCommand { .. }),
        "First timeout should retry"
    );

    // Resend
    core.start_inquiry(cmd_id(1), inquiry.clone(), Priority::Normal, camera_id, t1);
    core.inflight_inquiry_ids.insert(cmd_id(1));

    // Second timeout - past max_retry_duration (start + 350ms > 300ms limit)
    let t2 = start + Duration::from_millis(350);
    let actions2 = core.check_timeouts(t2);
    assert_eq!(actions2.len(), 1);
    match &actions2[0] {
        SchedulerAction::CommandFailed { id, error } => {
            assert_eq!(*id, cmd_id(1));
            assert!(
                matches!(error, Error::Timeout),
                "Should fail with Timeout due to duration exceeded"
            );
        }
        _ => panic!(
            "Expected CommandFailed due to duration exceeded, got: {:?}",
            actions2[0]
        ),
    }
}

#[test]
fn test_inquiry_transport_error_classification_after_retries() {
    // Test: When transport_error is set, terminal failure should be TransportError.
    // max_retries: 0 => Quick budget = 0 + 2 = 2
    let mut core = SchedulerCore::with_retry_config(
        TimeoutConfig {
            quick_timeout: Duration::from_millis(100),
            ..TimeoutConfig::default()
        },
        RetryConfig {
            max_retries: 0, // Quick budget = 0 + 2 = 2
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_secs(30),
            backoff_strategy: BackoffStrategy::Constant,
        },
    );

    let camera_id = CameraId::CAMERA_1;
    let start = Instant::now();

    let inquiry = create_test_inquiry(camera_id);

    // Start initial inquiry (attempt = 0)
    core.start_inquiry(
        cmd_id(1),
        inquiry.clone(),
        Priority::Normal,
        camera_id,
        start,
    );

    // First timeout (attempt 0 -> 1) triggers retry
    let t1 = start + Duration::from_millis(150);
    let actions1 = core.check_timeouts(t1);
    assert!(matches!(actions1[0], SchedulerAction::RetryCommand { .. }));

    // Mark as transport error BEFORE resend
    core.mark_retry_as_transport_error(cmd_id(1));

    // Resend
    core.start_inquiry(cmd_id(1), inquiry.clone(), Priority::Normal, camera_id, t1);
    core.inflight_inquiry_ids.insert(cmd_id(1));

    // Verify transport_error is still set after resend
    assert!(
        core.commands.get(&cmd_id(1)).unwrap().transport_error,
        "transport_error should be preserved after resend"
    );

    // Second timeout (attempt 1 -> 2)
    let t2 = t1 + Duration::from_millis(150);
    let actions2 = core.check_timeouts(t2);
    assert!(
        matches!(actions2[0], SchedulerAction::RetryCommand { .. }),
        "Second timeout should retry"
    );

    // Resend again (transport_error still set)
    core.start_inquiry(cmd_id(1), inquiry.clone(), Priority::Normal, camera_id, t2);
    core.inflight_inquiry_ids.insert(cmd_id(1));

    // Verify transport_error is STILL preserved after second resend
    assert!(
        core.commands.get(&cmd_id(1)).unwrap().transport_error,
        "transport_error should be preserved after second resend"
    );

    // Third timeout (attempt 2 >= Quick budget = 2) -> should FAIL with TransportError
    let t3 = t2 + Duration::from_millis(150);
    let actions3 = core.check_timeouts(t3);
    assert_eq!(actions3.len(), 1);
    match &actions3[0] {
        SchedulerAction::CommandFailed { id, error } => {
            assert_eq!(*id, cmd_id(1));
            match error {
                Error::TransportError(msg) => {
                    assert!(
                        msg.contains("Network error after max retries"),
                        "Expected transport error message, got: {}",
                        msg
                    );
                }
                _ => panic!("Expected TransportError, got: {:?}", error),
            }
        }
        _ => panic!(
            "Expected CommandFailed with TransportError, got: {:?}",
            actions3[0]
        ),
    }
}

#[test]
fn test_inquiries_order_no_duplicates_on_resend() {
    // Test: inquiries_order should not accumulate duplicates across resends.
    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let camera_id = CameraId::CAMERA_1;
    let now = Instant::now();

    let inquiry = create_test_inquiry(camera_id);

    // Start initial inquiry
    core.start_inquiry(cmd_id(1), inquiry.clone(), Priority::Normal, camera_id, now);
    assert_eq!(core.inquiries_order.len(), 1);

    // Simulate multiple resends
    for i in 1..=5 {
        let resend_time = now + Duration::from_millis(100 * i);
        core.start_inquiry(
            cmd_id(1),
            inquiry.clone(),
            Priority::Normal,
            camera_id,
            resend_time,
        );

        // Should still be exactly 1 entry
        assert_eq!(
            core.inquiries_order.len(),
            1,
            "inquiries_order should have at most 1 entry per id (iteration {})",
            i
        );
        assert_eq!(
            core.inquiries_order.back(),
            Some(&cmd_id(1)),
            "The single entry should be cmd_id(1)"
        );
    }
}

#[test]
fn test_inquiry_preserves_cancel_requested_flag() {
    // Test: start_inquiry on resend should NOT reset cancel_requested flag.
    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let camera_id = CameraId::CAMERA_1;
    let now = Instant::now();

    let inquiry = create_test_inquiry(camera_id);

    // Start initial inquiry
    core.start_inquiry(cmd_id(1), inquiry.clone(), Priority::Normal, camera_id, now);

    // Set cancel_requested (normally done by request_cancel_by_id)
    if let Some(state) = core.commands.get_mut(&cmd_id(1)) {
        state.cancel_requested = true;
    }

    // Verify flag is set
    assert!(
        core.commands.get(&cmd_id(1)).unwrap().cancel_requested,
        "cancel_requested should be set"
    );

    // Simulate resend
    let resend_time = now + Duration::from_millis(200);
    core.start_inquiry(
        cmd_id(1),
        inquiry.clone(),
        Priority::Normal,
        camera_id,
        resend_time,
    );

    // CRITICAL: cancel_requested should still be true
    assert!(
        core.commands.get(&cmd_id(1)).unwrap().cancel_requested,
        "cancel_requested flag should be PRESERVED after resend"
    );
}

#[test]
fn test_new_inquiry_starts_fresh() {
    // Test: A genuinely new inquiry (not a resend) should start with fresh state.
    let mut core = SchedulerCore::new(TimeoutConfig::default());
    let camera_id = CameraId::CAMERA_1;
    let now = Instant::now();

    let inquiry = create_test_inquiry(camera_id);

    // Start a new inquiry
    core.start_inquiry(cmd_id(1), inquiry.clone(), Priority::Normal, camera_id, now);

    let state = core.commands.get(&cmd_id(1)).expect("should exist");
    assert_eq!(state.attempt, 0, "New inquiry should have attempt = 0");
    assert_eq!(
        state.submitted_at, now,
        "New inquiry should have submitted_at = now"
    );
    assert!(
        !state.transport_error,
        "New inquiry should have transport_error = false"
    );
    assert!(
        !state.cancel_requested,
        "New inquiry should have cancel_requested = false"
    );

    // Start a different new inquiry
    let later = now + Duration::from_millis(100);
    core.start_inquiry(
        cmd_id(2),
        inquiry.clone(),
        Priority::Normal,
        camera_id,
        later,
    );

    let state2 = core.commands.get(&cmd_id(2)).expect("should exist");
    assert_eq!(
        state2.attempt, 0,
        "Second new inquiry should have attempt = 0"
    );
    assert_eq!(
        state2.submitted_at, later,
        "Second new inquiry should have submitted_at = later"
    );
}

// ============================================================================
// Tests for helper methods introduced in #491 (TimeoutSource refactoring)
// ============================================================================

#[test]
fn test_should_retry_timeout_respects_budget() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x00, VISCA_TERMINATOR],
        None,
        CommandCategory::Quick,
        camera_id,
    );

    // Register a pending ACK command
    core.register_pending_ack(cmd_id(1), command.clone(), Priority::Normal, camera_id, now);

    // With attempt = 0 and default budget (Quick gets base + 2 = 5), should retry
    assert!(
        core.should_retry_timeout(cmd_id(1), now),
        "Command at attempt 0 should be retryable"
    );

    // Set attempt to max_retries
    if let Some(state) = core.commands.get_mut(&cmd_id(1)) {
        state.attempt = 10; // Exceeds budget
    }
    assert!(
        !core.should_retry_timeout(cmd_id(1), now),
        "Command at max retries should not be retryable"
    );
}

#[test]
fn test_should_retry_timeout_respects_duration() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig {
        max_retry_duration: Duration::from_secs(5),
        ..RetryConfig::default()
    };
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x00, VISCA_TERMINATOR],
        None,
        CommandCategory::Quick,
        camera_id,
    );

    // Register a pending ACK command
    core.register_pending_ack(cmd_id(1), command.clone(), Priority::Normal, camera_id, now);

    // Within duration, should be retryable
    let within_duration = now + Duration::from_secs(3);
    assert!(
        core.should_retry_timeout(cmd_id(1), within_duration),
        "Command within duration should be retryable"
    );

    // After max_retry_duration, should not be retryable
    let after_duration = now + Duration::from_secs(10);
    assert!(
        !core.should_retry_timeout(cmd_id(1), after_duration),
        "Command after max_retry_duration should not be retryable"
    );
}

#[test]
fn test_timeout_terminal_error_with_transport_flag() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x00, VISCA_TERMINATOR],
        None,
        CommandCategory::Quick,
        camera_id,
    );

    // Register a pending ACK command and set transport_error flag
    core.register_pending_ack(cmd_id(1), command.clone(), Priority::Normal, camera_id, now);

    if let Some(state) = core.commands.get_mut(&cmd_id(1)) {
        state.transport_error = true;
    }

    let error = core.timeout_terminal_error(cmd_id(1));
    assert!(
        matches!(error, Error::TransportError(_)),
        "Should return TransportError when transport_error flag is set"
    );
}

#[test]
fn test_timeout_terminal_error_without_transport_flag() {
    let timeout_config = TimeoutConfig::default();
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x00, VISCA_TERMINATOR],
        None,
        CommandCategory::Quick,
        camera_id,
    );

    // Register a pending ACK command without transport_error flag
    core.register_pending_ack(cmd_id(1), command.clone(), Priority::Normal, camera_id, now);

    let error = core.timeout_terminal_error(cmd_id(1));
    assert!(
        matches!(error, Error::Timeout),
        "Should return Timeout when transport_error flag is not set"
    );
}

#[test]
fn test_update_earliest_none_to_some() {
    let mut earliest: Option<Instant> = None;
    let now = Instant::now();

    SchedulerCore::update_earliest(&mut earliest, now);
    assert_eq!(earliest, Some(now), "Should update from None to Some");
}

#[test]
fn test_update_earliest_earlier_wins() {
    let now = Instant::now();
    let later = now + Duration::from_secs(10);
    let earlier = now;

    let mut earliest = Some(later);
    SchedulerCore::update_earliest(&mut earliest, earlier);
    assert_eq!(earliest, Some(earlier), "Earlier deadline should win");
}

#[test]
fn test_update_earliest_later_ignored() {
    let now = Instant::now();
    let later = now + Duration::from_secs(10);
    let earlier = now;

    let mut earliest = Some(earlier);
    SchedulerCore::update_earliest(&mut earliest, later);
    assert_eq!(earliest, Some(earlier), "Later deadline should be ignored");
}

#[test]
fn test_handle_timeout_socket_frees_socket() {
    let timeout_config = TimeoutConfig {
        quick_timeout: Duration::from_millis(100),
        ..Default::default()
    };
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x00, VISCA_TERMINATOR],
        None,
        CommandCategory::Quick,
        camera_id,
    );

    // Register a pending ACK command
    core.register_pending_ack(cmd_id(1), command.clone(), Priority::Normal, camera_id, now);

    // Verify socket is free before ACK
    let (socket_free_before, _, _) = core.socket_state(ViscaSocket::S1);
    assert!(socket_free_before, "Socket should be free before ACK");

    // Process ACK to assign socket
    let _ack_actions = core.process_event(
        SchedulerEvent::Ack {
            source: ReplySource::BySocket {
                socket: ViscaSocket::S1,
            },
        },
        now,
    );

    // Socket should now be busy
    let (socket_free_after_ack, socket_cmd, _) = core.socket_state(ViscaSocket::S1);
    assert!(
        !socket_free_after_ack,
        "Socket should be busy after ACK assignment"
    );
    assert_eq!(
        socket_cmd,
        Some(cmd_id(1)),
        "Socket should be assigned to command 1"
    );

    // Now trigger timeout via check_timeouts
    let later = now + Duration::from_millis(200);
    let timeout_actions = core.check_timeouts(later);

    // Should have retry action
    assert!(
        !timeout_actions.is_empty(),
        "Should have timeout actions (retry)"
    );

    // Socket should be freed
    let (socket_free_after_timeout, _, _) = core.socket_state(ViscaSocket::S1);
    assert!(
        socket_free_after_timeout,
        "Socket should be freed after timeout"
    );
}

#[test]
fn test_handle_timeout_inquiry_removes_from_tracking() {
    let timeout_config = TimeoutConfig {
        quick_timeout: Duration::from_millis(100),
        ..Default::default()
    };
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;
    let inquiry = create_test_command(
        vec![0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR],
        Some(InquiryKind::Power),
        CommandCategory::Quick,
        camera_id,
    );

    // Start an inquiry
    core.start_inquiry(cmd_id(1), inquiry.clone(), Priority::Normal, camera_id, now);

    assert!(
        core.inflight_inquiry_ids.contains(&cmd_id(1)),
        "Inquiry should be tracked"
    );

    // Trigger timeout
    let later = now + Duration::from_millis(200);
    let _actions = core.check_timeouts(later);

    // Inquiry should be removed from tracking (either retried or failed)
    assert!(
        !core.inflight_inquiry_ids.contains(&cmd_id(1)),
        "Inquiry should be removed from tracking after timeout"
    );
}

#[test]
fn test_handle_timeout_ack_emits_timeout_action() {
    let timeout_config = TimeoutConfig {
        ack_timeout: Duration::from_millis(100),
        ..Default::default()
    };
    let retry_config = RetryConfig::default();
    let mut core = SchedulerCore::with_retry_config(timeout_config, retry_config);
    let now = Instant::now();
    let camera_id = CameraId::CAMERA_1;
    let command = create_test_command(
        vec![0x81, 0x01, 0x00, VISCA_TERMINATOR],
        None,
        CommandCategory::Quick,
        camera_id,
    );

    // Register a pending ACK command
    core.register_pending_ack(cmd_id(1), command.clone(), Priority::Normal, camera_id, now);

    // Trigger ACK timeout
    let later = now + Duration::from_millis(200);
    let actions = core.check_timeouts(later);

    // Should have Timeout action for ACK
    let has_timeout_action = actions.iter().any(|a| {
        matches!(
            a,
            SchedulerAction::Timeout {
                kind: TimeoutKind::Ack,
                ..
            }
        )
    });
    assert!(
        has_timeout_action,
        "Should emit Timeout action for ACK timeout"
    );
}
