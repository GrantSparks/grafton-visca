#![cfg(all(
    feature = "test-utils",
    any(not(feature = "mode-async"), feature = "runtime-tokio")
))]

use grafton_visca::{
    command::{
        CommandBehavior, InquiryResponseSpec, Response, ResponseParser, ViscaCommand,
        VISCA_TERMINATOR,
    },
    timeout::CommandCategory,
    CameraId, Error,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VendorStatus {
    major: u8,
    minor: u8,
}

#[derive(Debug, Clone, Copy)]
struct VendorStatusInquiry;

impl ViscaCommand for VendorStatusInquiry {
    const MAX_SIZE: usize = 5;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        let bytes = [
            camera_id.to_address_byte(),
            0x09,
            0x7E,
            0x55,
            VISCA_TERMINATOR,
        ];
        buffer[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }

    fn behavior(&self) -> CommandBehavior {
        CommandBehavior::Inquiry(InquiryResponseSpec::Raw)
    }
}

impl ResponseParser for VendorStatusInquiry {
    type Response = VendorStatus;

    fn from_response(resp: Response) -> Result<Self::Response, Error> {
        match resp {
            Response::RawInquiry(payload) => match payload.as_slice() {
                [major, minor] => Ok(VendorStatus {
                    major: *major,
                    minor: *minor,
                }),
                _ => Err(Error::UnexpectedResponseType),
            },
            Response::Error(error) => Err(error),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

#[cfg(not(feature = "mode-async"))]
#[test]
fn blocking_send_command_typed_returns_raw_custom_payload() {
    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, CameraBuilder},
        mode::BlockingFutureExt,
        testing::testkit::{ScriptedBlockingTransport, Step},
    };

    let transport = ScriptedBlockingTransport::new(vec![Step::OnSend {
        matches: Some(vec![0x81, 0x09, 0x7E, 0x55, VISCA_TERMINATOR]),
        responses: vec![vec![0x90, 0x50, 0x12, 0x34, VISCA_TERMINATOR]],
    }]);
    let camera = CameraBuilder::new()
        .build_blocking::<PtzOpticsG2, _>(transport)
        .expect("blocking camera should build");

    let status = camera
        .send_command_typed(&VendorStatusInquiry)
        .block()
        .expect("raw custom inquiry should parse");

    assert_eq!(
        status,
        VendorStatus {
            major: 0x12,
            minor: 0x34
        }
    );
}

#[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
#[tokio::test]
async fn async_send_command_typed_returns_raw_custom_payload() {
    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, CameraBuilder},
        testing::testkit::{ScriptedTransport, Step},
        TokioExecutor, TokioRuntime,
    };

    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![Step::OnSend {
        matches: Some(vec![0x81, 0x09, 0x7E, 0x55, VISCA_TERMINATOR]),
        responses: vec![vec![0x90, 0x50, 0x56, 0x78, VISCA_TERMINATOR]],
    }]);
    let runtime = TokioRuntime::from_current().expect("tokio runtime should be available");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("async camera should build");

    let status = camera
        .send_command_typed(&VendorStatusInquiry)
        .await
        .expect("raw custom inquiry should parse");

    assert_eq!(
        status,
        VendorStatus {
            major: 0x56,
            minor: 0x78
        }
    );
}
