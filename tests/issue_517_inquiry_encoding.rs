#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    sync::Mutex,
};

use grafton_visca::{
    command::{PowerInquiry, TallyGreenInquiry, ZoomPositionInquiry, VISCA_TERMINATOR},
    CameraId, Error, Request, ViscaInquiry,
};

struct CountingAllocator;

static ALLOCATION_TEST_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    static COUNTING_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    static ALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCATION_COUNT.try_with(|count| {
                count.set(count.get().saturating_add(1));
            });
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

struct AllocationCountingGuard;

impl Drop for AllocationCountingGuard {
    fn drop(&mut self) {
        let _ = COUNTING_ALLOCATIONS.try_with(|counting| counting.set(false));
    }
}

fn allocations_during(f: impl FnOnce()) -> usize {
    let _guard = ALLOCATION_TEST_LOCK.lock().unwrap();
    ALLOCATION_COUNT.with(|count| count.set(0));
    COUNTING_ALLOCATIONS.with(|counting| counting.set(true));

    {
        let _counting_guard = AllocationCountingGuard;
        f();
    }

    ALLOCATION_COUNT.with(Cell::get)
}

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
                actual: 4
            })
        ),
        "downstream inquiry should report exact buffer size, got {result:?}"
    );
}

#[test]
fn derived_inquiry_write_into_does_not_allocate() {
    let mut power_buffer = [0u8; PowerInquiry::MAX_SIZE];
    let power_allocations = allocations_during(|| {
        let len = PowerInquiry
            .write_into(CameraId::CAMERA_1, &mut power_buffer)
            .expect("power inquiry should encode");
        std::hint::black_box(len);
    });
    assert_eq!(power_allocations, 0, "PowerInquiry.write_into allocated");

    let mut tally_buffer = [0u8; TallyGreenInquiry::MAX_SIZE];
    let tally_allocations = allocations_during(|| {
        let len = TallyGreenInquiry
            .write_into(CameraId::CAMERA_1, &mut tally_buffer)
            .expect("tally inquiry should encode");
        std::hint::black_box(len);
    });
    assert_eq!(
        tally_allocations, 0,
        "TallyGreenInquiry.write_into allocated"
    );

    let mut downstream_buffer =
        [0u8; <DownstreamZoomPositionInquiry as grafton_visca::Request>::MAX_SIZE];
    let downstream_allocations = allocations_during(|| {
        let len = <DownstreamZoomPositionInquiry as grafton_visca::Request>::write_into(
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
    let mut buffer = [0u8; TallyGreenInquiry::MAX_SIZE];
    let len = TallyGreenInquiry
        .write_into(CameraId::CAMERA_3, &mut buffer)
        .expect("tally inquiry should encode");

    assert_eq!(TallyGreenInquiry::MAX_SIZE, 7);
    assert_eq!(
        &buffer[..len],
        &[0x83, 0x09, 0x7E, 0x04, 0x1A, 0x00, VISCA_TERMINATOR]
    );
}

#[test]
fn public_inquiry_still_encodes_canonical_standard_bytes() {
    let _guard = ALLOCATION_TEST_LOCK.lock().unwrap();
    let mut buffer = [0u8; ZoomPositionInquiry::MAX_SIZE];
    let len = ZoomPositionInquiry
        .write_into(CameraId::CAMERA_1, &mut buffer)
        .expect("zoom inquiry should encode");

    assert_eq!(&buffer[..len], &[0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR]);
}
