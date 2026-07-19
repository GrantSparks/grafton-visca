//! Lab tool for validating CAM_AE SET and CAM_AEModeInq against real cameras.
//!
//! This script sends raw VISCA bytes to test exposure mode inquiry and set
//! commands, validating that CAM_AEModeInq (81 09 04 39 FF) and CAM_AE SET
//! (81 01 04 39 0p FF) work correctly on PTZOptics cameras.
//!
//! **Warning:** This tool changes exposure mode on every camera in `CAMERAS`.
//! It attempts to restore each original mode and verifies the readback, but
//! interruption, transport failure, or camera failure can prevent restoration.
//! Review the hard-coded lab targets and pass `--apply` to acknowledge mutation.
//!
//! Run with:
//! ```sh
//! cargo run --example validate_ae_commands -- --apply
//! ```

use std::{
    error::Error,
    io::{self, Read, Write},
    net::TcpStream,
    time::Duration,
};

type ModeResults = Vec<(&'static str, bool)>;
type CameraResult = Result<(bool, ModeResults, bool), String>;

const VISCA_TERMINATOR: u8 = 0xFF;
const MAX_FRAME_SIZE: usize = 4096;

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

/// TCP is a byte stream, so preserve unread bytes when one read contains
/// several VISCA frames and keep reading when a frame is fragmented.
struct FramedViscaStream {
    stream: TcpStream,
    pending: Vec<u8>,
}

impl FramedViscaStream {
    fn connect(address: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(address)?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        stream.set_write_timeout(Some(Duration::from_secs(3)))?;

        Ok(Self {
            stream,
            pending: Vec::new(),
        })
    }

    fn send(&mut self, frame: &[u8]) -> io::Result<()> {
        self.stream.write_all(frame)
    }

    fn recv_frame(&mut self) -> io::Result<Vec<u8>> {
        loop {
            if let Some(end) = self
                .pending
                .iter()
                .position(|byte| *byte == VISCA_TERMINATOR)
            {
                return Ok(self.pending.drain(..=end).collect());
            }

            if self.pending.len() >= MAX_FRAME_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "VISCA response exceeded the lab tool's frame limit",
                ));
            }

            let mut chunk = [0_u8; 256];
            let read = self.stream.read(&mut chunk)?;
            if read == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "camera closed the connection before a complete VISCA frame",
                ));
            }

            self.pending.extend_from_slice(&chunk[..read]);
            if self.pending.len() > MAX_FRAME_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "VISCA response exceeded the lab tool's frame limit",
                ));
            }
        }
    }
}

fn connect_camera(address: &str) -> Result<FramedViscaStream, String> {
    FramedViscaStream::connect(address).map_err(|error| format!("connect/setup: {error}"))
}

/// Send bytes and read response, returning the raw response bytes.
fn send_and_receive(stream: &mut FramedViscaStream, bytes: &[u8]) -> Result<Vec<u8>, String> {
    stream.send(bytes).map_err(|e| format!("send: {e}"))?;
    stream.recv_frame().map_err(|error| match error.kind() {
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => {
            "inconclusive: no complete response before timeout".to_string()
        }
        _ => format!("read: {error}"),
    })
}

