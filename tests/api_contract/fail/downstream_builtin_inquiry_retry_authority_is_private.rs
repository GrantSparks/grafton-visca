use grafton_visca::{
    request, CameraId, ControlClass, Inquiry, InquiryRoute, Request, ResponseDecoder, RetryClass,
    TimeoutClass,
};

struct DownstreamInquiry;

impl Request for DownstreamInquiry {
    type Class = request::Inquiry;

    const MAX_SIZE: usize = 3;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Inquiry;
    const RETRY_CLASS: RetryClass = RetryClass::Inquiry;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> grafton_visca::Result<usize> {
        buffer[..3].copy_from_slice(&[target.to_address_byte(), 0x09, 0xff]);
        Ok(3)
    }
}

impl Inquiry for DownstreamInquiry {
    type Response = Vec<u8>;

    fn route(&self) -> InquiryRoute {
        InquiryRoute::RAW
    }

    fn decoder(&self) -> ResponseDecoder<Self::Response> {
        ResponseDecoder::from_fn(|payload| Ok(payload.to_vec()))
    }

    fn builtin_inquiry_syntax_retry(
        &self,
        _authority: grafton_visca::requests::BuiltinInquiryAuthority,
    ) -> bool {
        true
    }
}

fn main() {}

// Downstream inquiries cannot name the crate-only authority that records
// generated built-in provenance, so they retain the default `false` policy.
//~ E0603
//~ "module `requests` is private"
