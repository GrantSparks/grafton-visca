use grafton_visca::Session;

fn main() {
    let _ = Session::new;
    let _ = Session::open_with_runtime;
    let _ = Session::start;
}

//~ E0599
//~ "named `new` found for struct"
//~ "named `open_with_runtime` found for struct"
//~ "named `start` found for struct"
