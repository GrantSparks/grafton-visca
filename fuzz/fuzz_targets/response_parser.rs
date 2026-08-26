#![no_main]

use grafton_visca::{
    command::{parse_inquiry_payload, InquiryKind, Response},
    raw::Plain,
    ControlClass, RetryClass, TimeoutClass,
};
use libfuzzer_sys::fuzz_target;

// Exercises the public frame and response parsers with arbitrary bytes.
//
// The parsers must return an error or an opaque response for malformed data;
// they must never panic or require a live transport.
fuzz_target!(|bytes: &[u8]| {
    let _ = Plain::new(
        bytes,
        TimeoutClass::Quick,
        RetryClass::Never,
        ControlClass::Normal,
    );

    let _ = Response::parse(bytes);
    for inquiry in [
        InquiryKind::Power,
        InquiryKind::ZoomPosition,
        InquiryKind::PanTiltPosition,
    ] {
        let _ = Response::parse_with_type(bytes, &inquiry);
        let _ = parse_inquiry_payload(bytes, &inquiry);
    }
});
