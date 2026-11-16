#![cfg(all(feature = "blocking", any(feature = "async", feature = "blocking")))]

use std::time::Duration;

use grafton_visca_test_support::fake_camera;

use fake_camera::FakeCamera;
use grafton_visca::{
    blocking::Session,
    capabilities::Capabilities,
    config::SessionConfig,
    profiles::{
        CommandTimeouts, ProfileEnvelope, ProfileSpec, ProfileTiming, TransportCompatibility,
    },
    transport::SendSemantics,
};

fn runtime_power_profile() -> ProfileSpec {
    let mut capabilities =
        Capabilities::runtime_baseline("Issue 720 runtime camera", 1).expect("baseline");
    capabilities.power = Some(grafton_visca::capabilities::PowerCaps {
        status_query: grafton_visca::capabilities::PowerStatusQuery::Standard,
        on_time: Duration::from_secs(1),
        standby: None,
        wake_on_lan: false,
        retains_settings: false,
        home_on_up: false,
    });

    let timing = ProfileTiming::builder()
        .ack_timeout(Duration::from_millis(100))
        .command_timeouts(CommandTimeouts::default())
        .inquiry_timeout(Duration::from_secs(1))
        .cancellation_timeout(Duration::from_secs(1))
        .ambiguity_timeout(Duration::from_secs(1))
        .busy_timeout(Duration::ZERO)
        .raw_inquiry_reply_skew(Duration::ZERO)
        .inquiry_spacing(Duration::ZERO)
        .command_spacing(Duration::ZERO)
        .build()
        .expect("timing");

    ProfileSpec::builder(capabilities)
        .transports(TransportCompatibility::new().with_tcp(5678))
        .envelope(ProfileEnvelope::RawVisca)
        .timing(timing)
        .maximum_command_sockets(1)
        .supports_operation_complete(true)
        .supports_command_cancel(false)
        .position_inquiries(None)
        .build()
        .expect("runtime power profile")
}

#[test]
fn blocking_dynamic_view_drives_a_runtime_only_profile() {
    let fake = FakeCamera::acking(1);
    let session = Session::open(
        fake.blocking_wire().with_semantics(SendSemantics::Stream),
        &SessionConfig::new(runtime_power_profile()),
    )
    .expect("runtime-profile session");

    let camera = session
        .camera::<grafton_visca::profiles::AnyProfile>(
            grafton_visca::types::IndividualCameraId::CAMERA_1,
        )
        .expect("dynamic blocking camera");
    assert_eq!(
        camera.profile().capabilities().model_name,
        "Issue 720 runtime camera"
    );
    camera
        .execute(&grafton_visca::command::power::On)
        .expect("power command");

    assert_eq!(
        fake.writes(),
        vec![vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xff]]
    );
    session.close().expect("close session");
}
