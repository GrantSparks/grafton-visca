//@ build
//! A compile-time profile whose pan/tilt wire codec cannot frame its
//! coordinate system (Sony BRC-300 framing with unsigned-centered
//! coordinates) fails to build where its conversion is taken.

use grafton_visca::{
    capabilities::{CapabilityRange, CoordinateSystem, PanTilt, PanTiltWireCodec},
    PanTiltCoordinateConversion,
};

struct Inconsistent;

impl PanTilt for Inconsistent {
    const PAN_RANGE: CapabilityRange<i32> = CapabilityRange::<i32>::new(-100, 100);
    const TILT_RANGE: CapabilityRange<i32> = CapabilityRange::<i32>::new(-100, 100);
    const MAX_PAN_SPEED: u8 = 0x18;
    const MAX_TILT_SPEED: u8 = 0x18;
    const PAN_DEGREES_TO_UNITS: f32 = 1.0;
    const TILT_DEGREES_TO_UNITS: f32 = 1.0;
    const COORDINATE_SYSTEM: CoordinateSystem = CoordinateSystem::UnsignedCentered;
    const PAN_TILT_WIRE_CODEC: PanTiltWireCodec = PanTiltWireCodec::SonyBrc300;
}

fn main() {
    let conversion = PanTiltCoordinateConversion::for_profile::<Inconsistent>();
    println!("{:?}", conversion.wire_codec());
}

//~ E0080
//~ "Sony BRC-300 pan/tilt framing requires signed-centered coordinates"
