//! Continuous-drive timing and physical-rest sampling shared by the hardware
//! tests that drive pan/tilt or zoom continuously (`hardware_motion_test`,
//! `hardware_concurrent_test` and `hardware_wire_rows_test`).

use std::{fmt, thread::sleep, time::Duration};

use grafton_visca::Error;

use crate::{
    hardware::{observe, HwLog},
    hw,
};

/// How long a continuous drive runs before its STOP is sent; with the STOP's
/// own latency this keeps every drive within about 300 ms of wall time.
pub const DRIVE: Duration = Duration::from_millis(250);

/// Number of samples and spacing used for the physical-rest observation.
pub const REST_SAMPLES: usize = 5;
pub const REST_INTERVAL: Duration = Duration::from_millis(200);

/// Samples `read` `samples` times, `interval` apart, and reports whether every
/// sample succeeded and all samples are equal.
pub fn sample_rest<T, F>(
    hw: &impl HwLog,
    label: &str,
    samples: usize,
    interval: Duration,
    mut read: F,
) -> bool
where
    T: fmt::Debug + PartialEq,
    F: FnMut() -> Result<T, Error>,
{
    let mut values: Vec<T> = Vec::with_capacity(samples);
    let mut all_ok = true;
    for index in 0..samples {
        if index > 0 {
            sleep(interval);
        }
        let result = read();
        observe(hw, &format!("{label}.sample[{index}]"), &result);
        match result {
            Ok(value) => values.push(value),
            Err(_) => all_ok = false,
        }
    }
    let stable = all_ok && values.windows(2).all(|pair| pair[0] == pair[1]);
    hw!(hw, "{label}.rest_stable={stable}");
    stable
}
