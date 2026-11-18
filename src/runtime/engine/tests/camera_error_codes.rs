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
        for max_retries in [0, 1] {
            for allowed in [false, true] {
                let mut command = command(1);
                let mut inquiry = inquiry(1);
                for request in [&mut command, &mut inquiry] {
                    request.context_mut().retry.max_retries = max_retries;
                    match request {
                        RuntimeRequest::Command { not_executable, .. } => *not_executable = allowed,
                        RuntimeRequest::Inquiry { syntax, .. } => *syntax = allowed,
                    }
                    let restricted_code = if request.is_inquiry() { 2 } else { 0x41 };
                    let expected = max_retries > 0
                        && (matches!(code, 3 | 5) || (code == restricted_code && allowed));
                    assert_eq!(
                        decoded.retry_permission(request),
                        expected,
                        "code {code:02x}, lane {:?}, count {max_retries}, lane permission {allowed}",
                        request.lane()
                    );
                }
            }
        }
    }
}
