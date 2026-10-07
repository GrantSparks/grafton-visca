use grafton_visca::Error;
fn main() {
    match Error::connection_closed(None) {
        Error::ConnectionClosed { reason } => { let _ = reason; }
        _ => {}
    }
}
//~ E0638
//~ "`..` required with variant marked as non-exhaustive"
