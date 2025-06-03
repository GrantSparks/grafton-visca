/// Camera-specific constants for PTZOptics cameras
pub mod ptzoptics_g2 {
    /// Maximum pan position value in VISCA units
    pub const PAN_MAX: i16 = 2448;

    /// Maximum tilt position value in VISCA units  
    pub const TILT_MAX: i16 = 1296;

    /// Maximum zoom position value in VISCA units
    pub const ZOOM_MAX: u16 = 16384;

    /// Pan range in degrees
    pub const PAN_RANGE_DEGREES: f32 = 340.0;

    /// Tilt range in degrees
    pub const TILT_RANGE_DEGREES: f32 = 240.0;
}
