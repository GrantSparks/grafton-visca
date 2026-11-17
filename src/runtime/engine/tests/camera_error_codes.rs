use super::*;

#[test]
fn every_camera_code_preserves_routing_evidence_and_retry_permissions() {
    for code in 0..=u8::MAX {
        let decoded = CameraErrorCode::from(code);
        assert_eq!(decoded.answers_named_cancellation(), matches!(code, 4 | 5));
        assert_eq!(
            decoded.refuses_before_start(),
            matches!(code, 1 | 2 | 3 | 5 | 0x41)
        );
        assert_eq!(decoded.confirms_cancellation(), code == 4);
        // Named cancellation answers must be routed before command rejections.
        for named in [false, true] {
            let answers_cancel = named && decoded.answers_named_cancellation();
            let refuses = !answers_cancel && decoded.refuses_before_start();
            assert_eq!(answers_cancel, named && matches!(code, 4 | 5));
            assert_eq!(
                refuses,
                matches!(code, 1 | 2 | 3 | 0x41) || (!named && code == 5)
            );
        }
        for flags in 0..8 {
            let retry = RetryPolicy {
                buffer_full: flags & 1 != 0,
                movement_not_executable: flags & 2 != 0,
                builtin_inquiry_syntax: flags & 4 != 0,
                ..RetryPolicy::NEVER
            };
            for family in [
                SendingReply::Inquiry,
                SendingReply::Stop,
                SendingReply::AckThenCompletion,
                SendingReply::CompletionOnly,
                SendingReply::NoReply,
            ] {
                let actual = decoded.retry_permission(&retry, family);
                let expected = match code {
                    3 | 5 => flags & 1 != 0,
                    0x41 => flags & 2 != 0,
                    2 => family == SendingReply::Inquiry && flags & 4 != 0,
                    _ => false,
                };
                assert_eq!(
                    actual, expected,
                    "code {code:02x}, flags {flags}, {family:?}"
                );
            }
        }
    }
}
