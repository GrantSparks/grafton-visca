//! This test should fail to compile because we're trying to call .profile() twice

use grafton_visca::{CameraBuilder, camera::profiles::{PTZOpticsG2, GenericVisca}};

fn main() {
    // This should fail: cannot call profile() twice
    let _camera = CameraBuilder::tcp("192.168.1.100:52381")
        .profile::<PTZOpticsG2>()
        .profile::<GenericVisca>() // Error: no method named `profile` found
        .build();
}