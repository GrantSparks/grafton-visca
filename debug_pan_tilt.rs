fn main() {
    // PTZOpticsG2 conversion: 14.4 units per degree
    let pan_deg = 50.0;
    let tilt_deg = 20.0;
    let pan_units = (pan_deg * 14.4) as i16;
    let tilt_units = (tilt_deg * 14.4) as i16;
    
    println\!("Pan: {} degrees = {} units = 0x{:04X}", pan_deg, pan_units, pan_units);
    println\!("Tilt: {} degrees = {} units = 0x{:04X}", tilt_deg, tilt_units, tilt_units);
    
    // Show VISCA encoding
    let pan_bytes = encode_visca_i16(pan_units);
    let tilt_bytes = encode_visca_i16(tilt_units);
    
    println\!("Pan VISCA bytes: {:02X} {:02X} {:02X} {:02X}", pan_bytes[0], pan_bytes[1], pan_bytes[2], pan_bytes[3]);
    println\!("Tilt VISCA bytes: {:02X} {:02X} {:02X} {:02X}", tilt_bytes[0], tilt_bytes[1], tilt_bytes[2], tilt_bytes[3]);
    
    // Expected command bytes from test
    let expected = vec\![0x05, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00];
    println\!("\nExpected pan bytes from test: {:02X} {:02X} {:02X} {:02X}", expected[0], expected[1], expected[2], expected[3]);
    println\!("Expected tilt bytes from test: {:02X} {:02X} {:02X} {:02X}", expected[4], expected[5], expected[6], expected[7]);
    
    // Decode expected values
    let expected_pan = decode_visca_i16(&expected[0..4]);
    let expected_tilt = decode_visca_i16(&expected[4..8]);
    println\!("\nExpected pan units: {} = {} degrees", expected_pan, expected_pan as f32 / 14.4);
    println\!("Expected tilt units: {} = {} degrees", expected_tilt, expected_tilt as f32 / 14.4);
}

fn encode_visca_i16(value: i16) -> [u8; 4] {
    let abs_val = value.abs() as u16;
    let sign = if value < 0 { 0x0F } else { 0x00 };
    
    [
        sign  < /dev/null |  ((abs_val >> 12) & 0x0F) as u8,
        ((abs_val >> 8) & 0x0F) as u8,
        ((abs_val >> 4) & 0x0F) as u8,
        (abs_val & 0x0F) as u8,
    ]
}

fn decode_visca_i16(bytes: &[u8]) -> i16 {
    let is_negative = (bytes[0] & 0xF0) == 0xF0;
    let abs_val = ((bytes[0] & 0x0F) as u16) << 12
                | (bytes[1] as u16) << 8  
                | (bytes[2] as u16) << 4
                | (bytes[3] as u16);
    
    if is_negative {
        -(abs_val as i16)
    } else {
        abs_val as i16
    }
}
