//! This test should fail to compile because we're using a type that doesn't implement Profile

use grafton_visca::CameraBuilder;

// A type that doesn't implement Profile
struct NotAProfile;

fn main() {
    // This should fail: NotAProfile doesn't implement Profile
    let _camera = CameraBuilder::tcp("192.168.1.100:52381")
        .profile::<NotAProfile>() // Error: the trait bound `NotAProfile: Profile` is not satisfied
        .build();
}