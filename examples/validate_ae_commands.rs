//! Validate CAM_AE SET and CAM_AEModeInq against real cameras.
//!
//! This script sends raw VISCA bytes to test exposure mode inquiry and set
//! commands, validating that CAM_AEModeInq (81 09 04 39 FF) and CAM_AE SET
//! (81 01 04 39 0p FF) work correctly on PTZOptics cameras.
//!
//! Run with:
//! ```sh
//! cargo run --example validate_ae_commands
//! ```

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

type ModeResults = Vec<(&'static str, bool)>;
type CameraResult = Result<(bool, ModeResults, bool), String>;

/// Camera addresses to test.
const CAMERAS: &[(&str, &str)] = &[
    ("Camera 107", "192.168.0.107:5678"),
    ("Camera 108", "192.168.0.108:5678"),
    ("Camera 109", "192.168.0.109:5678"),
    ("Camera 110", "192.168.0.110:5678"),
    ("Camera 111", "192.168.0.111:5678"),
];

/// Exposure modes to test: (name, VISCA mode byte).
const EXPOSURE_MODES: &[(&str, u8)] = &[
    ("Auto", 0x00),
    ("Manual", 0x03),
    ("Shutter Priority", 0x0A),
    ("Iris Priority", 0x0B),
    ("Bright", 0x0D),
];

fn exposure_mode_name(byte: u8) -> &'static str {
    match byte {
        0x00 => "Auto",
        0x03 => "Manual",
        0x0A => "Shutter Priority",
        0x0B => "Iris Priority",
        0x0D => "Bright",
        _ => "Unknown",
    }
}

/// Send bytes and read response, returning the raw response bytes.
fn send_and_receive(stream: &mut TcpStream, bytes: &[u8]) -> Result<Vec<u8>, String> {
    stream.write_all(bytes).map_err(|e| format!("send: {e}"))?;

    // Wait for camera to respond
    std::thread::sleep(Duration::from_millis(150));

    let mut buf = [0u8; 64];
    match stream.read(&mut buf) {
        Ok(0) => Err("connection closed".to_string()),
        Ok(n) => Ok(buf[..n].to_vec()),
        Err(e)
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut =>
        {
            Err("timeout".to_string())
        }
        Err(e) => Err(format!("read: {e}")),
    }
}

/// Format bytes as hex string.
fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse a VISCA response. Returns Ok with the data payload for a successful
/// inquiry reply (90 50 ...), or Err with a description.
fn parse_inquiry_response(response: &[u8]) -> Result<Vec<u8>, String> {
    if response.len() < 3 {
        return Err(format!("short response: {}", hex(response)));
    }
    if response[0] != 0x90 {
        return Err(format!("unexpected source: {}", hex(response)));
    }
    if response[1] == 0x50 {
        // Data reply: 90 50 <data...> FF
        let end = response.len() - 1; // skip trailing FF
        Ok(response[2..end].to_vec())
    } else if response[1] == 0x60 {
        let code = response.get(2).copied().unwrap_or(0);
        let desc = match code {
            0x02 => "Syntax Error",
            0x03 => "Command Buffer Full",
            0x04 => "Command Cancelled",
            0x41 => "Command Not Executable",
            _ => "Unknown Error",
        };
        Err(format!("error 0x{code:02X} ({desc})"))
    } else {
        Err(format!("unexpected reply type: {}", hex(response)))
    }
}

/// Parse a VISCA command response. Reads ACK + Completion, handling the
/// two-message flow (90 4x FF for ACK, then 90 5x FF for completion).
fn send_command_and_wait(stream: &mut TcpStream, bytes: &[u8]) -> Result<(), String> {
    stream.write_all(bytes).map_err(|e| format!("send: {e}"))?;

    // Read ACK (90 4x FF)
    std::thread::sleep(Duration::from_millis(150));
    let mut buf = [0u8; 64];
    let n = stream
        .read(&mut buf)
        .map_err(|e| format!("read ACK: {e}"))?;
    let ack = &buf[..n];

    if n < 3 || ack[0] != 0x90 || (ack[1] & 0xF0) != 0x40 {
        // Might be a direct completion or error
        if n >= 3 && ack[0] == 0x90 && (ack[1] & 0xF0) == 0x50 {
            return Ok(()); // Direct completion
        }
        if n >= 3 && ack[0] == 0x90 && ack[1] == 0x60 {
            let code = ack.get(2).copied().unwrap_or(0);
            return Err(format!("error 0x{code:02X} (no ACK)"));
        }
        return Err(format!("unexpected ACK: {}", hex(ack)));
    }

    // Check if completion was bundled with ACK
    // Some cameras send ACK+Completion in the same TCP read
    if n >= 6 {
        let second = &ack[3..n];
        if second.len() >= 3 && second[0] == 0x90 && (second[1] & 0xF0) == 0x50 {
            return Ok(());
        }
    }

    // Read Completion (90 5x FF)
    std::thread::sleep(Duration::from_millis(150));
    let n = stream
        .read(&mut buf)
        .map_err(|e| format!("read completion: {e}"))?;
    let completion = &buf[..n];

    if n >= 3 && completion[0] == 0x90 && (completion[1] & 0xF0) == 0x50 {
        Ok(())
    } else if n >= 3 && completion[0] == 0x90 && completion[1] == 0x60 {
        let code = completion.get(2).copied().unwrap_or(0);
        Err(format!("error 0x{code:02X} on completion"))
    } else {
        Err(format!("unexpected completion: {}", hex(completion)))
    }
}

