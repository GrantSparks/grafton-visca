//! Socket capacity accepts the checked value, so invalid raw counts cannot cross configuration.
fn main() {
    let _ = grafton_visca::config::OperationalTuning::new().with_maximum_command_sockets(0);
    //~^ E0308 "mismatched types"
}
