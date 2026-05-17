use grafton_visca::{
    command::{Response, ViscaCommand},
    profiles::PtzOpticsG2,
    timeout::CommandCategory,
    transport::BlockingTransportHandle,
    BlockingCamera, CameraId, Error,
};

struct CustomCommand;

impl ViscaCommand for CustomCommand {
    const MAX_SIZE: usize = 6;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        buffer[..Self::MAX_SIZE].copy_from_slice(&[
            camera_id.to_address_byte(),
            0x01,
            0x04,
            0x00,
            0x02,
            0xFF,
        ]);
        Ok(Self::MAX_SIZE)
    }
}

fn use_raw_escape_hatch(
    camera: &BlockingCamera<PtzOpticsG2, BlockingTransportHandle>,
) -> Result<Response, Error> {
    camera.execute(CustomCommand)?;
    camera.send_command(&CustomCommand)
}

fn main() {
    let _ = use_raw_escape_hatch;
}
