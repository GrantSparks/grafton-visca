//! #842: real preparation must permit a healthy late-ACK attempt to finish.
use super::*;

#[tokio::test]
async fn async_prepared_retry_budget_covers_a_late_ack_and_completion() {
    for (sony, ack_override, ack_at, completion_at) in [
        (true, 10, 9, Duration::from_millis(16_500)),
        (false, 8, 7, Duration::from_millis(11_500)),
    ] {
        let profile = if sony {
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyFR7>().unwrap()
        } else {
            crate::ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().unwrap()
        };
        let initial = Instant::now();
        let runtime = ManualRuntime::with_polling_sleeps(initial);
        let mut owner_policy = if sony { sony_policy(2) } else { policy(2) };
        owner_policy.targets[crate::target::TargetSlot::CAMERA_1]
            .as_mut()
            .unwrap()
            .cancellation = if profile.supports_command_cancel() {
            CancellationPolicy::Supported
        } else {
            CancellationPolicy::Unsupported
        };
        let (handle, actor) = AsyncOwnerActor::new(owner_policy, runtime.clone()).unwrap();
        let harness = harness();
        let actor_task = tokio::spawn(actor.run(harness.driver));
        let command = crate::raw::AppliedOnly::new(
            [0x81, 0x01, 0x04, 0x07, 0x02, 0xff],
            crate::AffectedAxes::ZOOM,
            crate::TimeoutClass::Quick,
            crate::RetryClass::Standard,
            crate::ControlClass::Normal,
        )
        .unwrap();
        let prepared = crate::prepared::prepare_operation::<completion::AppliedOnly, _>(
            &command,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            crate::OperationalTuning::new().ack_timeout(Duration::from_secs(ack_override)),
            crate::prepared::ClassSelection::Request,
        )
        .unwrap();
        let mut operation = handle.submit_operation(prepared).await.unwrap();
        harness.started.recv_async().await.unwrap();
        harness
            .gates
            .send_async(Ok(TransmissionMeta {
                sequence: sony.then_some(1),
            }))
            .await
            .unwrap();
        assert_eq!(handle.snapshot().await.unwrap().active, 1);

        runtime.advance(Duration::from_secs(ack_at));
        let mut ack = ack(ViscaSocket::S1);
        if sony {
            ack.sequence = Some(EnvelopeSequence {
                value: 1,
                width: SequenceWidth::Full32,
            });
        }
        harness.frames.send_async(batch(vec![ack])).await.unwrap();
        assert_eq!(handle.snapshot().await.unwrap().active, 1);
        runtime.advance(completion_at - Duration::from_secs(ack_at));
        let mut completed = completion(ViscaSocket::S1);
        if sony {
            completed.sequence = Some(EnvelopeSequence {
                value: 1,
                width: SequenceWidth::Full32,
            });
        }
        harness
            .frames
            .send_async(batch(vec![completed]))
            .await
            .unwrap();
        operation
            .applied(None)
            .await
            .expect("the full healthy attempt fits its prepared budget");
        assert_eq!(harness.writes.lock().unwrap().len(), 1);
        handle.shutdown().unwrap();
        actor_task.await.unwrap();
    }
}
