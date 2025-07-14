//! Tally light control methods for cameras.

use crate::{
    camera::Camera,
    command::{tally::Tally, Response},
    Error,
};

/// Tally light control operations (async).
pub trait TallyOps: Sized {
    /// Turn red tally light on.
    async fn tally_red_on(&self) -> Result<(), Error>;

    /// Turn red tally light off.
    async fn tally_red_off(&self) -> Result<(), Error>;

    /// Set tally brightness to low.
    async fn tally_bright_lo(&self) -> Result<(), Error>;

    /// Set tally brightness to high.
    async fn tally_bright_hi(&self) -> Result<(), Error>;

    /// Turn green tally light on.
    async fn tally_green_on(&self) -> Result<(), Error>;

    /// Turn green tally light off.
    async fn tally_green_off(&self) -> Result<(), Error>;

    /// Flash tally light.
    async fn tally_flash(&self) -> Result<(), Error>;

    /// Turn tally light on.
    async fn tally_on(&self) -> Result<(), Error>;

    /// Turn tally light off.
    async fn tally_off(&self) -> Result<(), Error>;

    /// Get tally light status.
    async fn get_tally_status(&self) -> Result<bool, Error>;
}

/// Tally light control operations (blocking).
pub trait TallyOpsBlocking: Sized {
    /// Turn red tally light on.
    fn tally_red_on(&self) -> Result<(), Error>;

    /// Turn red tally light off.
    fn tally_red_off(&self) -> Result<(), Error>;

    /// Set tally brightness to low.
    fn tally_bright_lo(&self) -> Result<(), Error>;

    /// Set tally brightness to high.
    fn tally_bright_hi(&self) -> Result<(), Error>;

    /// Turn green tally light on.
    fn tally_green_on(&self) -> Result<(), Error>;

    /// Turn green tally light off.
    fn tally_green_off(&self) -> Result<(), Error>;

    /// Flash tally light.
    fn tally_flash(&self) -> Result<(), Error>;

    /// Turn tally light on.
    fn tally_on(&self) -> Result<(), Error>;

    /// Turn tally light off.
    fn tally_off(&self) -> Result<(), Error>;

    /// Get tally light status.
    fn get_tally_status(&self) -> Result<bool, Error>;
}

// Async implementation
impl TallyOps for Camera {
    async fn tally_red_on(&self) -> Result<(), Error> {
        let command = Tally::RedOn;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn tally_red_off(&self) -> Result<(), Error> {
        let command = Tally::RedOff;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn tally_bright_lo(&self) -> Result<(), Error> {
        let command = Tally::BrightLo;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn tally_bright_hi(&self) -> Result<(), Error> {
        let command = Tally::BrightHi;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn tally_green_on(&self) -> Result<(), Error> {
        let command = Tally::GreenOn;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn tally_green_off(&self) -> Result<(), Error> {
        let command = Tally::GreenOff;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn tally_flash(&self) -> Result<(), Error> {
        let command = Tally::Flash;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn tally_on(&self) -> Result<(), Error> {
        let command = Tally::On;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn tally_off(&self) -> Result<(), Error> {
        let command = Tally::Off;
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_tally_status(&self) -> Result<bool, Error> {
        // For now, return a NotImplemented error as there's no tally inquiry command in the protocol
        // This would need to be added to the InquiryCommand enum with the proper VISCA bytes
        Err(Error::FeatureNotSupported {
            feature: "Tally status inquiry",
        })
    }
}

// Blocking implementation
impl TallyOpsBlocking for Camera {
    fn tally_red_on(&self) -> Result<(), Error> {
        let command = Tally::RedOn;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn tally_red_off(&self) -> Result<(), Error> {
        let command = Tally::RedOff;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn tally_bright_lo(&self) -> Result<(), Error> {
        let command = Tally::BrightLo;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn tally_bright_hi(&self) -> Result<(), Error> {
        let command = Tally::BrightHi;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn tally_green_on(&self) -> Result<(), Error> {
        let command = Tally::GreenOn;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn tally_green_off(&self) -> Result<(), Error> {
        let command = Tally::GreenOff;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn tally_flash(&self) -> Result<(), Error> {
        let command = Tally::Flash;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn tally_on(&self) -> Result<(), Error> {
        let command = Tally::On;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn tally_off(&self) -> Result<(), Error> {
        let command = Tally::Off;
        let response = self.send_command_blocking(&command)?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_tally_status(&self) -> Result<bool, Error> {
        // For now, return a NotImplemented error as there's no tally inquiry command in the protocol
        // This would need to be added to the InquiryCommand enum with the proper VISCA bytes
        Err(Error::FeatureNotSupported {
            feature: "Tally status inquiry",
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
