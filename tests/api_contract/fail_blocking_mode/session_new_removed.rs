use grafton_visca::blocking::Session;

fn main() {
    let _ = Session::new;
}

//~ E0599
//~ "named `new` found for struct"
