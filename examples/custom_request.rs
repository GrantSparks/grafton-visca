//! Downstream custom requests and a runtime profile.
//!
//! The typed extension surface classifies every request at the type level. This
//! example implements a custom `PlainCommand` and a custom `OperationCommand`,
//! encodes them with the same allocation-free `write_into` the built-ins use,
//! submits them through the generic `execute` / `submit` entry points, and builds
//! a runtime `ProfileSpec` value — the same immutable representation a
//! compile-time profile lowers to.
//!
//! The encoding and profile parts need no camera and always run. Pass an address
//! (or set `VISCA_CAMERA_ADDR`) to also submit the custom requests to a camera.
//! The wire payloads below are illustrative; replace them with your firmware's
//! exact vendor opcodes.

mod support;

use std::env;

use grafton_visca::{
    blocking::{Connect, Session},
    camera::profiles::{GenericVisca, PtzOpticsG2},
    completion::AppliedOnly,
    profile::ProfileSpec,
    request::{Operation as OperationClass, Plain as PlainClass},
    AffectedAxes, CameraId, ControlClass, Error, OperationCommand, PlainCommand, Request,
    RetryClass, SessionConfig, TimeoutClass,
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

/// A custom applied-only operation: a vendor actuation with no meaningful
/// physical rest state, so it exposes `applied` but never `settled`.
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
    // A runtime ProfileSpec value, ready to register on a SessionConfig.
    demonstrate_profile_spec()?;

    // Submit to a camera only when an address is supplied.
    if let Some(address) = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
    {
        let session = Connect::open_tcp::<PtzOpticsG2>(&address)?;
        let result = submit_custom(&session);
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

fn demonstrate_profile_spec() -> Result<(), Error> {
    // A validated runtime profile. Downstream code builds these with
    // `ProfileSpec::builder(Capabilities::runtime_baseline(..))`; deriving one
    // from a compile-time profile is the shortest way to obtain a valid value.
    let spec = ProfileSpec::from_compile_time::<GenericVisca>()?;
    let _config = SessionConfig::new(spec);
    println!("built a runtime ProfileSpec and registered it on a SessionConfig");
    Ok(())
}

fn submit_custom(session: &Session) -> Result<(), Error> {
    let camera = session.camera::<PtzOpticsG2>()?;
    // A plain command returns one final `Result`.
    camera.execute(&SetVendorTone(3))?;
    // An applied-only operation returns a handle you wait on with `applied`.
    camera.submit::<AppliedOnly, _>(&VendorNudge)?.applied()?;
    Ok(())
}
