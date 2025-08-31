//! Tally light control methods for unified camera API.

use crate::Error;

/// Tally light control operations for cameras.
///
/// This trait provides tally light control methods that work for both blocking and async cameras.
/// The implementation differs based on the camera type - async cameras return futures,
/// while blocking cameras perform operations synchronously.
pub trait TallyControl {
    /// Turn red tally light on.
    #[cfg(feature = "async")]
    fn tally_red_on(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Turn red tally light on.
    #[cfg(not(feature = "async"))]
    fn tally_red_on(&mut self) -> Result<(), Error>;

    /// Turn red tally light off.
    #[cfg(feature = "async")]
    fn tally_red_off(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Turn red tally light off.
    #[cfg(not(feature = "async"))]
    fn tally_red_off(&mut self) -> Result<(), Error>;

    /// Set tally brightness to low.
    #[cfg(feature = "async")]
    fn tally_bright_lo(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set tally brightness to low.
    #[cfg(not(feature = "async"))]
    fn tally_bright_lo(&mut self) -> Result<(), Error>;

    /// Set tally brightness to high.
    #[cfg(feature = "async")]
    fn tally_bright_hi(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set tally brightness to high.
    #[cfg(not(feature = "async"))]
    fn tally_bright_hi(&mut self) -> Result<(), Error>;

    /// Turn green tally light on.
    #[cfg(feature = "async")]
    fn tally_green_on(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Turn green tally light on.
    #[cfg(not(feature = "async"))]
    fn tally_green_on(&mut self) -> Result<(), Error>;

    /// Turn green tally light off.
    #[cfg(feature = "async")]
    fn tally_green_off(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Turn green tally light off.
    #[cfg(not(feature = "async"))]
    fn tally_green_off(&mut self) -> Result<(), Error>;

    /// Flash tally light.
    #[cfg(feature = "async")]
    fn tally_flash(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Flash tally light.
    #[cfg(not(feature = "async"))]
    fn tally_flash(&mut self) -> Result<(), Error>;

    /// Turn tally light on.
    #[cfg(feature = "async")]
    fn tally_on(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Turn tally light on.
    #[cfg(not(feature = "async"))]
    fn tally_on(&mut self) -> Result<(), Error>;

    /// Turn tally light off.
    #[cfg(feature = "async")]
    fn tally_off(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Turn tally light off.
    #[cfg(not(feature = "async"))]
    fn tally_off(&mut self) -> Result<(), Error>;

    /// Get tally light status.
    #[cfg(feature = "async")]
    fn get_tally_status(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Get tally light status.
    #[cfg(not(feature = "async"))]
    fn get_tally_status(&mut self) -> Result<bool, Error>;

    /// Query red tally light state.
    #[cfg(feature = "async")]
    fn get_red_tally_status(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Query red tally light state.
    #[cfg(not(feature = "async"))]
    fn get_red_tally_status(&mut self) -> Result<bool, Error>;

    /// Query green tally light state (FR7 specific).
    #[cfg(feature = "async")]
    fn get_green_tally_status(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Query green tally light state (FR7 specific).
    #[cfg(not(feature = "async"))]
    fn get_green_tally_status(&mut self) -> Result<bool, Error>;
}

// Keep the old trait names for backward compatibility during transition
/// Async tally control trait (deprecated, use TallyControl instead).
/// Blocking tally control trait (deprecated, use TallyControl instead).
// Async implementation for AsyncCamera
#[cfg(feature = "async")]
impl<P, Tr, Exec> TallyControl for crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
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

// Blocking implementation for BlockingCamera
#[cfg(not(feature = "async"))]
impl<P, Tr> TallyControl for crate::camera::Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
{
    fn tally_red_on(&mut self) -> Result<(), Error> {
        use crate::command::tally::Tally;
        let cmd = Tally::RedOn;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn tally_red_off(&mut self) -> Result<(), Error> {
        use crate::command::tally::Tally;

        let cmd = Tally::RedOff;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn tally_bright_lo(&mut self) -> Result<(), Error> {
        use crate::command::tally::Tally;

        let cmd = Tally::BrightLo;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn tally_bright_hi(&mut self) -> Result<(), Error> {
        use crate::command::tally::Tally;

        let cmd = Tally::BrightHi;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn tally_green_on(&mut self) -> Result<(), Error> {
        use crate::command::tally::Tally;

        let cmd = Tally::GreenOn;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn tally_green_off(&mut self) -> Result<(), Error> {
        use crate::command::tally::Tally;

        let cmd = Tally::GreenOff;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn tally_flash(&mut self) -> Result<(), Error> {
        use crate::command::tally::Tally;

        let cmd = Tally::Flash;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn tally_on(&mut self) -> Result<(), Error> {
        use crate::command::tally::Tally;

        let cmd = Tally::On;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn tally_off(&mut self) -> Result<(), Error> {
        use crate::command::tally::Tally;

        let cmd = Tally::Off;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn get_tally_status(&mut self) -> Result<bool, Error> {
        use crate::command::{response::ViscaResponse, tally::TallyInquiry, InquiryResponse};

        let inquiry = TallyInquiry::Red;
        let response = self.send_command(&inquiry)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TallyRed { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_red_tally_status(&mut self) -> Result<bool, Error> {
        use crate::command::{response::ViscaResponse, tally::TallyInquiry, InquiryResponse};

        let inquiry = TallyInquiry::Red;
        let response = self.send_command(&inquiry)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TallyRed { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_green_tally_status(&mut self) -> Result<bool, Error> {
        use crate::command::{response::ViscaResponse, tally::TallyInquiry, InquiryResponse};

        let inquiry = TallyInquiry::Green;
        let response = self.send_command(&inquiry)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TallyGreen { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}
