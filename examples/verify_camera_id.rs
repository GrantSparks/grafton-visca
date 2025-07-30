//! Verify camera ID implementation produces correct VISCA addresses

use grafton_visca::camera_id::CameraId;

fn main() {
    println!("Camera ID to VISCA Address Byte Verification:");
    println!("=============================================");

    // Test all valid camera IDs
    for id in 1..=8 {
        let camera_id = CameraId::new(id).unwrap();
        let address_byte = camera_id.to_address_byte();
        println!("CameraId({id}) -> 0x{address_byte:02X} (binary: 0b{address_byte:08b})");
    }

    println!("\nSpecific important cases:");
    println!("-------------------------");

    // The user's specific concern: Camera 1 should produce 0x81
    let camera1 = CameraId::CAMERA_1;
    let addr1 = camera1.to_address_byte();
    println!("CameraId::CAMERA_1.to_address_byte() = 0x{addr1:02X}");
    assert_eq!(addr1, 0x81, "Camera 1 should produce 0x81");

    // PTZOptics default
    println!("PTZOptics cameras use: 0x81 ✓");

    // Verify the formula: 0x80 | camera_id
    println!("\nFormula verification:");
    let result = 0x80 | 0x01;
    println!("0x80 | 0x01 = 0x{result:02X}");
    let matches = addr1 == 0x81;
    println!("This matches our Camera 1 address: {matches}");
}
