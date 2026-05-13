use std::time::Duration;

use grafton_visca::testing::testkit::{helpers, Step};

#[cfg(not(feature = "mode-async"))]
use grafton_visca::testing::testkit::ScriptedBlockingTransport;

#[cfg(feature = "mode-async")]
use grafton_visca::testing::testkit::ScriptedTransport;

fn main() {
    let _ = helpers::ack(1);
    let _ = helpers::complete(1);
    let _ = helpers::standard_command_response(1);

    let step = Step::After {
        delay: Duration::from_millis(5),
        responses: vec![helpers::complete(1)],
    };

    #[cfg(not(feature = "mode-async"))]
    {
        let transport = ScriptedBlockingTransport::new(vec![step]);
        transport.add_response(helpers::complete(1));
        let _: Vec<Vec<u8>> = transport.sent();
    }

    #[cfg(feature = "mode-async")]
    {
        let transport = ScriptedTransport::<()>::new(vec![step]);
        transport.add_response(helpers::complete(1));
        let _: Vec<Vec<u8>> = transport.sent();
    }
}
