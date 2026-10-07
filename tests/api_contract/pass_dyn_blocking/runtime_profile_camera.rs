use std::marker::PhantomData;

use grafton_visca::{
    blocking::{Camera, Operation, Session},
    completion::AppliedOnly,
    dynapi::BlockingDynSessionCamera,
    profiles::GenericVisca,
    request::builtin::ZoomStop,
    CameraId, Error, ProfileSpec, StateCache, SubmissionClass,
};

fn surface(
    session: &Session,
    camera: &mut BlockingDynSessionCamera,
) -> Result<(), Error> {
    let _: CameraId = camera.target();
    let _: &ProfileSpec = camera.profile();
    let _: StateCache = camera.state_cache();
    let _: Option<SubmissionClass> = camera.submission_class();
    camera.set_submission_class(Some(SubmissionClass::Background));

    let _: Result<Camera<GenericVisca>, Error> = camera.camera::<GenericVisca>();
    let _: Result<Operation<AppliedOnly>, Error> =
        camera.submit::<AppliedOnly, _>(&ZoomStop);

    let _: Result<BlockingDynSessionCamera, Error> = session.camera_dyn();
    let _: Result<BlockingDynSessionCamera, Error> =
        session.camera_dyn_for(CameraId::CAMERA_1);
    Ok(())
}

fn main() {
    let _: PhantomData<BlockingDynSessionCamera> = PhantomData;
    let _ = surface;
}
