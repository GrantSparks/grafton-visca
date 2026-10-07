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
//!   lifts the fault. The test then reads white-balance mode until eight
//!   answers match a fresh session's ground truth (bounded at 60 s). While the
//!   faulted inquiry's reply is still owed, `Error::InquiryCorrelationLost` is
//!   the expected refusal; a wrong `Ok` value fails the test. Inquiries only.

use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

use grafton_visca::blocking::{Camera, Connect};
use grafton_visca::camera::profiles::PtzOpticsG2;
use grafton_visca::Error;

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

    // The #795 contract: while the faulted inquiry's reply is still owed, an
    // inquiry to this camera fails with `InquiryCorrelationLost` instead of
    // binding a stale reply; once the owed reply arrives (TCP retransmits it
    // after the fault is lifted) the same session answers correctly again.
    // A wrong `Ok` value is the defect; a correlation refusal is not.
    let recovery_deadline = Instant::now() + Duration::from_secs(60);
    let mut wrong_values = 0;
    let mut refusals = 0;
    let mut other_errors = 0;
    let mut correct_after_recovery = 0;
    let mut index = 0;
    while correct_after_recovery < 8 && Instant::now() < recovery_deadline {
        let started = Instant::now();
        let mode = camera.white_balance().mode();
        let outcome = match &mode {
            Ok(_) if format!("{mode:?}") == format!("{truth:?}") => {
                correct_after_recovery += 1;
                "correct"
            }
            Ok(_) => {
                wrong_values += 1;
                "WRONG_VALUE"
            }
            Err(Error::InquiryCorrelationLost { .. }) => {
                refusals += 1;
                "correlation_refused"
            }
            Err(_) => {
                other_errors += 1;
                "other_error"
            }
        };
        hw.log(format_args!(
            "after_fault.white_balance_mode[{index}] elapsed_ms={} result={mode:?} outcome={outcome}",
            started.elapsed().as_millis()
        ));
        index += 1;
        if outcome != "correct" {
            sleep(Duration::from_millis(250));
        }
    }
    let close = session.close();
    hw.log(format_args!("session.close={close:?}"));
    hw.log(format_args!(
        "SUMMARY wrong_values={wrong_values} correlation_refusals={refusals} \
         other_errors={other_errors} correct_after_recovery={correct_after_recovery}"
    ));
    assert_eq!(
        wrong_values, 0,
        "a stale reply was returned as a later inquiry's value"
    );
    assert_eq!(
        other_errors, 0,
        "only correlation refusals may precede recovery"
    );
    assert_eq!(
        correct_after_recovery, 8,
        "the session did not recover once the owed reply could arrive"
    );
}
