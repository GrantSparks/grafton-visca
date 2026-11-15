use grafton_visca::{raw, ControlClass, ResponseDecoder, RetryClass, TimeoutClass};

fn main() {
    let _ = raw::Inquiry::new(
        [0x81, 0x09, 0x04, 0x00, 0xff],
        ResponseDecoder::from_fn(|_: &[u8]| Ok::<_, grafton_visca::Error>(0_u8)),
        TimeoutClass::Inquiry,
        RetryClass::Inquiry,
        ControlClass::Normal,
    );
}

//~ E0061
//~ "this function takes 4 arguments but 5 arguments were supplied"
