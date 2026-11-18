use grafton_visca::r#async::Session;
//~^ E0432 "unresolved import `grafton_visca::r#async`"

fn main() {
    let _: Option<Session> = None;
}
