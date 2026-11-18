use grafton_visca::Error;
fn main() {
    match Error::connection_closed(None) {
        Error::ConnectionClosed { source } => {
        //~^ E0638 "`..` required with variant marked as non-exhaustive"
            let _ = source;
        }
        _ => {}
    }
}
