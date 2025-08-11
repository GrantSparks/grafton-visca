//! Tally light control methods for cameras.

use crate::{
    command::{
        tally::{Tally, TallyInquiry},
        InquiryResponse,
    },
    Error,
};

/// Tally light control operations (async).
#[cfg(feature = "async")]
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

    /// Query red tally light state.
    async fn get_red_tally_status(&self) -> Result<bool, Error>;

    /// Query green tally light state (FR7 specific).
    async fn get_green_tally_status(&self) -> Result<bool, Error>;
}

/// Tally light control operations (blocking).
#[cfg(not(feature = "async"))]
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

    /// Query red tally light state.
    fn get_red_tally_status(&self) -> Result<bool, Error>;

    /// Query green tally light state (FR7 specific).
    fn get_green_tally_status(&self) -> Result<bool, Error>;
}

// Async implementation
#[cfg(feature = "async")]
impl<P: crate::capabilities::Profile, T: crate::transport::Transport + Send + Sync + 'static>
    TallyOps for crate::camera::generic::Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    async fn tally_red_on(&self) -> Result<(), Error> {
        let command = Tally::RedOn;
        self.send_action_command(&command).await
    }

    async fn tally_red_off(&self) -> Result<(), Error> {
        let command = Tally::RedOff;
        self.send_action_command(&command).await
    }

    async fn tally_bright_lo(&self) -> Result<(), Error> {
        let command = Tally::BrightLo;
        self.send_action_command(&command).await
    }

    async fn tally_bright_hi(&self) -> Result<(), Error> {
        let command = Tally::BrightHi;
        self.send_action_command(&command).await
    }

    async fn tally_green_on(&self) -> Result<(), Error> {
        let command = Tally::GreenOn;
        self.send_action_command(&command).await
    }

    async fn tally_green_off(&self) -> Result<(), Error> {
        let command = Tally::GreenOff;
        self.send_action_command(&command).await
    }

    async fn tally_flash(&self) -> Result<(), Error> {
        let command = Tally::Flash;
        self.send_action_command(&command).await
    }

    async fn tally_on(&self) -> Result<(), Error> {
        let command = Tally::On;
        self.send_action_command(&command).await
    }

    async fn tally_off(&self) -> Result<(), Error> {
        let command = Tally::Off;
        self.send_action_command(&command).await
    }

    async fn get_tally_status(&self) -> Result<bool, Error> {
        // Default to red tally status for backward compatibility
        TallyOps::get_red_tally_status(self).await
    }

    async fn get_red_tally_status(&self) -> Result<bool, Error> {
        let command = TallyInquiry::Red;
        let inquiry = self.send_inquiry_command(&command).await?;
        match inquiry {
            InquiryResponse::TallyRed { on } => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_green_tally_status(&self) -> Result<bool, Error> {
        let command = TallyInquiry::Green;
        let inquiry = self.send_inquiry_command(&command).await?;
        match inquiry {
            InquiryResponse::TallyGreen { on } => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation
#[cfg(not(feature = "async"))]
impl<
        P: crate::capabilities::Profile,
        T: crate::transport::Transport
            + Send
            + Sync
            + 'static
            + crate::transport::core::BlockingTransport,
    > TallyOpsBlocking for crate::camera::generic::Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn tally_red_on(&self) -> Result<(), Error> {
        let command = Tally::RedOn;
        self.send_action_command_blocking(&command)
    }

    fn tally_red_off(&self) -> Result<(), Error> {
        let command = Tally::RedOff;
        self.send_action_command_blocking(&command)
    }

    fn tally_bright_lo(&self) -> Result<(), Error> {
        let command = Tally::BrightLo;
        self.send_action_command_blocking(&command)
    }

    fn tally_bright_hi(&self) -> Result<(), Error> {
        let command = Tally::BrightHi;
        self.send_action_command_blocking(&command)
    }

    fn tally_green_on(&self) -> Result<(), Error> {
        let command = Tally::GreenOn;
        self.send_action_command_blocking(&command)
    }

    fn tally_green_off(&self) -> Result<(), Error> {
        let command = Tally::GreenOff;
        self.send_action_command_blocking(&command)
    }

    fn tally_flash(&self) -> Result<(), Error> {
        let command = Tally::Flash;
        self.send_action_command_blocking(&command)
    }

    fn tally_on(&self) -> Result<(), Error> {
        let command = Tally::On;
        self.send_action_command_blocking(&command)
    }

    fn tally_off(&self) -> Result<(), Error> {
        let command = Tally::Off;
        self.send_action_command_blocking(&command)
    }

    fn get_tally_status(&self) -> Result<bool, Error> {
        // Default to red tally status for backward compatibility
        Self::get_red_tally_status(self)
    }

    fn get_red_tally_status(&self) -> Result<bool, Error> {
        let command = TallyInquiry::Red;
        let inquiry = self.send_inquiry_command_blocking(&command)?;
        match inquiry {
            InquiryResponse::TallyRed { on } => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_green_tally_status(&self) -> Result<bool, Error> {
        let command = TallyInquiry::Green;
        let inquiry = self.send_inquiry_command_blocking(&command)?;
        match inquiry {
            InquiryResponse::TallyGreen { on } => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_tally_methods_compile() {
        use crate::Error;
        // This test demonstrates that tally methods are available for all cameras

        fn _test_tally_methods<P, T>(_camera: &crate::Camera<P, T>)
        where
            P: crate::capabilities::Profile,
            T: crate::transport::Transport + Send + Sync + 'static,
            T::Error: Into<Error> + Send,
            for<'a> T::SendFut<'a>: Send,
            for<'a> T::RecvFut<'a>: Send,
        {
            // All cameras can use tally methods
        }
    }
}
