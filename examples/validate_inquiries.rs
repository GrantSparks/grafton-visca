//! Lab tool for validating VISCA inquiry support against a real camera.
//!
//! This script sends raw VISCA inquiry bytes and reports what the camera responds with.
//! It deliberately bypasses the high-level camera API so protocol behavior can be
//! inspected directly. A timeout is inconclusive; it is not proof that an inquiry
//! is unsupported.
//!
//! Run with:
//! ```sh
//! cargo run --example validate_inquiries
//! ```

use std::{
    error::Error,
    io::{self, Read, Write},
    net::TcpStream,
    time::Duration,
};

const CAMERA_IP: &str = "192.168.0.110:5678";
const VISCA_TERMINATOR: u8 = 0xFF;
const MAX_FRAME_SIZE: usize = 4096;

/// TCP is a byte stream, so a read can contain a partial VISCA frame or several
/// coalesced frames. Keep unread bytes between calls and return exactly one
/// terminator-delimited frame at a time.
struct FramedViscaStream {
    stream: TcpStream,
    pending: Vec<u8>,
}

impl FramedViscaStream {
    fn connect(address: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(address)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;

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

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn visca_error_name(code: u8) -> &'static str {
    match code {
        0x02 => "Syntax Error",
        0x03 => "Command Buffer Full",
        0x04 => "Command Cancelled",
        0x05 => "No Socket",
        0x41 => "Command Not Executable",
        _ => "Unknown",
    }
}

fn report_response(response: &[u8], expected: &str) {
    print!("Response: {}", hex(response));

    if response.last() != Some(&VISCA_TERMINATOR) {
        println!(" [MALFORMED: missing terminator]");
        return;
    }
    if response.len() < 3 {
        println!(" [MALFORMED: response is too short]");
        return;
    }
    if response[0] != 0x90 {
        println!(" [MALFORMED: unexpected source 0x{:02X}]", response[0]);
        return;
    }

    if response[1] == 0x50 {
        let payload = &response[2..response.len() - 1];
        if payload.is_empty() {
            println!(" [MALFORMED: empty data reply]");
        } else {
            println!(" [DATA REPLY - SUPPORTED]");
            println!("{:35} Expected: {expected}", "");
        }
    } else if response[1] & 0xF0 == 0x60 {
        let payload = &response[2..response.len() - 1];
        match payload {
            [code] => println!(" [ERROR 0x{code:02X}] {}", visca_error_name(*code)),
            _ => println!(" [MALFORMED: VISCA error must contain one error code]"),
        }
    } else if response[1] & 0xF0 == 0x40 {
        println!(" [ACK - unexpected for inquiry]");
    } else if response[1] & 0xF0 == 0x50 {
        println!(" [COMPLETION - unexpected for inquiry]");
    } else {
        println!(" [MALFORMED: unknown response class]");
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== VISCA Inquiry Validation ===\n");
    println!("Lab target: {CAMERA_IP}");
    println!("Each inquiry uses a fresh TCP connection so an incomplete response cannot desynchronize later probes.\n");

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
        // Candidate imaging inquiries requiring bench validation.
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
        // Additional candidate inquiries requiring bench validation.
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

        // A fresh connection makes a timeout or malformed response local to
        // this probe instead of allowing a late frame to be mistaken for the
        // response to the next inquiry.
        let mut stream = FramedViscaStream::connect(CAMERA_IP)?;

        // Send inquiry
        if let Err(e) = stream.send(&bytes) {
            println!("SEND ERROR: {e}");
            continue;
        }

        match stream.recv_frame() {
            Ok(response) => report_response(&response, expected),
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                println!("INCONCLUSIVE (no complete response within 2s)");
            }
            Err(e) => {
                println!("INCONCLUSIVE (read error: {e})");
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
    println!(
        "  INCONCLUSIVE = No complete response; transport, timing, or support may be the cause"
    );

    Ok(())
}
