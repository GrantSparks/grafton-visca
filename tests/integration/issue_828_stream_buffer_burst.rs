//! #828: a valid stream burst must never overflow the receive limits.
//!
//! `BufferConfig { recv_buffer_size: 64, max_buffer_size: 64 }` passes
//! validation. The owner reads up to `recv_buffer_size` bytes at a time, so a
//! reply split across reads followed by one full read of valid replies used to
//! exceed the framer's 64-byte bound and poison a perfectly healthy raw stream.
//! The framer now always has room for one full read on top of the retention
//! bound.

#[cfg(feature = "blocking")]
use grafton_visca_test_support::fake_camera;

#[cfg(feature = "blocking")]
use grafton_visca::{
    blocking::Session,
    config::SessionConfig,
    profiles::{ProfileSpec, PtzOpticsG2},
    transport::{AddressingMode, IoBounds, SendSemantics},
};

#[cfg(feature = "blocking")]
use fake_camera::FakeCamera;

/// A raw TCP camera that answers the first write with a reply split across a
/// short read and one full `recv_buffer_size` read.
#[cfg(feature = "blocking")]
fn burst_camera(recv_buffer_size: usize) -> FakeCamera {
    let mut answered = false;
    FakeCamera::new(move |_, answer| {
        if !std::mem::replace(&mut answered, true) {
            // The ACK arrives split: two bytes now ...
            answer.reply(vec![0x90, 0x41]);
            // ... and its terminator leads one full 64-byte read that also
            // carries the completion and twenty Network Change notices.
            let mut burst = vec![0xff, 0x90, 0x51, 0xff];
            for _ in 0..20 {
                burst.extend_from_slice(&[0x90, 0x38, 0xff]);
            }
            assert_eq!(burst.len(), recv_buffer_size);
            answer.reply(burst);
        }
    })
}

#[cfg(feature = "blocking")]
#[test]
fn a_split_reply_followed_by_a_full_read_keeps_the_stream_running() {
    let config = IoBounds::default()
        .with_buffer_config(
            grafton_visca::transport::BufferConfig::default()
                .with_recv_buffer_size(64)
                .with_max_buffer_size(64),
        )
        .expect("buffer bounds");
    let fake = burst_camera(config.buffer_config().recv_buffer_size());
    let wire = fake
        .blocking_wire()
        .with_config(config)
        .with_semantics(SendSemantics::Stream)
        .with_addressing(AddressingMode::Ip);

    let session = Session::open(
        wire,
        &SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("profile")),
    )
    .expect("owner session");
    let camera = session
        .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .expect("camera view");

    camera
        .power()
        .on()
        .expect("the burst completes the command instead of poisoning the stream");
    session.close().expect("clean close");
}

#[cfg(all(feature = "test-utils", any(feature = "async", feature = "blocking")))]
mod read_timing {
    use std::{
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };

    use grafton_visca::{
        config::{OperationalTuning, SessionConfig},
        observability::MetricsSnapshot,
        profiles::{AnyProfile, GenericVisca, ProfileSpec, SonyFr7},
        raw::InquiryRequest,
        request::{InquiryRetryClass, ResponseDecoder, SubmissionClass},
        testing::testkit::ManualClock,
        transport::{IoBounds, SendSemantics},
        types::IndividualCameraId,
        Error,
    };
    use grafton_visca_test_support::fake_camera::{frames, FakeCamera};

    const DEADLINE: Duration = Duration::from_secs(1);
    const TICK: Duration = Duration::from_nanos(1);

    #[derive(Clone, Copy, Debug)]
    struct Case {
        sony: bool,
        position: usize,
        first_read_at: Duration,
        split: bool,
    }

    fn cases() -> impl Iterator<Item = Case> {
        [false, true].into_iter().flat_map(|sony| {
            [1, 64, 65, 130]
                .into_iter()
                .flat_map(move |position| {
                    [DEADLINE - TICK, DEADLINE, DEADLINE + TICK]
                        .into_iter()
                        .map(move |first_read_at| Case {
                            sony,
                            position,
                            first_read_at,
                            split: false,
                        })
                })
                .chain([
                    Case {
                        sony,
                        position: 65,
                        first_read_at: DEADLINE - TICK,
                        split: true,
                    },
                    // 64 full batches force a boundary-first selection
                    // before the inquiry reply can be decoded.
                    Case {
                        sony,
                        position: 64 * 64 + 1,
                        first_read_at: DEADLINE,
                        split: false,
                    },
                ])
        })
    }

