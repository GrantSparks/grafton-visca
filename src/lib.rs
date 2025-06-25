//! # grafton-visca
//!
//! Rust library for VISCA-over-IP protocol to control PTZ cameras.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
)]
#![warn(
    clippy::all,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unused_qualifications,
)]

/// Camera profile system for type-safe, model-specific control
pub mod camera;

/// VISCA command definitions
pub mod command;

/// Error types
mod error;
pub use error::{Error, Result};

/// Transport layer (TCP, UDP, serial, etc.)
pub mod transport;

/// Type definitions and abstractions
pub mod types;

/// Semantic unit types (Degrees, Percentage, etc.)
pub mod units;

/// Timeout utilities (for internal macros)
pub mod timeout;

mod constants;
mod macros;

// Core API exports
pub use camera::{
    Camera,
    CameraProfile,
    CustomProfile,
    CustomProfileBuilder,
    CustomProfileTypedBuilder,
};
pub use command::{Command, InquiryResponse, Response};
pub use types::{FStop, IntoIrisLevel};
pub use units::{
    Degrees,
    Fraction,
    Kelvin,
    Magnification,
    Normalized,
    Percentage,
    Raw,
    ViscaUnits,
};

/// Predefined camera profiles
pub mod profiles {
    pub use crate::camera::profiles::{
        GenericVisca,
        PTZOptics30X,
        PTZOpticsG2,
        SonyEVID70,
    };
}

// ## What is VISCA?
//
// VISCA (Video System Control Architecture) is Sony's protocol for controlling PTZ cameras
// over IP. It is widely used in robotics, broadcasting, conferencing, and surveillance.
//
// ## Features
//
// - **Type-safe profiles**: Compile-time validation per camera model
// - **Full command coverage**: Supports PTZOptics G2, Sony EVI-D70, and more
// - **Profile-aware units**: Automatic conversions based on camera specs
// - **Inquiry support**: Query camera state for all features
// - **Transport-agnostic**: Plug in TCP, UDP, serial, etc.
// - **Builder patterns**: Define custom profiles
// - **Runtime-agnostic async**: Works with tokio, async-std, smol, etc.
//
// ## Quick Start
//
// ```ignore
// use grafton_visca::{Camera, profiles::PTZOpticsG2, transport::blocking::create, units::Degrees};
//
// fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let transport = create::udp("192.168.1.100:52381")?;
//     let mut camera = Camera::<PTZOpticsG2, _>::new(transport);
//
//     camera.power_on()?;
//     camera.home()?;
//     camera.set_position(Degrees(45.0), Degrees(-15.0))?;
//     camera.zoom_in()?;
//     camera.zoom_stop()?;
//     Ok(())
// }
// ```
//
// For async usage with tokio:
//
// ```ignore
// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let transport = grafton_visca::transport::create::tcp("192.168.1.100:5678").await?;
//     let camera = Camera::<PTZOpticsG2, _>::new(transport);
//     camera.power_on().await?;
//     camera.home().await?;
//     Ok(())
// }
// 
