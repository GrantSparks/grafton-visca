use grafton_visca::{
    profiles::PtzOpticsG2, transport::BlockingTransportHandle, BlockingCamera, Error,
};

fn unsupported_tally(
    camera: BlockingCamera<PtzOpticsG2, BlockingTransportHandle>,
) -> Result<(), Error> {
    camera.tally().bright_hi()?;
    Ok(())
}

fn main() {
    let _ = unsupported_tally;
}
