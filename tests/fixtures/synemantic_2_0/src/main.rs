use std::{future::Future, marker::PhantomData};

use grafton_visca::{
    camera::{
        profiles::{GenericVisca, SonyBRC300},
        Connect,
    },
    command::PictureEffectMode,
    runtime::TokioRuntime,
    Camera, CameraSession, CompileTimeProfile, Error, ProfileSpec, Session, SessionConfig,
};

fn typed_camera_surface<P: CompileTimeProfile>() -> PhantomData<Camera<P>> {
    PhantomData
}

/// Synemantic constructs its own object-safe facade around the typed camera,
/// rather than enabling grafton-visca's optional dynamic projection. Keep the
/// actual owner-backed Tokio constructor Send: its future crosses the host's
/// task boundary before the camera is erased behind that facade.
fn assert_tokio_connector_future<F>(_future: F)
where
    F: Future<Output = Result<CameraSession<GenericVisca>, Error>> + Send + 'static,
{
}

fn assert_tokio_serial_connector_future<F>(_future: F)
where
    F: Future<Output = Result<Session, Error>> + Send + 'static,
{
}

fn tokio_connector_surface(runtime: TokioRuntime) {
    // An async constructor is lazy: this creates (but never polls) the
    // opaque future, so the compile contract cannot perform network I/O.
    assert_tokio_connector_future(Connect::open_tcp::<GenericVisca, _>(
        "127.0.0.1:5678",
        runtime.clone(),
    ));
    assert_tokio_connector_future(Connect::open_udp::<GenericVisca, _>(
        "127.0.0.1:52381",
        runtime,
    ));
}

fn tokio_serial_connector_surface(runtime: TokioRuntime) {
    // As above, this validates the serial constructor's future type only; no
    // device is opened unless a caller later polls the returned future.
    assert_tokio_serial_connector_future(Connect::open_serial::<SonyBRC300, _>(
        "/dev/ttyUSB0",
        9_600,
        runtime,
    ));
}

/// The BRC-300 is a migration-sensitive profile: its source-backed image
/// capability must expose the base noun before Synemantic can dispatch image
/// operations through its profile-specific adapter.
fn sony_brc300_image_surface(camera: &Camera<SonyBRC300>) {
    let image = camera.image();
    let _ = image.backlight();
    let _ = image.set_backlight(true);
}

fn main() {
    let profile = ProfileSpec::from_compile_time::<GenericVisca>()
        .expect("the built-in profile must lower to a runtime specification");
    let _json = serde_json::to_string(&profile).expect("profile serialization");
    let _schema = schemars::schema_for!(ProfileSpec);
    let _typescript_name =
        <grafton_visca::types::PanSpeed as ts_rs::TS>::name(&ts_rs::Config::default());

    let _session_config = SessionConfig::from_compile_time::<GenericVisca>()
        .expect("the built-in profile must be accepted by SessionConfig");
    let _: PhantomData<Session> = PhantomData;
    let _: PhantomData<Camera<GenericVisca>> = typed_camera_surface();
    let _ = tokio_connector_surface as fn(TokioRuntime);
    let _ = tokio_serial_connector_surface as fn(TokioRuntime);
    let _ = sony_brc300_image_surface as fn(&Camera<SonyBRC300>);
    let _ = PictureEffectMode::BlackAndWhite;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sony_brc300_declares_the_base_image_surface() {
        let profile = ProfileSpec::from_compile_time::<SonyBRC300>()
            .expect("Sony BRC-300 must lower to a runtime profile");
        assert!(profile.capabilities().has_image_processing);
    }

    #[test]
    fn picture_effect_contract_keeps_only_source_backed_values() {
        assert_eq!(PictureEffectMode::Off.as_byte(), 0x00);
        assert_eq!(PictureEffectMode::BlackAndWhite.as_byte(), 0x04);
        assert!(matches!(
            PictureEffectMode::from_byte(0x7f),
            PictureEffectMode::Unknown(0x7f)
        ));
    }
}
