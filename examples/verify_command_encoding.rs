//! Verify that multi-camera support works correctly with different camera IDs
//!
//! This example demonstrates the camera ID system that enables controlling
//! multiple cameras on the same network. Each camera has a unique address
//! that gets encoded into VISCA commands.

use grafton_visca::{CameraId, Error};

fn main() -> Result<(), Error> {
    println!("Camera ID Address Verification:");
    println!("===============================\n");

    // Test individual camera IDs
    let cameras = [
        ("Camera 1", CameraId::CAMERA_1, 0x81),
        ("Camera 2", CameraId::CAMERA_2, 0x82),
        ("Camera 3", CameraId::CAMERA_3, 0x83),
        ("Camera 4", CameraId::CAMERA_4, 0x84),
        ("Camera 5", CameraId::CAMERA_5, 0x85),
        ("Camera 6", CameraId::CAMERA_6, 0x86),
        ("Camera 7", CameraId::CAMERA_7, 0x87),
        ("Broadcast", CameraId::BROADCAST, 0x88),
    ];

    println!("Testing camera ID to VISCA address byte conversion:");
    for (name, camera_id, expected_addr) in cameras {
        let addr_byte = camera_id.to_address_byte();
        let id = camera_id.id();
        println!("  {name} (ID {id}): 0x{addr_byte:02X}");

        assert_eq!(
            addr_byte, expected_addr,
            "{name} should produce address byte 0x{expected_addr:02X}"
        );
    }

    println!("\n✅ All address conversions verified!");

    // Test ID properties
    println!("\nTesting camera ID properties:");
    assert!(!CameraId::CAMERA_1.is_broadcast());
    assert!(!CameraId::CAMERA_7.is_broadcast());
    assert!(CameraId::BROADCAST.is_broadcast());
    println!("  ✅ Individual cameras are not broadcast");
    println!("  ✅ Broadcast camera is correctly identified");

    // Test valid ID range
    println!("\nTesting ID validation:");
    for id in 1..=7 {
        assert!(CameraId::try_from(id).is_ok(), "ID {id} should be valid");
    }
    assert!(
        CameraId::try_from(8).is_ok(),
        "Broadcast ID 8 should be valid"
    );
    assert!(CameraId::try_from(0).is_err(), "ID 0 should be invalid");
    assert!(CameraId::try_from(9).is_err(), "ID 9 should be invalid");
    println!("  ✅ ID validation works correctly (1-7 for cameras, 8 for broadcast)");

    println!("\n🎯 Key Benefits:");
    println!("- Each camera gets a unique VISCA address (0x81-0x87)");
    println!("- Broadcast address (0x88) sends commands to all cameras");
    println!("- Type-safe: Invalid IDs are rejected at runtime");
    println!("- Multiple cameras can share the same network connection");

    Ok(())
}
