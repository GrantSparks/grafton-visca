//! Downstream custom requests and a runtime profile.
//!
//! The typed extension surface classifies every request at the type level. This
//! example implements a custom `PlainCommand` and a custom `OperationCommand`,
//! encodes them with the same allocation-free `write_into` the built-ins use,
//! submits them through the generic `execute` / `submit` entry points, and
//! builds and drives a custom runtime `ProfileSpec` through blocking
//! `camera_dyn` without implementing `CompileTimeProfile`.
//!
//! The encoding and profile parts need no camera and always run. Pass an address
//! (or set `VISCA_CAMERA_ADDR`) to also submit the custom requests to a camera.
//! The live TCP address must include its port, for example
//! `192.168.0.110:5678`. This example requires `--features dyn-api`.
//! The wire payloads below are illustrative; replace them with your firmware's
//! exact vendor opcodes.

mod support;

use std::{env, time::Duration};

use grafton_visca::{
    blocking::Session,
    capabilities::{Capabilities, CoordinateSystem},
    completion::AppliedOnly,
    dynapi::BlockingDynSessionCamera,
    profile::{
        PositionInquirySupport, ProfileEnvelope, ProfileSpec, ProfileTiming, TransportCompatibility,
    },
    request::{Operation as OperationClass, Plain as PlainClass},
    transport::Transport,
    AffectedAxes, CameraId, CommandTimeouts, ControlClass, Error, OperationCommand, PlainCommand,
    Request, RetryClass, SessionConfig, TimeoutClass,
};

use support::finish_session;

/// A custom plain command: a vendor configuration write. Plain commands complete
/// at applied and expose no settled or cancellation lifecycle.
struct SetVendorTone(u8);

impl Request for SetVendorTone {
    type Class = PlainClass;

    const MAX_SIZE: usize = 7;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
    const RETRY_CLASS: RetryClass = RetryClass::Never;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        // Illustrative vendor payload: 8x 01 04 7E 01 0p FF.
        let bytes = [
            camera_id.to_address_byte(),
            0x01,
            0x04,
            0x7E,
            0x01,
            self.0 & 0x0F,
            0xFF,
        ];
        if buffer.len() < bytes.len() {
            return Err(Error::BufferTooSmall {
                required: bytes.len(),
                actual: buffer.len(),
            });
        }
        buffer[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }
}

/// A custom applied-only operation: a vendor actuation with no meaningful target
/// state, so it exposes `applied` but never `settled`.
struct VendorNudge;

impl Request for VendorNudge {
    type Class = OperationClass<AppliedOnly>;

    const MAX_SIZE: usize = 6;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Movement;
    const RETRY_CLASS: RetryClass = RetryClass::Movement;
    const CONTROL_CLASS: ControlClass = ControlClass::User;

    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        let bytes = [camera_id.to_address_byte(), 0x01, 0x06, 0x7F, 0x03, 0xFF];
        if buffer.len() < bytes.len() {
            return Err(Error::BufferTooSmall {
                required: bytes.len(),
                actual: buffer.len(),
            });
        }
        buffer[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }
}

impl OperationCommand<AppliedOnly> for VendorNudge {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::PAN_TILT
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Type-level classification and allocation-free encoding: no camera needed.
    demonstrate_encoding()?;
    // A custom runtime ProfileSpec value, ready to register on a SessionConfig.
    let profile = runtime_profile_spec()?;
    println!("built a custom runtime ProfileSpec");

    // Submit to a camera only when an address is supplied.
    if let Some(address) = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
    {
        let transport = Transport::tcp().address(address).build_blocking()?;
        let session = Session::open(transport, SessionConfig::new(profile))?;
        let result = session
            .camera_dyn()
            .and_then(|camera| submit_custom(&camera));
        finish_session(result, session.close())?;
    } else {
        println!("no address given; skipped live submission");
    }
    Ok(())
}

fn demonstrate_encoding() -> Result<(), Error> {
    // These bounds prove the request class at compile time.
    fn assert_plain<T: PlainCommand>() {}
    fn assert_applied_only<T: OperationCommand<AppliedOnly>>() {}
    assert_plain::<SetVendorTone>();
    assert_applied_only::<VendorNudge>();

    let mut buffer = [0u8; SetVendorTone::MAX_SIZE];
    let len = SetVendorTone(3).write_into(CameraId::CAMERA_1, &mut buffer)?;
    println!("SetVendorTone encodes to {:02X?}", &buffer[..len]);

    let mut buffer = [0u8; VendorNudge::MAX_SIZE];
    let len = VendorNudge.write_into(CameraId::CAMERA_1, &mut buffer)?;
    println!("VendorNudge encodes to {:02X?}", &buffer[..len]);
    Ok(())
}

fn runtime_profile_spec() -> Result<ProfileSpec, Error> {
    let capabilities = Capabilities::runtime_baseline("Custom vendor camera", 1)?;
    let timing = ProfileTiming::builder()
        .ack_timeout(Duration::from_millis(100))
        .command_timeouts(CommandTimeouts::default())
        .inquiry_timeout(Duration::from_secs(1))
        .cancellation_timeout(Duration::from_secs(1))
        .ambiguity_timeout(Duration::from_secs(1))
        .busy_timeout(Duration::ZERO)
        .raw_inquiry_reply_skew(Duration::ZERO)
        .minimum_inquiry_spacing(Duration::ZERO)
        .minimum_command_spacing(Duration::ZERO)
        .build()?;

    ProfileSpec::builder(capabilities)
        .pan_tilt(
            -1_000..=1_000,
            -500..=500,
            8,
            8,
            10.0,
            10.0,
            CoordinateSystem::SignedCentered,
            true,
        )
        .transports(TransportCompatibility::new(Some(5678), None, false))
        .envelope(ProfileEnvelope::RawVisca)
        .timing(timing)
        .maximum_command_sockets(1)
        .supports_operation_complete(true)
        .supports_command_cancel(false)
        .preset_recall_axes(None)
        .position_inquiries(PositionInquirySupport::new(false, false, false))
        .build()
}

fn submit_custom(camera: &BlockingDynSessionCamera<'_>) -> Result<(), Error> {
    // A plain command returns one final `Result`.
    camera.execute(&SetVendorTone(3))?;
    // An applied-only operation returns a handle you wait on with `applied`.
    camera.submit::<AppliedOnly, _>(&VendorNudge)?.applied()?;
    Ok(())
}
