use grafton_visca::{
    camera::CameraSession,
    mode::Async,
    profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
    Executor,
};

fn ptzoptics_g2_session<Tr, Exec>(session: &CameraSession<Async, PtzOpticsG2, Tr, Exec>)
where
    Exec: Executor,
{
    let _ = session.nd_filter();
    let _ = session.motion_sync();
}

fn generic_session<Tr, Exec>(session: &CameraSession<Async, GenericVisca, Tr, Exec>)
where
    Exec: Executor,
{
    let _ = session.motion_sync();
}

fn sony_fr7_session<Tr, Exec>(session: &CameraSession<Async, SonyFR7, Tr, Exec>)
where
    Exec: Executor,
{
    let _ = session.motion_sync();
}

fn main() {}
