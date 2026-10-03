use grafton_visca::{completion::Targeted, Operation};

// Waits borrow the handle exclusively (#777): two waits cannot be in flight
// on one handle at once.
async fn concurrent_waits(mut handle: Operation<Targeted>) {
    let applied = handle.applied();
    let settled = handle.settled();
    let _ = (applied.await, settled.await);
}

fn main() {
    let _ = concurrent_waits;
}

//~ E0499
//~ "cannot borrow `handle` as mutable more than once at a time"
