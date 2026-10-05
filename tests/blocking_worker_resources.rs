//! Resource and latency measurements for the blocking owner worker (D24, #780).
//!
//! Each blocking session owns one native worker thread that reads the
//! transport in slices of at most 10 ms. These measurements, over UDP
//! loopback, record what that costs and what it buys: threads, idle CPU and
//! resident memory per session, and the latency from a STOP call to its bytes
//! at the camera while another thread waits on a running operation, and of
//! `close`.
//!
//! They depend on the host, so they are ignored by default. Run them with
//!
//! ```text
//! cargo test --release --features blocking --test blocking_worker_resources \
//!     -- --ignored --nocapture --test-threads=1
//! ```
//!
//! The assertions are loose bounds that catch a regression in kind (a busy
//! loop, a leaked thread, a STOP that waits for a read timeout); the printed
//! figures are the measurement.

#![cfg(all(feature = "blocking", target_os = "linux"))]
#![allow(clippy::expect_used, clippy::print_stderr)]

use std::{
    net::UdpSocket,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[path = "common/fake_camera.rs"]
mod fake_camera;

use fake_camera::{frames, ZOOM_STOP, ZOOM_TELE};
use grafton_visca::{
    blocking::{CameraSession, Connect},
    completion::AppliedOnly,
    profiles::PtzOpticsG2,
    request::builtin::{ZoomDrive, ZoomStop},
};

const IDLE_SESSIONS: usize = 8;
const IDLE_WINDOW: Duration = Duration::from_secs(2);
const STOP_SAMPLES: usize = 100;
/// Tells the scripted camera to stop serving.
const QUIT: &[u8] = &[0xff];

fn proc_status(field: &str) -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").expect("/proc/self/status");
    status
        .lines()
        .find_map(|line| line.strip_prefix(field))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|value| value.parse().ok())
        .expect("numeric status field")
}

fn threads() -> u64 {
    proc_status("Threads:")
}

fn rss_kib() -> u64 {
    proc_status("VmRSS:")
}

/// Process user plus system CPU time, in clock ticks of 10 ms.
fn cpu_ticks() -> u64 {
    let stat = std::fs::read_to_string("/proc/self/stat").expect("/proc/self/stat");
    // Fields after the parenthesised command name; utime and stime are the
    // 14th and 15th fields of the whole line.
    let after_name = stat.rsplit_once(") ").expect("stat command name").1;
    let fields: Vec<&str> = after_name.split_whitespace().collect();
    let utime: u64 = fields[11].parse().expect("utime");
    let stime: u64 = fields[12].parse().expect("stime");
    utime + stime
}

fn open(peer: &UdpSocket) -> CameraSession<PtzOpticsG2> {
    let address = peer.local_addr().expect("peer address").to_string();
    Connect::open_udp::<PtzOpticsG2>(address).expect("blocking UDP session")
}

fn percentile(sorted: &[Duration], percent: usize) -> Duration {
    sorted[(sorted.len() * percent / 100).min(sorted.len() - 1)]
}

#[test]
#[ignore = "host-dependent measurement; run with --ignored"]
fn idle_sessions_cost_one_thread_and_little_cpu() {
    let peer = UdpSocket::bind("127.0.0.1:0").expect("peer socket");
    let threads_before = threads();
    let rss_before = rss_kib();
    let sessions: Vec<_> = (0..IDLE_SESSIONS).map(|_| open(&peer)).collect();
    let threads_open = threads();
    let rss_open = rss_kib();

    let cpu_before = cpu_ticks();
    thread::sleep(IDLE_WINDOW);
    let cpu_ticks_idle = cpu_ticks() - cpu_before;

    let mut close_latencies = Vec::with_capacity(IDLE_SESSIONS);
    for session in sessions {
        let started = Instant::now();
        session.close().expect("close joins the worker");
        close_latencies.push(started.elapsed());
    }
    let threads_closed = threads();
    close_latencies.sort();

    let per_session_cpu =
        Duration::from_millis(cpu_ticks_idle * 10) / u32::try_from(IDLE_SESSIONS).expect("count");
    let cpu_percent = per_session_cpu.as_secs_f64() / IDLE_WINDOW.as_secs_f64() * 100.0;
    eprintln!(
        "threads: +{} for {IDLE_SESSIONS} sessions, +{} after close",
        threads_open - threads_before,
        threads_closed.saturating_sub(threads_before),
    );
    eprintln!(
        "idle CPU: {cpu_percent:.2}% of one core per session ({per_session_cpu:?} over {IDLE_WINDOW:?})"
    );
    eprintln!(
        "RSS: +{} KiB per session",
        rss_open.saturating_sub(rss_before) / IDLE_SESSIONS as u64
    );
    eprintln!(
        "close: max {:?}, median {:?}",
        close_latencies[close_latencies.len() - 1],
        percentile(&close_latencies, 50)
    );

    assert_eq!(threads_open - threads_before, IDLE_SESSIONS as u64);
    assert_eq!(threads_closed, threads_before, "close joins every worker");
    assert!(cpu_percent < 5.0, "an idle worker must not spin");
    assert!(
        close_latencies[close_latencies.len() - 1] < Duration::from_millis(100),
        "close waits at most one read slice, not the read timeout"
    );
}

