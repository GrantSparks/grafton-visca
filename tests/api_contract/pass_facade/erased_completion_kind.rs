use grafton_visca::operation::{AppliedOnly, Kind};

// Receipt storage is private. Erasing a completion marker still needs only
// its original plan binding.
fn erased(_: &dyn Kind<Plan = ()>) {}

fn main() {
    erased(&AppliedOnly);
}
