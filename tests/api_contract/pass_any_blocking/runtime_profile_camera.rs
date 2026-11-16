use std::marker::PhantomData;

use grafton_visca::{
    blocking::Camera, blocking::Operation, blocking::Session, operation::AppliedOnly,
    profiles::GenericVisca, profiles::ProfileSpec, request::SubmissionClass,
    state_cache::StateCache, types::CameraId, Error,
};

fn surface(session: &Session, camera: &mut grafton_visca::blocking::Camera) -> Result<(), Error> {
    let _: CameraId = camera.target();
    let _: &ProfileSpec = camera.profile();
    let _: StateCache = camera.state_cache();
    let _: Option<SubmissionClass> = camera.submission_class();
    camera.set_submission_class(Some(SubmissionClass::Background));

    let _: Result<Camera<GenericVisca>, Error> = camera.clone().into_typed::<GenericVisca>();
    let _: Result<Operation<AppliedOnly>, Error> =
        camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop);

    let _: Result<grafton_visca::blocking::Camera, Error> = session
        .camera::<grafton_visca::profiles::AnyProfile>(
        grafton_visca::types::CameraId::CAMERA_1,
    );
    let _: Result<grafton_visca::blocking::Camera, Error> =
        session.camera::<grafton_visca::profiles::AnyProfile>(CameraId::CAMERA_1);
    Ok(())
}

fn main() {
    let _: PhantomData<grafton_visca::blocking::Camera> = PhantomData;
    let _ = surface;
}