/// Test a single camera and return (inquiry_ok, set_results, restore_ok).
fn test_camera(name: &str, addr: &str) -> CameraResult {
    println!("\n  Connecting to {addr}...");
    let mut stream = TcpStream::connect(addr).map_err(|e| format!("connect: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    println!("  Connected to {name}");

    // Step 1: Query current exposure mode
    println!("  [1] Querying current exposure mode...");
    let inquiry_bytes = [0x81, 0x09, 0x04, 0x39, 0xFF];
    let response = send_and_receive(&mut stream, &inquiry_bytes)?;
    let original_mode = match parse_inquiry_response(&response) {
        Ok(data) => {
            let mode = data[0];
            println!(
                "      Current mode: 0x{mode:02X} ({})",
                exposure_mode_name(mode)
            );
            mode
        }
        Err(e) => {
            println!("      Inquiry FAILED: {e}");
            return Ok((false, vec![], false));
        }
    };

    // Step 2: Test all 5 exposure modes
    println!("  [2] Testing exposure mode SET commands...");
    let mut set_results = Vec::new();

    for &(mode_name, mode_byte) in EXPOSURE_MODES {
        // Inter-command spacing for PTZOptics
        std::thread::sleep(Duration::from_millis(200));

        // Send CAM_AE SET
        let set_cmd = [0x81, 0x01, 0x04, 0x39, mode_byte, 0xFF];
        print!("      SET {mode_name:<20} (0x{mode_byte:02X}): ");

        match send_command_and_wait(&mut stream, &set_cmd) {
            Ok(()) => {
                // Verify by re-querying
                std::thread::sleep(Duration::from_millis(200));
                let verify = send_and_receive(&mut stream, &inquiry_bytes);
                match verify
                    .and_then(|r| parse_inquiry_response(&r).map_err(|e| format!("verify: {e}")))
                {
                    Ok(data) => {
                        if data[0] == mode_byte {
                            println!("OK (verified 0x{:02X})", data[0]);
                            set_results.push((mode_name, true));
                        } else {
                            println!(
                                "SET succeeded but readback=0x{:02X} (expected 0x{mode_byte:02X})",
                                data[0]
                            );
                            set_results.push((mode_name, false));
                        }
                    }
                    Err(e) => {
                        println!("SET succeeded, verify failed: {e}");
                        set_results.push((mode_name, false));
                    }
                }
            }
            Err(e) => {
                println!("FAILED: {e}");
                set_results.push((mode_name, false));
            }
        }
    }

    // Step 3: Restore original mode
    std::thread::sleep(Duration::from_millis(200));
    println!(
        "  [3] Restoring original mode (0x{original_mode:02X} = {})...",
        exposure_mode_name(original_mode)
    );
    let restore_cmd = [0x81, 0x01, 0x04, 0x39, original_mode, 0xFF];
    let restore_ok = match send_command_and_wait(&mut stream, &restore_cmd) {
        Ok(()) => {
            println!("      Restored successfully");
            true
        }
        Err(e) => {
            println!("      Restore FAILED: {e}");
            false
        }
    };

    Ok((true, set_results, restore_ok))
}

fn main() {
    println!("=== CAM_AE SET / CAM_AEModeInq Validation ===");
    println!("Testing exposure mode inquiry and set commands on all cameras.\n");

    let mut summary: Vec<(String, bool, ModeResults, bool)> = Vec::new();

    for &(name, addr) in CAMERAS {
        println!("{:-<70}", "");
        println!("Testing {name} ({addr})");

        match test_camera(name, addr) {
            Ok((inquiry_ok, set_results, restore_ok)) => {
                summary.push((name.to_string(), inquiry_ok, set_results, restore_ok));
            }
            Err(e) => {
                println!("  SKIP: {e}");
                summary.push((name.to_string(), false, vec![], false));
            }
        }
    }

    // Print summary table
    println!("\n{:=<70}", "");
    println!("SUMMARY");
    println!("{:=<70}", "");
    println!(
        "{:<15} {:<10} {:<8} {:<8} {:<10} {:<8} {:<8} {:<10}",
        "Camera", "Inquiry", "Auto", "Manual", "Shutter", "Iris", "Bright", "Restored"
    );
    println!("{:-<70}", "");

    for (name, inquiry_ok, set_results, restore_ok) in &summary {
        let inq = if *inquiry_ok { "OK" } else { "FAIL" };
        let res = if *restore_ok { "OK" } else { "FAIL" };

        let mode_status = |mode_name: &str| -> &str {
            set_results
                .iter()
                .find(|(n, _)| *n == mode_name)
                .map(|(_, ok)| if *ok { "OK" } else { "FAIL" })
                .unwrap_or("-")
        };

        println!(
            "{:<15} {:<10} {:<8} {:<8} {:<10} {:<8} {:<8} {:<10}",
            name,
            inq,
            mode_status("Auto"),
            mode_status("Manual"),
            mode_status("Shutter Priority"),
            mode_status("Iris Priority"),
            mode_status("Bright"),
            res,
        );
    }

    println!("{:-<70}", "");
    println!("\n=== Validation Complete ===");
}