    impl Case {
        fn config(self) -> SessionConfig {
            let profile = if self.sony {
                ProfileSpec::from_compile_time::<SonyFr7>()
            } else {
                ProfileSpec::from_compile_time::<GenericVisca>()
            };
            SessionConfig::new(profile.expect("profile"))
                .with_tuning(OperationalTuning::new().with_inquiry_timeout(DEADLINE))
        }

        fn start(self, clock: &ManualClock) -> ReadSchedule {
            let sent_at = Arc::new(Mutex::new(None));
            let stamp = Arc::clone(&sent_at);
            let send_clock = clock.clone();
            let fake = FakeCamera::new(move |write, answer| {
                if write.starts_with(&[0x02, 0x00]) {
                    return; // Sony sequence reset does not produce VISCA input.
                }
                *stamp.lock().unwrap() = Some(send_clock.now());
                let mut bytes = Vec::new();
                let sequence = frames::sony_sequence(write);
                let unrelated = sequence.map_or_else(
                    || frames::ack(1),
                    |seq| frames::sony_reply(seq.wrapping_add(1), &frames::ack(1)),
                );
                for _ in 1..self.position {
                    bytes.extend_from_slice(&unrelated);
                }
                let reply = frames::inquiry_reply(&[2]);
                bytes.extend(
                    sequence.map_or_else(|| reply.clone(), |seq| frames::sony_reply(seq, &reply)),
                );
                if self.split {
                    let tail = bytes.pop().unwrap();
                    answer.reply(bytes).reply(vec![tail]);
                } else {
                    answer.reply(bytes);
                }
            });
            ReadSchedule {
                case: self,
                clock: clock.clone(),
                fake,
                sent_at,
            }
        }

        fn check(self, result: Result<u8, Error>, metrics: MetricsSnapshot, fake: &FakeCamera) {
            let late = self.split || self.first_read_at > DEADLINE;
            if late {
                assert!(
                    matches!(&result, Err(Error::InquiryFailed { source, .. })
                        if matches!(source.as_ref(), Error::Timeout { .. })),
                    "{self:?}: {result:?}"
                );
            } else {
                assert_eq!(
                    result.unwrap_or_else(|error| panic!("{self:?}: {error:?}")),
                    2
                );
            }
            assert_eq!(metrics.inquiry_timeouts, u64::from(late), "{self:?}");
            assert_eq!(metrics.received_frames, self.position as u64, "{self:?}");
            assert_eq!(fake.read_count(), 1 + usize::from(self.split), "{self:?}");
        }
    }

    #[derive(Clone, Debug)]
    struct ReadSchedule {
        case: Case,
        clock: ManualClock,
        fake: FakeCamera,
        sent_at: Arc<Mutex<Option<Instant>>>,
    }

    impl ReadSchedule {
        fn on_read(&self, read: usize) {
            let write_at = self.sent_at.lock().unwrap().expect("inquiry was sent");
            let arrival = write_at
                + if read == 1 {
                    self.case.first_read_at
                } else {
                    DEADLINE + TICK
                };
            self.clock
                .advance(arrival.saturating_duration_since(self.clock.now()));
        }
    }

    fn io_bounds() -> IoBounds {
        IoBounds::default()
            .with_buffer_config(
                grafton_visca::transport::BufferConfig::default()
                    .with_recv_buffer_size(48 * 1024)
                    .with_max_buffer_size(48 * 1024),
            )
            .expect("one transport read fits the entire burst")
    }

    fn inquiry() -> InquiryRequest<u8> {
        InquiryRequest::new(
            [0x81, 0x09, 0x01, 0xff],
            ResponseDecoder::from_fn(|payload| Ok(payload[0])),
            InquiryRetryClass::Never,
            SubmissionClass::Normal,
        )
        .expect("inquiry")
    }

    #[cfg(feature = "async")]
    mod async_facade {
        use std::{
            future::Future,
            sync::atomic::{AtomicUsize, Ordering},
        };

        use grafton_visca::{
            r#async::Session,
            runtime::Executor,
            testing::testkit::{ManualClockExecutor, ManualClockTimer},
        };