/// Format bytes as hex string.
fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse the single-byte CAM_AEModeInq response.
fn parse_exposure_mode_response(response: &[u8]) -> Result<u8, String> {
    if response.len() < 3 {
        return Err(format!("short response: {}", hex(response)));
    }
    if response.last() != Some(&VISCA_TERMINATOR) {
        return Err(format!("missing terminator: {}", hex(response)));
    }
    if response[0] != 0x90 {
        return Err(format!("unexpected source: {}", hex(response)));
    }
    if response[1] == 0x50 {
        let payload = &response[2..response.len() - 1];
        return match payload {
            [mode] => Ok(*mode),
            _ => Err(format!(
                "CAM_AEModeInq expected one data byte, received {}: {}",
                payload.len(),
                hex(response)
            )),
        };
    }
    if response[1] & 0xF0 == 0x60 {
        let payload = &response[2..response.len() - 1];
        let [code] = payload else {
            return Err(format!("malformed VISCA error: {}", hex(response)));
        };
        let desc = match *code {
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

#[derive(Debug, Clone, Copy)]
enum CommandReply {
    Ack(u8),
    Completion(u8),
    Error { socket: u8, code: u8 },
}

fn parse_command_reply(response: &[u8]) -> Result<CommandReply, String> {
    if response.last() != Some(&VISCA_TERMINATOR) {
        return Err(format!("missing terminator: {}", hex(response)));
    }
    if response.len() < 3 {
        return Err(format!("short command response: {}", hex(response)));
    }
    if response[0] != 0x90 {
        return Err(format!("unexpected response source: {}", hex(response)));
    }

    let socket = response[1] & 0x0F;
    match response[1] & 0xF0 {
        0x40 if response.len() == 3 => Ok(CommandReply::Ack(socket)),
        0x50 if response.len() == 3 => Ok(CommandReply::Completion(socket)),
        0x60 if response.len() == 4 => Ok(CommandReply::Error {
            socket,
            code: response[2],
        }),
        0x40 => Err(format!("malformed ACK: {}", hex(response))),
        0x50 => Err(format!("malformed completion: {}", hex(response))),
        0x60 => Err(format!("malformed VISCA error: {}", hex(response))),
        _ => Err(format!("unexpected command reply: {}", hex(response))),
    }
}

fn receive_command_reply(
    stream: &mut FramedViscaStream,
    stage: &str,
) -> Result<CommandReply, String> {
    let response = stream.recv_frame().map_err(|error| match error.kind() {
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => {
            format!("inconclusive: no complete {stage} before timeout")
        }
        _ => format!("read {stage}: {error}"),
    })?;
    parse_command_reply(&response)
}

/// Read the ACK + Completion flow. Coalesced frames remain buffered and are
/// returned by the second `recv_frame` call; fragmented frames are reassembled.
fn send_command_and_wait(stream: &mut FramedViscaStream, bytes: &[u8]) -> Result<(), String> {
    stream.send(bytes).map_err(|e| format!("send: {e}"))?;

    match receive_command_reply(stream, "ACK or completion")? {
        CommandReply::Completion(_) => Ok(()),
        CommandReply::Error { socket, code } => {
            Err(format!("camera error 0x{code:02X} on socket {socket}"))
        }
        CommandReply::Ack(ack_socket) => match receive_command_reply(stream, "completion")? {
            CommandReply::Completion(completion_socket)
                if completion_socket == ack_socket || completion_socket == 0 =>
            {
                Ok(())
            }
            CommandReply::Completion(completion_socket) => Err(format!(
                "completion socket {completion_socket} did not match ACK socket {ack_socket}"
            )),
            CommandReply::Error { socket, code } => Err(format!(
                "camera error 0x{code:02X} on socket {socket} after ACK on socket {ack_socket}"
            )),
            CommandReply::Ack(socket) => Err(format!("unexpected second ACK on socket {socket}")),
        },
    }
}

/// Test a single camera and return (inquiry_ok, set_results, restore_ok).
fn test_camera(name: &str, addr: &str) -> CameraResult {
    println!("\n  Connecting to {addr}...");
    let mut stream = connect_camera(addr)?;
    println!("  Connected to {name}");

    // Step 1: Query current exposure mode
    println!("  [1] Querying current exposure mode...");
    let inquiry_bytes = [0x81, 0x09, 0x04, 0x39, 0xFF];
    let response = send_and_receive(&mut stream, &inquiry_bytes)?;
    let original_mode = match parse_exposure_mode_response(&response) {
        Ok(mode) => {
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
    drop(stream);

    // Step 2: Test all 5 exposure modes
    println!("  [2] Testing exposure mode SET commands...");
    let mut set_results = Vec::new();

    for &(mode_name, mode_byte) in EXPOSURE_MODES {
        // Inter-command spacing for PTZOptics
        std::thread::sleep(Duration::from_millis(200));

        // Send CAM_AE SET
        let set_cmd = [0x81, 0x01, 0x04, 0x39, mode_byte, 0xFF];
        print!("      SET {mode_name:<20} (0x{mode_byte:02X}): ");

        // Isolate each mutation on a fresh connection. If one exchange times
        // out or is malformed, a late response cannot be mistaken for the next
        // command's ACK or inquiry reply.
        let mut stream = match connect_camera(addr) {
            Ok(stream) => stream,
            Err(error) => {
                println!("FAILED: {error}");
                set_results.push((mode_name, false));
                continue;
            }
        };

        match send_command_and_wait(&mut stream, &set_cmd) {
            Ok(()) => {
                // Verify by re-querying
                std::thread::sleep(Duration::from_millis(200));
                let verify = send_and_receive(&mut stream, &inquiry_bytes);
                match verify.and_then(|response| {
                    parse_exposure_mode_response(&response)
                        .map_err(|error| format!("verify: {error}"))
                }) {
                    Ok(readback) => {
                        if readback == mode_byte {
                            println!("OK (verified 0x{readback:02X})");
                            set_results.push((mode_name, true));
                        } else {
                            println!(
                                "SET succeeded but readback=0x{readback:02X} (expected 0x{mode_byte:02X})"
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
    let restore_ok = connect_camera(addr)
        .and_then(|mut restore_stream| {
            send_command_and_wait(&mut restore_stream, &restore_cmd)?;
            std::thread::sleep(Duration::from_millis(200));
            let response = send_and_receive(&mut restore_stream, &inquiry_bytes)?;
            let readback = parse_exposure_mode_response(&response)
                .map_err(|error| format!("restore verification: {error}"))?;
            if readback == original_mode {
                Ok(())
            } else {
                Err(format!(
                    "restore readback was 0x{readback:02X}, expected 0x{original_mode:02X}"
                ))
            }
        })
        .map_or_else(
            |error| {
                println!("      Restore FAILED: {error}");
                false
            },
            |()| {
                println!("      Restored and verified successfully");
                true
            },
        );

    Ok((true, set_results, restore_ok))
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    if arguments.next().as_deref() != Some("--apply") || arguments.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "this lab tool mutates every camera in CAMERAS; review the targets, then run `cargo run --example validate_ae_commands -- --apply`",
        )
        .into());
    }

    println!("=== CAM_AE SET / CAM_AEModeInq Validation ===");
    println!("WARNING: this mutates exposure mode on every configured lab camera.");
    println!("Original modes are restored and verified when execution reaches the restore step;");
    println!("interruption or transport/camera failure can still leave a camera changed.\n");
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

    let all_checks_passed = summary
        .iter()
        .all(|(_, inquiry_ok, set_results, restore_ok)| {
            *inquiry_ok
                && *restore_ok
                && set_results.len() == EXPOSURE_MODES.len()
                && set_results.iter().all(|(_, passed)| *passed)
        });
    if !all_checks_passed {
        return Err(io::Error::other(
            "one or more exposure checks failed; inspect the summary and verify every camera's restored state",
        )
        .into());
    }

    Ok(())
}
