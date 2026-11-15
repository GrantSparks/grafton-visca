use grafton_visca::{raw, ControlClass, RetryClass};

fn main() {
    let inquiry = raw::Inquiry::from_fn(
        [0x81, 0x09, 0x04, 0x00, 0xff],
        |_: &[u8]| Ok::<_, grafton_visca::Error>(true),
        RetryClass::Inquiry,
        ControlClass::Normal,
    ).unwrap();
    let _ = inquiry.route();
}

//~ E0599
//~ "no method named `route`"