        use super::*;

        /// Advance immediately after returning the physical read's sample.
        /// A later owner batch therefore sees time beyond any exact deadline.
        #[derive(Clone)]
        struct SampleThenAdvance {
            inner: ManualClockExecutor,
            schedule: ReadSchedule,
            sampled_reads: Arc<AtomicUsize>,
        }

        impl Executor for SampleThenAdvance {
            type Timer = ManualClockTimer;

            fn timer(&self, deadline: Instant) -> Self::Timer {
                self.inner.timer(deadline)
            }

            fn spawn_detached<F: Future<Output = ()> + Send + 'static>(&self, future: F) {
                self.inner.spawn_detached(future);
            }

            fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
                self.inner.sleep(duration)
            }

            fn now(&self) -> Instant {
                let read = self.schedule.fake.read_count();
                if self.sampled_reads.swap(read, Ordering::Relaxed) == read {
                    return self.inner.now();
                }
                self.schedule.on_read(read);
                let sampled = self.inner.now();
                self.schedule.clock.advance(TICK * 2);
                sampled
            }
        }

        #[test]
        fn whole_read_uses_the_physical_sample_across_every_batch() {
            for case in cases() {
                let clock = ManualClock::new();
                let schedule = case.start(&clock);
                let executor = SampleThenAdvance {
                    inner: ManualClockExecutor::local(&clock),
                    schedule: schedule.clone(),
                    sampled_reads: Arc::new(AtomicUsize::new(0)),
                };
                executor.inner.run_until(async {
                    let session = Session::open(
                        schedule
                            .fake
                            .async_wire()
                            .with_config(io_bounds())
                            .with_semantics(SendSemantics::Stream),
                        &case.config(),
                        executor.clone(),
                    )
                    .await
                    .expect("stream session");
                    let camera = session
                        .camera::<AnyProfile>(IndividualCameraId::CAMERA_1)
                        .unwrap();
                    let result = camera.inquire(&inquiry()).await;
                    case.check(result, session.metrics().await.unwrap(), &schedule.fake);
                    session.close().await.expect("clean close");
                });
            }
        }
    }

    #[cfg(feature = "blocking")]
    mod blocking_facade {
        use grafton_visca::transport::{
            BlockingTransport, HasTransportConfig, ReceiveOutcome, TransportIdentity,
        };
        use grafton_visca_test_support::fake_camera::BlockingWire;

        use super::*;

        // Local fake: attach scripted arrival times to successful native reads.
        #[derive(Debug)]
        struct TimedRead {
            wire: BlockingWire,
            schedule: ReadSchedule,
        }

        impl HasTransportConfig for TimedRead {
            fn identity(&self) -> TransportIdentity<'_> {
                self.wire.identity()
            }

            fn io_bounds(&self) -> &IoBounds {
                self.wire.io_bounds()
            }
        }

        impl BlockingTransport for TimedRead {
            fn send_with_timeout(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), Error> {
                self.wire.send_with_timeout(bytes, timeout)
            }

            fn recv_into_with_timeout(
                &mut self,
                dst: &mut [u8],
                timeout: Duration,
            ) -> Result<ReceiveOutcome, Error> {
                let reply = self.wire.recv_into_with_timeout(dst, timeout)?;
                self.schedule.on_read(self.schedule.fake.read_count());
                Ok(reply)
            }
        }

        #[test]
        fn complete_and_split_replies_use_their_own_read_time() {
            for case in cases() {
                let clock = ManualClock::new();
                let participant = clock.enter();
                let schedule = case.start(&clock);
                let wire = TimedRead {
                    wire: schedule
                        .fake
                        .blocking_wire()
                        .with_config(io_bounds())
                        .with_semantics(SendSemantics::Stream)
                        .on_clock(&clock),
                    schedule: schedule.clone(),
                };
                let session = clock
                    .open_blocking_session(&participant, wire, case.config())
                    .expect("stream session");
                let camera = session
                    .camera::<AnyProfile>(IndividualCameraId::CAMERA_1)
                    .unwrap();
                let result = camera.inquire(&inquiry());
                case.check(result, session.metrics().unwrap(), &schedule.fake);
                session.close().expect("clean close");
            }
        }
    }
}
