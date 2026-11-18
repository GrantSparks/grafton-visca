fn main() {
    let _ = <grafton_visca::types::IrisLevel as From<grafton_visca::types::FStop>>::from(
    //~^ E0277 "From<FStop>"
        grafton_visca::types::FStop::F5_6,
    );
}
