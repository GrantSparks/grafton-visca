//! Test scenario building utilities.
//!
//! Provides high-level abstractions for creating complex test scenarios
//! that simulate real camera behavior patterns.

use std::time::Duration;

/// VISCA command terminator byte.
const VISCA_TERMINATOR: u8 = 0xFF;

use crate::common::{patterns, MockResponse, MockTransport, ResponseBuilder};

/// Builder for creating test scenarios
#[derive(Debug)]
pub struct ScenarioBuilder {
    name: String,
    description: String,
    steps: Vec<ScenarioStep>,
}

/// A single step in a test scenario
#[derive(Debug)]
pub enum ScenarioStep {
    /// Expect a command and queue responses
    ExpectCommand {
        command: Vec<u8>,
        responses: Vec<MockResponse>,
        description: String,
    },

    /// Simulate a delay
    Delay(Duration),

    /// Simulate disconnection
    Disconnect,

    /// Simulate reconnection
    Reconnect,
}

/// A complete test scenario
#[derive(Debug)]
pub struct TestScenario {
    pub name: String,
    pub description: String,
    steps: Vec<ScenarioStep>,
}

impl ScenarioBuilder {
    /// Create a new scenario builder
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: String::new(),
            steps: Vec::new(),
        }
    }

    /// Add a description to the scenario
    pub fn description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    /// Expect a power on command
    pub fn expect_power_on(mut self) -> Self {
        self.steps.push(ScenarioStep::ExpectCommand {
            command: patterns::power::ON.to_vec(),
            responses: vec![
                MockResponse::Immediate(ResponseBuilder::ack(1)),
                MockResponse::Delayed(ResponseBuilder::completion(1), Duration::from_millis(50)),
            ],
            description: "Power on".to_string(),
        });
        self
    }

    /// Expect a power standby command
    pub fn expect_power_standby(mut self) -> Self {
        self.steps.push(ScenarioStep::ExpectCommand {
            command: patterns::power::STANDBY.to_vec(),
            responses: vec![
                MockResponse::Immediate(ResponseBuilder::ack(1)),
                MockResponse::Delayed(ResponseBuilder::completion(1), Duration::from_millis(1000)),
            ],
            description: "Power standby".to_string(),
        });
        self
    }

    /// Expect a pan/tilt home command
    pub fn expect_home(mut self) -> Self {
        self.steps.push(ScenarioStep::ExpectCommand {
            command: patterns::pan_tilt::HOME.to_vec(),
            responses: vec![
                MockResponse::Immediate(ResponseBuilder::ack(1)),
                MockResponse::Delayed(ResponseBuilder::completion(1), Duration::from_millis(3000)),
            ],
            description: "Pan/tilt home".to_string(),
        });
        self
    }

    /// Expect a preset recall with settling time
    pub fn expect_preset_recall(mut self, preset: u8) -> Self {
        self.steps.push(ScenarioStep::ExpectCommand {
            command: vec![0x81, 0x01, 0x04, 0x3F, 0x02, preset,  VISCA_TERMINATOR],
            responses: vec![
                MockResponse::Immediate(ResponseBuilder::ack(1)),
                MockResponse::Delayed(ResponseBuilder::completion(1), Duration::from_millis(2000)),
            ],
            description: format!("Recall preset {preset}"),
        });

        // Add settling delay for cameras that need it
        self.steps
            .push(ScenarioStep::Delay(Duration::from_millis(240)));

        self
    }

    /// Expect a custom command
    pub fn expect_command(mut self, command: &[u8], responses: Vec<MockResponse>) -> Self {
        self.steps.push(ScenarioStep::ExpectCommand {
            command: command.to_vec(),
            responses,
            description: "Custom command".to_string(),
        });
        self
    }

    /// Build the scenario
    pub fn build(self) -> TestScenario {
        TestScenario {
            name: self.name,
            description: self.description,
            steps: self.steps,
        }
    }
}

impl TestScenario {
    /// Apply this scenario to a mock transport
    pub fn apply_to(&self, transport: &mut MockTransport) -> Result<(), String> {
        for step in self.steps.iter() {
            match step {
                ScenarioStep::ExpectCommand {
                    command,
                    responses,
                    description,
                } => {
                    let mut expectation =
                        transport.expect_command(command).described_as(description);

                    for response in responses {
                        expectation = expectation.will_respond(response.clone());
                    }
                    // expectation will be committed when it drops here
                }

                ScenarioStep::Delay(duration) => {
                    std::thread::sleep(*duration);
                }

                ScenarioStep::Disconnect => {
                    transport.disconnect();
                }

                ScenarioStep::Reconnect => {
                    // For MockTransport, we need to access the inner state
                    // This is a limitation - in real code we'd add a reconnect method
                    // For now, we'll skip this functionality
                    eprintln!("Warning: Reconnect not implemented for MockTransport");
                }
            }
        }

        Ok(())
    }
}
