#![cfg(feature = "blocking")]
//! Targeted hardware experiments for the rc.3 bench findings (PTZOptics G2,
//! Raw VISCA over TCP). Ignored by default; each needs `VISCA_CAMERA_IP`.
//!
//! - `e1_manual_focus_halt` (`VISCA_HW_ALLOW_SETTINGS=1`): switch to manual
//!   focus, run the owner halt, restore auto focus. Discriminates whether the
//!   G2 `90 6y 41` focus-STOP rejection is caused by auto-focus mode.
//! - `e5_stale_stream_reply_binding` (`VISCA_HW_FAULT_FLAG=<path>`): an
//!   operator drops this host's outbound packets to the camera while one
//!   power inquiry times out and is retried into the stalled TCP stream, then
//!   lifts the fault. The test then reads white-balance mode repeatedly and
//!   compares it with a fresh session's ground truth. Inquiries only.

use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

use grafton_visca::blocking::{Camera, Connect};
use grafton_visca::camera::profiles::PtzOpticsG2;

struct Hw(Instant);

impl Hw {
    fn log(&self, line: impl std::fmt::Display) {
        eprintln!("HW|t={}|{line}", self.0.elapsed().as_millis());
    }
}

fn address(hw: &Hw) -> Option<String> {
    match std::env::var("VISCA_CAMERA_IP") {
        Ok(ip) => Some(format!("{ip}:5678")),
        Err(_) => {
            hw.log("SKIP VISCA_CAMERA_IP not set");
            None
        }
    }
}

/// Restores auto focus on drop, so a failed step never leaves manual focus.
struct AutoFocusGuard<'a> {
    camera: &'a Camera<PtzOpticsG2>,
    hw: &'a Hw,
}

impl Drop for AutoFocusGuard<'_> {
    fn drop(&mut self) {
        let restore = self.camera.focus().auto();
        self.hw.log(format_args!("restore.focus_auto={restore:?}"));
        let mode = self.camera.focus().mode();
        self.hw.log(format_args!("RESTORED focus_mode={mode:?}"));
    }
}

#[test]
#[ignore]
fn e1_manual_focus_halt() {
    let hw = Hw(Instant::now());
    let Some(address) = address(&hw) else { return };
    if std::env::var("VISCA_HW_ALLOW_SETTINGS").as_deref() != Ok("1") {
        hw.log("SKIP VISCA_HW_ALLOW_SETTINGS!=1");
        return;
    }
    let session = Connect::open_tcp::<PtzOpticsG2>(address).expect("open TCP session");
    let camera = session.camera();
    let original = camera.focus().mode();
    hw.log(format_args!("focus_mode.original={original:?}"));
    {
        let _guard = AutoFocusGuard { camera, hw: &hw };
        let manual = camera.focus().manual();
        hw.log(format_args!("focus_manual.applied={manual:?}"));
        hw.log(format_args!(
            "focus_mode.during={:?}",
            camera.focus().mode()
        ));
        let started = Instant::now();
        let report = camera.motion().stop_all_motion();
        hw.log(format_args!(
            "halt.elapsed_ms={} report={report:?}",
            started.elapsed().as_millis()
        ));
    }
    let restored_mode = camera.focus().mode();
    let close = session.close();
    hw.log(format_args!("session.close={close:?}"));
    assert_eq!(
        format!("{restored_mode:?}"),
        format!("{original:?}"),
        "focus mode must be restored"
    );
}

#[test]
#[ignore]
fn e5_stale_stream_reply_binding() {
    let hw = Hw(Instant::now());
    let Some(address) = address(&hw) else { return };
    let Some(flag) = std::env::var_os("VISCA_HW_FAULT_FLAG").map(PathBuf::from) else {
        hw.log("SKIP VISCA_HW_FAULT_FLAG not set");
        return;
    };
    assert!(!flag.exists(), "fault flag must not exist before the test");

    let truth = {
        let fresh = Connect::open_tcp::<PtzOpticsG2>(address.clone()).expect("open truth");
        let mode = fresh.camera().white_balance().mode();
        let _ = fresh.close();
        mode
    };
    hw.log(format_args!("truth.white_balance_mode={truth:?}"));

    let session = Connect::open_tcp::<PtzOpticsG2>(address.clone()).expect("open TCP session");
    let camera = session.camera();
    hw.log(format_args!(
        "before_fault.white_balance_mode={:?}",
        camera.white_balance().mode()
    ));
    hw.log("READY_FOR_FAULT");
    let deadline = Instant::now() + Duration::from_secs(60);
    while !flag.exists() {
        assert!(Instant::now() < deadline, "fault never applied");
        sleep(Duration::from_millis(50));
    }
    hw.log("fault.flag_seen");

    let started = Instant::now();
    let power = camera.power().state();
    hw.log(format_args!(
        "during_fault.power elapsed_ms={} result={power:?}",
        started.elapsed().as_millis()
    ));
    hw.log("POWER_INQUIRY_DONE");
    let deadline = Instant::now() + Duration::from_secs(60);
    while flag.exists() {
        assert!(Instant::now() < deadline, "fault never lifted");
        sleep(Duration::from_millis(50));
    }
    hw.log("fault.flag_removed");

    let mut wrong = 0;
    for index in 0..8 {
        let started = Instant::now();
        let mode = camera.white_balance().mode();
        let matches = format!("{mode:?}") == format!("{truth:?}");
        if !matches {
            wrong += 1;
        }
        hw.log(format_args!(
            "after_fault.white_balance_mode[{index}] elapsed_ms={} result={mode:?} matches_truth={matches}",
            started.elapsed().as_millis()
        ));
    }
    let close = session.close();
    hw.log(format_args!("session.close={close:?}"));
    hw.log(format_args!("SUMMARY wrong_values={wrong}"));
    assert_eq!(
        wrong, 0,
        "a stale reply was returned as a later inquiry's value"
    );
}
