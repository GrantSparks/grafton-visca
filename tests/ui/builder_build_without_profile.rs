//! This test should fail to compile because we're trying to call .build() before .profile()

use grafton_visca::CameraBuilder;

fn main() {
    // This should fail: cannot call build() directly on CameraBuilder
    let _camera = CameraBuilder::tcp("192.168.1.100:52381")
        .build(); // Error: no method named `build` found
}