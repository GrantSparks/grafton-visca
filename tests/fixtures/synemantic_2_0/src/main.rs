use std::marker::PhantomData;

use grafton_visca::{
    dynapi::DynSessionCameraControl, profiles::GenericVisca, Camera, CompileTimeProfile,
    ProfileSpec, Session, SessionConfig,
};

fn typed_camera_surface<P: CompileTimeProfile>() -> PhantomData<Camera<P>> {
    PhantomData
}

fn dynamic_camera_surface(_: Option<&dyn DynSessionCameraControl>) {}

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
    dynamic_camera_surface(None);
}
