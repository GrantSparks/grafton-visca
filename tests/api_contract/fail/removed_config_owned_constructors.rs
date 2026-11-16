//! Configurations are reusable data; facade types own all I/O constructors.
use grafton_visca::{config::CameraConfig, profiles::PtzOpticsG2};

fn main() {
    let config = CameraConfig::<PtzOpticsG2>::serial("/dev/ttyUSB0", 9600);
    let _ = config.open();
    let _ = config.open_async(());
    let _ = config.open_serial();
    let _ = config.open_serial_async(());
}

//~ E0599
//~ "no method named `open`"
//~ "no method named `open_async`"
//~ "no method named `open_serial`"
//~ "no method named `open_serial_async`"