/// A camera that holds a zoom drive executing on socket one and answers
/// every STOP on socket two, reporting when each STOP arrived.
fn camera(peer: UdpSocket, arrivals: mpsc::Sender<Instant>) {
    let mut buffer = [0_u8; 64];
    while let Ok((length, from)) = peer.recv_from(&mut buffer) {
        let arrived = Instant::now();
        let replies = match &buffer[..length] {
            ZOOM_TELE => vec![frames::ack(1)],
            ZOOM_STOP => {
                if arrivals.send(arrived).is_err() {
                    return;
                }
                vec![frames::ack(2), frames::complete(2)]
            }
            QUIT => return,
            _ => Vec::new(),
        };
        for reply in replies {
            peer.send_to(&reply, from).expect("camera reply");
        }
    }
}

#[test]
#[ignore = "host-dependent measurement; run with --ignored"]
fn stop_reaches_the_camera_within_one_read_slice() {
    let peer = UdpSocket::bind("127.0.0.1:0").expect("peer socket");
    let peer_address = peer.local_addr().expect("peer address");
    let session = open(&peer);
    let (arrivals_tx, arrivals) = mpsc::channel();
    let camera_thread = thread::spawn(move || camera(peer, arrivals_tx));
    let camera_view = session.camera().clone();

    // Another thread waits on a running zoom for the whole measurement.
    let mut zoom = camera_view
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .expect("zoom admitted");
    let waiter = thread::spawn(move || zoom.applied());

    let mut latencies = Vec::with_capacity(STOP_SAMPLES);
    for sample in 0..STOP_SAMPLES {
        // Wait out the profile's command spacing since the previous STOP, so
        // the sample measures the worker rather than pacing, and let the
        // worker settle into an idle read. The offset moves each call across
        // the read slice, which a fixed delay would sample at one phase.
        let offset = u64::try_from(sample * 7 % 10).expect("offset");
        thread::sleep(Duration::from_millis(150 + offset));
        let started = Instant::now();
        let mut stop = camera_view
            .submit::<AppliedOnly, _>(&ZoomStop)
            .expect("STOP admitted");
        let arrived = arrivals
            .recv_timeout(Duration::from_secs(5))
            .expect("STOP reached the camera");
        latencies.push(arrived.saturating_duration_since(started));
        stop.applied().expect("STOP applied");
    }

    session.close().expect("close");
    waiter
        .join()
        .expect("waiter thread")
        .expect_err("closing the session ends the zoom wait");
    UdpSocket::bind("127.0.0.1:0")
        .and_then(|socket| socket.send_to(QUIT, peer_address))
        .expect("stop the camera");
    camera_thread.join().expect("camera thread");

    latencies.sort();
    let p50 = percentile(&latencies, 50);
    let p99 = percentile(&latencies, 99);
    let max = latencies[latencies.len() - 1];
    eprintln!(
        "STOP call to camera ({STOP_SAMPLES} samples): p50 {p50:?}, p99 {p99:?}, max {max:?}"
    );
    assert!(
        p99 < Duration::from_millis(50),
        "a STOP waits at most one read slice"
    );
}
