use grafton_visca::{
    request, request::Inquiry, request::Request, request::ResponseDecoder,
    types::CameraId,
};

struct DownstreamInquiry;

impl Request for DownstreamInquiry {
    type Class = request::InquiryClass;

    const MAX_SIZE: usize = 3;
    fn policy(&self) -> grafton_visca::request::InquiryPolicy {
        grafton_visca::request::InquiryPolicy::new(grafton_visca::request::InquiryRetryClass::Inquiry, grafton_visca::request::SubmissionClass::Normal)
    }

    fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> grafton_visca::Result<usize> {
        buffer[..3].copy_from_slice(&[(0x80 | target.value()), 0x09, 0xff]);
        Ok(3)
    }
}

impl Inquiry for DownstreamInquiry {
    type Response = Vec<u8>;

    fn decoder(&self) -> ResponseDecoder<Self::Response> {
        ResponseDecoder::from_fn(|payload| Ok(payload.to_vec()))
    }

    fn builtin_inquiry_syntax_retry(
        &self,
        _authority: grafton_visca::requests::BuiltinInquiryAuthority,
        //~^ E0603 "module `requests` is private"
    ) -> grafton_visca::requests::BuiltinInquiryProtocol {
        //~^ E0603 "module `requests` is private"
        unreachable!()
    }
}

fn main() {}

// Downstream inquiries cannot name the crate-only authority or override the
// built-in syntax replay and Sony envelope metadata.
