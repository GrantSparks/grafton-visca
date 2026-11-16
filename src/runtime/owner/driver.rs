//! Turn driving shared by the native blocking worker and async actor.
//!
//! The expansion contains no future on the blocking surface. Each shell's
//! `io` supplies only its clock, cooperative handoff, event selection and
//! bounded write; the core calls and write-first ordering live here.

macro_rules! owner_turn_driver {
    (
        async: [$($async:tt)*],
        await: [$($await:tt)*],
        driver: $driver:path $(,)?
    ) => {
        pub(crate) $($async)* fn run<D>(mut self, mut driver: D) -> OwnerSnapshot
        where
            D: $driver,
        {
            while self.core.is_running() {
                if self.core.begin_turn() {
                    self.io.cooperative_handoff() $($await)*;
                }
                // A handoff may let time advance or boundaries arrive. Plan
                // only after it, using this shell's selected clock.
                let now = self.io.now();
                let event = match self.core.plan(now, &mut driver) {
                    TurnPlan::Redeliver(retained) => self.core.redeliver(retained),
                    TurnPlan::Select(selection) => self
                        .io
                        .select_event(&mut self.core, selection, &mut driver)
                        $($await)*,
                };
                let selected_at = self.io.now();
                let Some(ClassifiedEvent {
                    event,
                    selected,
                    suppress_due,
                }) = self.core.classify(event, selected_at)
                else {
                    continue;
                };
                let control_consumed_wake = match &event {
                    OwnerEvent::Control(_) => self.core.control_consumed_wake(self.io.now()),
                    _ => None,
                };
                let outcome = self
                    .handle_event(event, &mut driver, selected_at, suppress_due)
                    $($await)*;
                if !self.core.finish(selected, control_consumed_wake, outcome) {
                    break;
                }
            }
            let snapshot = self.core.conclude();
            // Release the transport before the core's liveness sender. Its
            // Drop drains boundaries that raced conclude before publishing
            // the final teardown barrier (#626).
            drop(driver);
            snapshot
        }

        $($async)* fn handle_event<D>(
            &mut self,
            event: OwnerEvent,
            driver: &mut D,
            selected_at: std::time::Instant,
            suppress_due_for_raw_probe_fault: bool,
        ) -> TurnOutcome
        where
            D: $driver,
        {
            let mut step = self.core.handle(
                event,
                driver,
                selected_at,
                suppress_due_for_raw_probe_fault,
            );
            loop {
                match step {
                    TurnStep::Drive { effects, then } => {
                        self.drive(driver, effects, then.input_turn()) $($await)*;
                        step = self.core.resume(then, driver);
                    }
                    TurnStep::Pace { pause, outcome } => {
                        self.core.pace_receive(pause, self.io.now());
                        return outcome;
                    }
                    TurnStep::Done(outcome) => return outcome,
                }
            }
        }

        $($async)* fn drive<D>(
            &mut self,
            driver: &mut D,
            mut effects: std::collections::VecDeque<Effect>,
            turn: Option<&OwnerInputTurn>,
        )
        where
            D: $driver,
        {
            let write_timeout = self.core.state.facts().write_timeout;
            while let Some(staged) = self.core.next_transmit(&mut effects, self.io.now()) {
                let result = match self.core.state.prepare_write(&staged) {
                    Ok(write) => self.io.write(driver, write, write_timeout) $($await)*,
                    Err(error) => WriteOutcome::NotSent(error),
                };
                // Finish this write and drive its effects before selecting
                // input or staging another write (#860). NotSent proves only
                // this attempt avoided I/O; the engine owns logical certainty.
                self.core.finish_transmit(
                    turn,
                    &staged,
                    result,
                    self.io.now(),
                    &mut effects,
                );
            }
            self.core.recycle_effects(effects);
        }
    };
}

pub(super) use owner_turn_driver;

#[cfg(test)]
mod tests;
