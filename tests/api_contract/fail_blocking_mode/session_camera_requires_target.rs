use grafton_visca::{blocking::Session, profiles::PtzOpticsG2};

fn missing_target(session: &Session) {
    let _ = session.camera::<PtzOpticsG2>();
}

fn main() {
    let _ = missing_target;
}

//~ E0061
//~ "this method takes 1 argument but 0 arguments were supplied"
