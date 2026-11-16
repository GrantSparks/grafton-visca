#![allow(clippy::expect_used, clippy::unwrap_used)]

use grafton_visca::{
    request::Request, types::CameraId, wire::VISCA_TERMINATOR, Error, ViscaInquiry,
};

use crate::counting::{allocations_during, ALLOCATION_TEST_LOCK};

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x47, response = ZoomPosition)]
struct DownstreamZoomPositionInquiry;

#[test]
fn downstream_derive_uses_public_terminator_and_exact_buffer_size() {
    let _guard = ALLOCATION_TEST_LOCK.lock().unwrap();
    let inquiry = DownstreamZoomPositionInquiry;
    let mut buffer = [0u8; DownstreamZoomPositionInquiry::MAX_SIZE];

    let len = inquiry
        .write_into(CameraId::CAMERA_2, &mut buffer)
        .expect("downstream inquiry should encode");

    assert_eq!(DownstreamZoomPositionInquiry::MAX_SIZE, 5);
    assert_eq!(&buffer[..len], &[0x82, 0x09, 0x04, 0x47, VISCA_TERMINATOR]);

    let mut short_buffer = [0u8; 4];
    let result = inquiry.write_into(CameraId::CAMERA_2, &mut short_buffer);
    assert!(
        matches!(
            result,
            Err(Error::BufferTooSmall {
                required: 5,
                actual: 4,
                ..
            })
        ),
        "downstream inquiry should report exact buffer size, got {result:?}"
    );
}

#[test]
fn derived_inquiry_write_into_does_not_allocate() {
    let mut power_buffer = [0u8; grafton_visca::command::power::StatusInquiry::MAX_SIZE];
    let power_allocations = allocations_during(|| {
        let len = grafton_visca::command::power::StatusInquiry
            .write_into(CameraId::CAMERA_1, &mut power_buffer)
            .expect("power inquiry should encode");
        std::hint::black_box(len);
    });
    assert_eq!(
        power_allocations, 0,
        "grafton_visca::command::power::StatusInquiry.write_into allocated"
    );

    let mut tally_buffer = [0u8; grafton_visca::command::tally::GreenInquiry::MAX_SIZE];
    let tally_allocations = allocations_during(|| {
        let len = grafton_visca::command::tally::GreenInquiry
            .write_into(CameraId::CAMERA_1, &mut tally_buffer)
            .expect("tally inquiry should encode");
        std::hint::black_box(len);
    });
    assert_eq!(
        tally_allocations, 0,
        "grafton_visca::command::tally::GreenInquiry.write_into allocated"
    );

    let mut downstream_buffer =
        [0u8; <DownstreamZoomPositionInquiry as grafton_visca::request::Request>::MAX_SIZE];
    let downstream_allocations = allocations_during(|| {
        let len = <DownstreamZoomPositionInquiry as grafton_visca::request::Request>::write_into(
            &DownstreamZoomPositionInquiry,
            CameraId::CAMERA_1,
            &mut downstream_buffer,
        )
        .expect("downstream inquiry should encode");
        std::hint::black_box(len);
    });
    assert_eq!(
        downstream_allocations, 0,
        "downstream derived inquiry write_into allocated"
    );
}

#[test]
fn internal_extended_inquiry_remains_extended() {
    let _guard = ALLOCATION_TEST_LOCK.lock().unwrap();
    let mut buffer = [0u8; grafton_visca::command::tally::GreenInquiry::MAX_SIZE];
    let len = grafton_visca::command::tally::GreenInquiry
        .write_into(CameraId::CAMERA_3, &mut buffer)
        .expect("tally inquiry should encode");

    assert_eq!(grafton_visca::command::tally::GreenInquiry::MAX_SIZE, 6);
    assert_eq!(
        &buffer[..len],
        &[0x83, 0x09, 0x7E, 0x04, 0x1A, VISCA_TERMINATOR]
    );

    let mut red = [0u8; grafton_visca::command::tally::RedInquiry::MAX_SIZE];
    let red_len = grafton_visca::command::tally::RedInquiry
        .write_into(CameraId::CAMERA_3, &mut red)
        .expect("red tally inquiry should encode");
    assert_eq!(grafton_visca::command::tally::RedInquiry::MAX_SIZE, 6);
    assert_eq!(
        &red[..red_len],
        &[0x83, 0x09, 0x7E, 0x01, 0x0A, VISCA_TERMINATOR]
    );

    let mut camera_1_green = [0u8; grafton_visca::command::tally::GreenInquiry::MAX_SIZE];
    let camera_1_green_len = grafton_visca::command::tally::GreenInquiry
        .write_into(CameraId::CAMERA_1, &mut camera_1_green)
        .expect("camera 1 green tally inquiry should encode");
    assert_eq!(
        &camera_1_green[..camera_1_green_len],
        &[0x81, 0x09, 0x7E, 0x04, 0x1A, VISCA_TERMINATOR]
    );

    let mut camera_1_red = [0u8; grafton_visca::command::tally::RedInquiry::MAX_SIZE];
    let camera_1_red_len = grafton_visca::command::tally::RedInquiry
        .write_into(CameraId::CAMERA_1, &mut camera_1_red)
        .expect("camera 1 red tally inquiry should encode");
    assert_eq!(
        &camera_1_red[..camera_1_red_len],
        &[0x81, 0x09, 0x7E, 0x01, 0x0A, VISCA_TERMINATOR]
    );
}

#[test]
fn public_inquiry_still_encodes_canonical_standard_bytes() {
    let _guard = ALLOCATION_TEST_LOCK.lock().unwrap();
    let mut buffer = [0u8; grafton_visca::command::zoom::PositionInquiry::MAX_SIZE];
    let len = grafton_visca::command::zoom::PositionInquiry
        .write_into(CameraId::CAMERA_1, &mut buffer)
        .expect("zoom inquiry should encode");

    assert_eq!(&buffer[..len], &[0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR]);
}
