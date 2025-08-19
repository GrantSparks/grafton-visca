//! Tests to verify compile-time safety of the generic camera API.

#[cfg(feature = "async")]
use grafton_visca::camera::AsyncMode;
#[cfg(feature = "async")]
use grafton_visca::prelude::r#async::{PtzOpticsG2Cam, SonyFR7Cam};

#[cfg(not(feature = "async"))]
use grafton_visca::camera::BlockingMode;
#[cfg(not(feature = "async"))]
use grafton_visca::prelude::blocking::{PtzOpticsG2Cam, SonyFR7Cam};

use grafton_visca::{
    camera::{
        profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
        Camera,
    },
    capabilities::{PanTilt, Profile, ProfileMetadata, Zoom},
};

#[test]
fn test_profile_super_trait() {
    // These types should compile because they implement Profile
    fn accepts_profile<P: Profile>() {}

    accepts_profile::<PtzOpticsG2>();
    accepts_profile::<SonyFR7>();
    accepts_profile::<GenericVisca>();
}

#[test]
fn test_profile_constants() {
    assert_eq!(PtzOpticsG2::MODEL_NAME, "PtzOptics G2");
    assert_eq!(SonyFR7::MODEL_NAME, "Sony FR7");
    assert_eq!(GenericVisca::MODEL_NAME, "Generic VISCA Camera");

    assert_eq!(PtzOpticsG2::ZOOM_SPEED_RANGE, 0..8);
    assert_eq!(SonyFR7::ZOOM_SPEED_RANGE, 0..8);

    assert_eq!(PtzOpticsG2::MAX_PAN_SPEED, 24);
    assert_eq!(SonyFR7::MAX_PAN_SPEED, 24);
}

#[test]
fn test_generic_camera_methods() {
    #[cfg(feature = "async")]
    type _G2Camera<T, E> = Camera<AsyncMode, PtzOpticsG2, T, E>;
    #[cfg(not(feature = "async"))]
    type _G2Camera<T> = Camera<BlockingMode, PtzOpticsG2, T, ()>;

    #[cfg(feature = "async")]
    type _FR7Camera<T, E> = Camera<AsyncMode, SonyFR7, T, E>;
    #[cfg(not(feature = "async"))]
    type _FR7Camera<T> = Camera<BlockingMode, SonyFR7, T, ()>;

    type _G2Alias<T> = PtzOpticsG2Cam<T>;
    type _FR7Alias<T> = SonyFR7Cam<T>;
}
