use grafton_visca::{profiles::PtzOpticsG2, r#async::Session};

fn missing_target(session: &Session) {
    let _ = session.camera::<PtzOpticsG2>();
    //~^ E0061 "this method takes 1 argument but 0 arguments were supplied"
}

fn main() {
    let _ = missing_target;
}
