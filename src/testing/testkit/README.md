# Virtual owner time

`ManualClock` is the testkit's one virtual clock. It advances explicitly with
`advance`, or automatically once every participant is blocked. Keep all
transports, session deadlines and test helpers on that clock.

For blocking sessions, call `clock.enter()`, keep the returned
`ClockParticipant` on its thread and pass it to `open_blocking_session` or
`open_blocking_camera_session`. The guard must belong to the same clock.
Build the transport with `ScriptedBlockingTransport::new(ScriptedTransportIdentity::Tcp, steps).on_clock(&clock)`.
Session handle waits, including diagnostic timeouts, then use virtual time.
`ManualClock::spawn` returns a clock-aware join handle; a poll panic restores
participation before unwinding continues.

For runtime-free async tests, construct `ManualClockExecutor::local(&clock)`
and call `run_until(future)`. The driver accepts borrowed futures and drives
background tasks on the calling thread. It returns as soon as the supplied
future completes even while periodic tasks remain active.
Use `ManualClockExecutor::new(runtime_executor, &clock)` with a
`TokioExecutor` or `SmolExecutor` to drive the same virtual deadlines on that
runtime's scheduler.

`ScriptedTransport::new(ScriptedTransportIdentity::Tcp, steps).with_executor(executor)` accepts any executor,
including an `Arc<ManualClockExecutor>`. `Step::After` uses that executor's
clock. Without an executor, the default `Immediate` transport delivers delayed
batches immediately. Blocking delayed batches also deliver immediately unless
`on_clock` selected a manual clock.

`Step::OnSend` and `Step::DynamicResponse` consume one send. `Step::EverySend`
persists and answers every subsequent send; the auto-response helpers use it.
Dynamic response functions are shared with `Arc`, so a cloned step has the
same behavior. A signalled shutdown wins over a ready reply on the same poll.

Use real-time executors for tests whose subject is an OS/network timing
contract. Integration scenarios shared across facades or runtimes use the
workspace's `facade_matrix!` and `runtime_matrix!` harnesses.

Choose a `ScriptedTransportIdentity` that matches the camera connection:
`Tcp`, `Udp`, `Serial { bus }`, or a `Custom` identity with explicit send
semantics and addressing. Each stream reply acts as a chunk and can span
several reads. Each UDP reply acts as one datagram.

With `runtime-tokio`, `ViscaCameraSimulator::default()` uses the default timing,
two command sockets per camera and no added loss, busy replies or jitter.
To adjust those settings, pass a `SimulatorConfig` to
`ViscaCameraSimulator::new(config)`. Its setters use `CommandSocketCount` for
the socket count and `UnitInterval` for reply loss and busy probabilities.
