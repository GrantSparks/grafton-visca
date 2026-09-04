use grafton_visca::{
    blocking::Operation,
    completion::{AppliedOnly, Targeted},
};

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

fn main() {
    // Blocking handles retain a caller-thread host reference. They must not
    // become transferable across threads merely because their completion
    // marker is zero-sized.
    assert_send::<Operation<'static, AppliedOnly>>();
    assert_sync::<Operation<'static, AppliedOnly>>();
    assert_send::<Operation<'static, Targeted>>();
    assert_sync::<Operation<'static, Targeted>>();
}

//~ E0277
//~ "cannot be shared between threads safely"
