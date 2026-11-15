use grafton_visca::completion::{Settlement, Targeted};

fn main() {
    // The lifecycle's public outcome remains independently available.
    let _: Option<grafton_visca::Settlement> = None;
    let _: Option<Settlement<Targeted>> = None;
}

//~ E0603
//~ "struct `Settlement` is private"
