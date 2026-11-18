use grafton_visca::r#async::Session;

fn main() {
    let _ = Session::new;
    //~^ E0599 "named `new` found for struct"
    let _ = Session::open_with_runtime;
    //~^ E0599 "named `open_with_runtime` found for struct"
    let _ = Session::start;
    //~^ E0599 "named `start` found for struct"
}
