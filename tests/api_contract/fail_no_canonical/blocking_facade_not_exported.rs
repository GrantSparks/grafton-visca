use grafton_visca::blocking::Session;
//~^ E0432 "unresolved import `grafton_visca::blocking`"

fn main() {
    let _: Option<Session> = None;
}
