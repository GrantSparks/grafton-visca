//! Validate VISCA inquiry support against a real camera.
//!
//! This script sends raw VISCA inquiry bytes and reports what the camera responds with.
//! Use this to validate whether specific inquiries are actually supported.
//!
//! Run with:
//! ```sh
//! cargo run --example validate_inquiries
//! ```

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

const CAMERA_IP: &str = "192.168.0.110:5678";

fn main() {
    println!("=== VISCA Inquiry Validation ===\n");
    println!("Connecting to camera at {CAMERA_IP}...");

    let mut stream = match TcpStream::connect(CAMERA_IP) {
        Ok(s) => {
            println!("Connected!\n");
            s
        }
        Err(e) => {
            eprintln!("Failed to connect: {e}");
            return;
        }
    };

    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();

    // Test inquiries - format: (name, bytes, expected_response_description)
    let inquiries = [
        // Known working inquiries (baseline)
        (
            "Power",
            vec![0x81, 0x09, 0x04, 0x00, 0xFF],
            "0x02=On, 0x03=Off",
        ),
        (
            "Zoom Position",
            vec![0x81, 0x09, 0x04, 0x47, 0xFF],
            "4 nibbles",
        ),
        (
            "Focus Position",
            vec![0x81, 0x09, 0x04, 0x48, 0xFF],
            "4 nibbles",
        ),
        (
            "Exposure Mode",
            vec![0x81, 0x09, 0x04, 0x39, 0xFF],
            "mode byte",
        ),
        // Phase 1 inquiries - NEED VALIDATION
        (
            "Sharpness Position (0x42)",
            vec![0x81, 0x09, 0x04, 0x42, 0xFF],
            "UNKNOWN - position?",
        ),
        (
            "Dynamic Range (0x25)",
            vec![0x81, 0x09, 0x04, 0x25, 0xFF],
            "UNKNOWN - level?",
        ),
        (
            "Auto Focus Sensitivity (0x58)",
            vec![0x81, 0x09, 0x04, 0x58, 0xFF],
            "UNKNOWN - mode?",
        ),
        // Phase 2 inquiries - NEED VALIDATION
        (
            "Auto Slow Shutter (0x5A)",
            vec![0x81, 0x09, 0x04, 0x5A, 0xFF],
            "UNKNOWN - on/off?",
        ),
        (
            "Digital Zoom / Menu (0x06)",
            vec![0x81, 0x09, 0x04, 0x06, 0xFF],
            "UNKNOWN - menu or dzoom?",
        ),
    ];

    println!("Sending {} inquiries...\n", inquiries.len());
    println!("{:-<80}", "");

    for (name, bytes, expected) in inquiries {
        print!("{name:<35} ");

        // Send inquiry
        if let Err(e) = stream.write_all(&bytes) {
            println!("SEND ERROR: {e}");
            continue;
        }

        // Small delay to let camera respond
        std::thread::sleep(Duration::from_millis(100));

        // Read response
        let mut buf = [0u8; 64];
        match stream.read(&mut buf) {
            Ok(0) => {
                println!("NO RESPONSE (connection closed)");
            }
            Ok(n) => {
                let response = &buf[..n];
                print!("Response: ");
                for b in response {
                    print!("{:02X} ", b);
                }

                // Interpret response
                if n >= 2 {
                    if response[0] == 0x90 && response[1] == 0x50 {
                        print!(" [DATA REPLY - SUPPORTED]");
                    } else if response[0] == 0x90 && response[1] == 0x60 {
                        let error_code = if n > 2 { response[2] } else { 0 };
                        print!(" [ERROR 0x{:02X}]", error_code);
                        match error_code {
                            0x02 => print!(" Syntax Error"),
                            0x03 => print!(" Command Buffer Full"),
                            0x04 => print!(" Command Cancelled"),
                            0x05 => print!(" No Socket"),
                            0x41 => print!(" Command Not Executable"),
                            _ => print!(" Unknown"),
                        }
                    } else if response[0] == 0x90 && (response[1] & 0xF0) == 0x40 {
                        print!(" [ACK - unexpected for inquiry]");
                    }
                }
                println!();

                // Show expected format
                if response[0] == 0x90 && response[1] == 0x50 {
                    println!("{:35} Expected: {expected}", "");
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                println!("TIMEOUT (no response within 2s)");
            }
            Err(e) => {
                println!("READ ERROR: {e}");
            }
        }

        println!("{:-<80}", "");

        // Small delay between inquiries
        std::thread::sleep(Duration::from_millis(200));
    }

    println!("\n=== Validation Complete ===");
    println!("\nLegend:");
    println!("  [DATA REPLY - SUPPORTED] = Camera responded with data (inquiry works)");
    println!("  [ERROR 0xNN] = Camera rejected the command");
    println!("  TIMEOUT = Camera didn't respond (inquiry likely not supported)");
}
