//! Tally light control methods for cameras.

use crate::{
    camera::unified::Camera,
    command::{tally::Tally, Response},
    Error,
};


/// Tally light control operations.
pub trait TallyOps: Sized {
    /// Turn red tally light on.
    #[cfg(feature = "tokio")]
    async fn tally_red_on(&self) -> Result<(), Error>;

    /// Turn red tally light on (blocking).
    #[cfg(not(feature = "tokio"))]
    fn tally_red_on_blocking(&mut self) -> Result<(), Error>;

    /// Turn red tally light off.
    #[cfg(feature = "tokio")]
    async fn tally_red_off(&self) -> Result<(), Error>;

    /// Turn red tally light off (blocking).
    #[cfg(not(feature = "tokio"))]
    fn tally_red_off_blocking(&mut self) -> Result<(), Error>;

    /// Set tally brightness to low.
    #[cfg(feature = "tokio")]
    async fn tally_bright_lo(&self) -> Result<(), Error>;

    /// Set tally brightness to low (blocking).
    #[cfg(not(feature = "tokio"))]
    fn tally_bright_lo_blocking(&mut self) -> Result<(), Error>;

    /// Set tally brightness to high.
    #[cfg(feature = "tokio")]
    async fn tally_bright_hi(&self) -> Result<(), Error>;

    /// Set tally brightness to high (blocking).
    #[cfg(not(feature = "tokio"))]
    fn tally_bright_hi_blocking(&mut self) -> Result<(), Error>;

    /// Turn green tally light on.
    #[cfg(feature = "tokio")]
    async fn tally_green_on(&self) -> Result<(), Error>;

    /// Turn green tally light on (blocking).
    #[cfg(not(feature = "tokio"))]
    fn tally_green_on_blocking(&mut self) -> Result<(), Error>;

    /// Turn green tally light off.
    #[cfg(feature = "tokio")]
    async fn tally_green_off(&self) -> Result<(), Error>;

    /// Turn green tally light off (blocking).
    #[cfg(not(feature = "tokio"))]
    fn tally_green_off_blocking(&mut self) -> Result<(), Error>;

    /// Flash tally light.
    #[cfg(feature = "tokio")]
    async fn tally_flash(&self) -> Result<(), Error>;

    /// Flash tally light (blocking).
    #[cfg(not(feature = "tokio"))]
    fn tally_flash_blocking(&mut self) -> Result<(), Error>;

    /// Turn tally light on.
    #[cfg(feature = "tokio")]
    async fn tally_on(&self) -> Result<(), Error>;

    /// Turn tally light on (blocking).
    #[cfg(not(feature = "tokio"))]
    fn tally_on_blocking(&mut self) -> Result<(), Error>;

    /// Turn tally light off.
    #[cfg(feature = "tokio")]
    async fn tally_off(&self) -> Result<(), Error>;

    /// Turn tally light off (blocking).
    #[cfg(not(feature = "tokio"))]
    fn tally_off_blocking(&mut self) -> Result<(), Error>;

    /// Get tally light status.
    #[cfg(feature = "tokio")]
    async fn get_tally_status(&self) -> Result<bool, Error>;

    /// Get tally light status (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_tally_status_blocking(&mut self) -> Result<bool, Error>;
}

impl TallyOps for Camera {
    #[cfg(feature = "tokio")]
    async fn tally_red_on(&self) -> Result<(), Error> {
        let command = Tally::RedOn;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn tally_red_on_blocking(&mut self) -> Result<(), Error> {
        let command = Tally::RedOn;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(feature = "tokio")]
    async fn tally_red_off(&self) -> Result<(), Error> {
        let command = Tally::RedOff;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn tally_red_off_blocking(&mut self) -> Result<(), Error> {
        let command = Tally::RedOff;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(feature = "tokio")]
    async fn tally_bright_lo(&self) -> Result<(), Error> {
        let command = Tally::BrightLo;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn tally_bright_lo_blocking(&mut self) -> Result<(), Error> {
        let command = Tally::BrightLo;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(feature = "tokio")]
    async fn tally_bright_hi(&self) -> Result<(), Error> {
        let command = Tally::BrightHi;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn tally_bright_hi_blocking(&mut self) -> Result<(), Error> {
        let command = Tally::BrightHi;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(feature = "tokio")]
    async fn tally_green_on(&self) -> Result<(), Error> {
        let command = Tally::GreenOn;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn tally_green_on_blocking(&mut self) -> Result<(), Error> {
        let command = Tally::GreenOn;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(feature = "tokio")]
    async fn tally_green_off(&self) -> Result<(), Error> {
        let command = Tally::GreenOff;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn tally_green_off_blocking(&mut self) -> Result<(), Error> {
        let command = Tally::GreenOff;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(feature = "tokio")]
    async fn tally_flash(&self) -> Result<(), Error> {
        let command = Tally::Flash;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn tally_flash_blocking(&mut self) -> Result<(), Error> {
        let command = Tally::Flash;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(feature = "tokio")]
    async fn tally_on(&self) -> Result<(), Error> {
        let command = Tally::On;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn tally_on_blocking(&mut self) -> Result<(), Error> {
        let command = Tally::On;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(feature = "tokio")]
    async fn tally_off(&self) -> Result<(), Error> {
        let command = Tally::Off;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn tally_off_blocking(&mut self) -> Result<(), Error> {
        let command = Tally::Off;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(feature = "tokio")]
    async fn get_tally_status(&self) -> Result<bool, Error> {
        // For now, return a NotImplemented error as there's no tally inquiry command in the protocol
        // This would need to be added to the InquiryCommand enum with the proper VISCA bytes
        Err(Error::FeatureNotSupported {
            feature: "Tally status inquiry".to_string(),
        })
    }

    #[cfg(not(feature = "tokio"))]
    fn get_tally_status_blocking(&mut self) -> Result<bool, Error> {
        // For now, return a NotImplemented error as there's no tally inquiry command in the protocol
        // This would need to be added to the InquiryCommand enum with the proper VISCA bytes
        Err(Error::FeatureNotSupported {
            feature: "Tally status inquiry".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tally_methods_compile() {
        // This test demonstrates that tally methods are available for all cameras
        
        fn _test_tally_methods(_camera: &Camera) {
            // All cameras can use tally methods
        }
    }
}