//! Tally light control methods for cameras.

use crate::Error;

/// Tally light control operations (async).
#[cfg(feature = "async")]
pub trait TallyControl: Sized {
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
pub trait TallyControlBlocking: Sized {
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

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> TallyControl for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor::Executor,
{
    async fn tally_red_on(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::RedOn;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn tally_red_off(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::RedOff;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn tally_bright_lo(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::BrightLo;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn tally_bright_hi(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::BrightHi;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn tally_green_on(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::GreenOn;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn tally_green_off(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::GreenOff;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn tally_flash(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::Flash;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn tally_on(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::On;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn tally_off(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::Off;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn get_tally_status(&self) -> Result<bool, Error> {
        use crate::command::{response::ViscaResponse, tally::TallyInquiry, InquiryResponse};
        let inquiry = TallyInquiry::Red;
        let response = self.send_command(&inquiry).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TallyRed { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_red_tally_status(&self) -> Result<bool, Error> {
        use crate::command::{response::ViscaResponse, tally::TallyInquiry, InquiryResponse};
        let inquiry = TallyInquiry::Red;
        let response = self.send_command(&inquiry).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TallyRed { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_green_tally_status(&self) -> Result<bool, Error> {
        use crate::command::{response::ViscaResponse, tally::TallyInquiry, InquiryResponse};
        let inquiry = TallyInquiry::Green;
        let response = self.send_command(&inquiry).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TallyGreen { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation for Camera with BlockingMode
impl<P, T> TallyControlBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile,
    T: crate::transport::BlockingTransport + Send + Sync + 'static,
{
    fn tally_red_on(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::RedOn;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn tally_red_off(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::RedOff;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn tally_bright_lo(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::BrightLo;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn tally_bright_hi(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::BrightHi;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn tally_green_on(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::GreenOn;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn tally_green_off(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::GreenOff;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn tally_flash(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::Flash;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn tally_on(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::On;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn tally_off(&self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::Off;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn get_tally_status(&self) -> Result<bool, Error> {
        use crate::command::{response::ViscaResponse, tally::TallyInquiry, InquiryResponse};
        let inquiry = TallyInquiry::Red;
        let response = self.send_command(&inquiry)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TallyRed { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_red_tally_status(&self) -> Result<bool, Error> {
        use crate::command::{response::ViscaResponse, tally::TallyInquiry, InquiryResponse};
        let inquiry = TallyInquiry::Red;
        let response = self.send_command(&inquiry)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TallyRed { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_green_tally_status(&self) -> Result<bool, Error> {
        use crate::command::{response::ViscaResponse, tally::TallyInquiry, InquiryResponse};
        let inquiry = TallyInquiry::Green;
        let response = self.send_command(&inquiry)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TallyGreen { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}
