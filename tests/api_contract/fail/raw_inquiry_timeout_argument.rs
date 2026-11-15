use grafton_visca::{raw, ControlClass, InquiryRoute, ResponseDecoder, RetryClass, TimeoutClass};

fn main() {
    let _ = raw::Inquiry::new(
        [0x81, 0x09, 0x04, 0x00, 0xff],
        InquiryRoute::RAW,
        ResponseDecoder::from_fn(|_: &[u8]| Ok::<_, grafton_visca::Error>(0_u8)),
        TimeoutClass::Inquiry,
        RetryClass::Inquiry,
        ControlClass::Normal,
    );
}

//~ E0061
//~ "this function takes 5 arguments but 6 arguments were supplied"
