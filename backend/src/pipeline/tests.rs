use super::*;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

const DEADLINE: Duration = Duration::from_secs(3);

struct Echo;

impl AudioProcessor for Echo {
    type Output = f32;

    fn process(
        &mut self,
        audio: &Message<Box<[f32]>>,
        _: &Cancellation,
    ) -> Result<Option<f32>, PipelineError> {
        Ok(Some(audio.value[0]))
    }
}

struct FinishMarker;

impl AudioProcessor for FinishMarker {
    type Output = f32;

    fn process(
        &mut self,
        audio: &Message<Box<[f32]>>,
        _: &Cancellation,
    ) -> Result<Option<f32>, PipelineError> {
        Ok(Some(audio.value[0]))
    }

    fn finish(&mut self, _: &Cancellation) -> Result<Option<f32>, PipelineError> {
        Ok(Some(99.0))
    }
}

fn next_result(pipeline: &Pipeline<f32>) -> Message<f32> {
    let deadline = Instant::now() + DEADLINE;
    loop {
        if let Some(result) = pipeline.try_result().unwrap() {
            return result;
        }
        assert!(Instant::now() < deadline, "worker did not produce a result");
        thread::yield_now();
    }
}

struct Gated {
    entered: mpsc::Sender<(u64, bool)>,
    release: Option<mpsc::Receiver<()>>,
    dropped: Arc<AtomicBool>,
}

impl AudioProcessor for Gated {
    type Output = f32;

    fn process(
        &mut self,
        audio: &Message<Box<[f32]>>,
        _: &Cancellation,
    ) -> Result<Option<f32>, PipelineError> {
        self.entered
            .send((audio.sequence, audio.discontinuity))
            .unwrap();
        if let Some(release) = self.release.take() {
            release.recv_timeout(DEADLINE).unwrap();
        }
        Ok(Some(audio.value[0]))
    }
}

impl Drop for Gated {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

struct GateFixture {
    processor: Gated,
    entered: mpsc::Receiver<(u64, bool)>,
    release: mpsc::Sender<()>,
    dropped: Arc<AtomicBool>,
}

fn gated() -> GateFixture {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let dropped = Arc::new(AtomicBool::new(false));
    GateFixture {
        processor: Gated {
            entered: entered_tx,
            release: Some(release_rx),
            dropped: Arc::clone(&dropped),
        },
        entered: entered_rx,
        release: release_tx,
        dropped,
    }
}

#[test]
fn slow_processor_does_not_block_producer_and_oldest_audio_is_discarded() {
    let GateFixture {
        processor,
        entered,
        release,
        ..
    } = gated();
    let mut pipeline = Pipeline::default();
    let id = pipeline.start(processor).unwrap();
    pipeline.submit(id, &[1.0], false).unwrap();
    assert_eq!(entered.recv_timeout(DEADLINE).unwrap(), (1, false));
    // The worker is held at a barrier. These submissions must finish before release.
    for value in 2..=21 {
        pipeline.submit(id, &[value as f32], false).unwrap();
    }
    release.send(()).unwrap();
    for sequence in 14..=21 {
        assert_eq!(
            entered.recv_timeout(DEADLINE).unwrap(),
            (sequence, sequence == 14)
        );
    }
    pipeline.stop().unwrap();
}

#[test]
fn stop_discards_in_flight_output_and_restart_rejects_old_session() {
    let GateFixture {
        processor,
        entered,
        release,
        dropped,
    } = gated();
    let mut pipeline = Pipeline::default();
    let old = pipeline.start(processor).unwrap();
    pipeline.submit(old, &[1.0], false).unwrap();
    entered.recv_timeout(DEADLINE).unwrap();
    pipeline.request_stop();
    assert!(pipeline.try_result().unwrap().is_none());
    assert_eq!(
        pipeline.submit(old, &[2.0], false),
        Err(PipelineError::Stopped)
    );
    release.send(()).unwrap();
    pipeline.stop().unwrap();
    pipeline.stop().unwrap();
    assert!(dropped.load(Ordering::Acquire));
    let new = pipeline.start(Echo).unwrap();
    assert!(new > old);
    assert_eq!(
        pipeline.submit(old, &[3.0], false),
        Err(PipelineError::StaleSession)
    );
    pipeline.submit(new, &[4.0], false).unwrap();
    let output = next_result(&pipeline);
    assert_eq!(
        (output.session_id, output.sequence, output.value),
        (new, 1, 4.0)
    );
    pipeline.stop().unwrap();
}

#[test]
fn finish_drains_queued_audio_and_returns_processor_final_output() {
    let mut pipeline = Pipeline::default();
    let session_id = pipeline.start(FinishMarker).unwrap();
    pipeline.submit(session_id, &[1.0], false).unwrap();
    pipeline.submit(session_id, &[2.0], false).unwrap();

    let results = pipeline.finish().unwrap();
    assert_eq!(
        results
            .iter()
            .map(|message| message.value)
            .collect::<Vec<_>>(),
        vec![1.0, 2.0, 99.0]
    );
    assert!(
        results
            .iter()
            .all(|message| message.session_id == session_id)
    );
}

#[test]
fn output_queue_is_bounded_and_marks_discontinuity_for_slow_readers() {
    let mut pipeline = Pipeline::default();
    let id = pipeline.start(Echo).unwrap();
    // Wait for each output without consuming it, isolating output backpressure.
    for value in 1..=20 {
        pipeline.submit(id, &[value as f32], false).unwrap();
        let deadline = Instant::now() + DEADLINE;
        loop {
            let session = pipeline.session.as_ref().unwrap();
            if session.shared.output.last_sequence() == Some(value) {
                break;
            }
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
    }
    let first = pipeline.try_result().unwrap().unwrap();
    assert_eq!(first.sequence, 13);
    assert!(first.discontinuity);
    for sequence in 14..=20 {
        assert_eq!(pipeline.try_result().unwrap().unwrap().sequence, sequence);
    }
    assert!(pipeline.try_result().unwrap().is_none());
    pipeline.stop().unwrap();
}

#[test]
fn drop_joins_idle_worker_and_async_shutdown_releases_processor() {
    for asynchronous in [false, true] {
        let GateFixture {
            processor, dropped, ..
        } = gated();
        let mut pipeline = Pipeline::default();
        pipeline.start(processor).unwrap();
        if asynchronous {
            tauri::async_runtime::block_on(pipeline.shutdown()).unwrap();
        } else {
            drop(pipeline);
        }
        assert!(dropped.load(Ordering::Acquire));
    }
}

#[test]
fn validates_audio_and_rejects_duplicate_start() {
    let mut pipeline = Pipeline::default();
    let id = pipeline.start(Echo).unwrap();
    assert_eq!(pipeline.start(Echo), Err(PipelineError::AlreadyRunning));
    for samples in [
        &[][..],
        &[f32::NAN],
        &[f32::INFINITY],
        &vec![0.0; MAX_BLOCK_SAMPLES + 1],
    ] {
        assert_eq!(
            pipeline.submit(id, samples, false),
            Err(PipelineError::InvalidAudio)
        );
    }
    pipeline
        .submit(id, &vec![0.0; MAX_BLOCK_SAMPLES], true)
        .unwrap();
    let result = next_result(&pipeline);
    assert_eq!(result.sequence, 1);
    assert!(result.discontinuity);
    pipeline.stop().unwrap();
}

struct Failure {
    panic: bool,
}

impl AudioProcessor for Failure {
    type Output = f32;
    fn process(
        &mut self,
        _: &Message<Box<[f32]>>,
        _: &Cancellation,
    ) -> Result<Option<f32>, PipelineError> {
        assert!(!self.panic, "simulated worker failure");
        Err(PipelineError::ProcessorFailed)
    }
}

#[test]
fn worker_failures_are_reported_and_can_be_restarted() {
    for panic in [false, true] {
        let mut pipeline = Pipeline::default();
        let id = pipeline.start(Failure { panic }).unwrap();
        pipeline.submit(id, &[0.0], false).unwrap();
        let expected = if panic {
            PipelineError::WorkerFailed
        } else {
            PipelineError::ProcessorFailed
        };
        let deadline = Instant::now() + DEADLINE;
        loop {
            if let Err(error) = pipeline.try_result() {
                assert_eq!(error, expected);
                break;
            }
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        assert_eq!(pipeline.stop(), Err(expected));
        assert!(pipeline.start(Echo).unwrap() > id);
        pipeline.stop().unwrap();
    }
}

#[test]
fn session_counter_never_wraps_to_an_old_identity() {
    let mut pipeline = Pipeline::<f32> {
        last_session: u64::MAX,
        session: None,
    };
    assert_eq!(pipeline.start(Echo), Err(PipelineError::CounterExhausted));
}

#[test]
fn closed_queue_rejects_late_writes_and_capacity_one_marks_loss() {
    let queue = Queue::new(1);
    for sequence in 1..=3 {
        assert!(queue.push(Message {
            session_id: 1,
            sequence,
            discontinuity: false,
            value: ()
        }));
    }
    let last = queue.try_recv().unwrap();
    assert_eq!(last.sequence, 3);
    assert!(last.discontinuity);
    queue.close();
    assert!(!queue.push(last));
    assert!(queue.recv().is_none());
}

#[test]
fn discontinuity_survives_blocks_without_output() {
    struct SkipFirst;
    impl AudioProcessor for SkipFirst {
        type Output = f32;
        fn process(
            &mut self,
            audio: &Message<Box<[f32]>>,
            _: &Cancellation,
        ) -> Result<Option<f32>, PipelineError> {
            Ok((audio.sequence > 1).then_some(audio.value[0]))
        }
    }
    let mut pipeline = Pipeline::default();
    let id = pipeline.start(SkipFirst).unwrap();
    pipeline.submit(id, &[0.0], true).unwrap();
    pipeline.submit(id, &[1.0], false).unwrap();
    let result = next_result(&pipeline);
    assert_eq!(result.sequence, 2);
    assert!(result.discontinuity);
    pipeline.stop().unwrap();
}

#[test]
fn processing_receives_cooperative_cancellation_before_join() {
    struct ObserveCancellation(mpsc::Sender<Cancellation>, mpsc::Receiver<()>);
    impl AudioProcessor for ObserveCancellation {
        type Output = f32;
        fn process(
            &mut self,
            _: &Message<Box<[f32]>>,
            cancellation: &Cancellation,
        ) -> Result<Option<f32>, PipelineError> {
            self.0.send(cancellation.clone()).unwrap();
            self.1.recv_timeout(DEADLINE).unwrap();
            assert!(cancellation.is_cancelled());
            Ok(Some(1.0))
        }
    }
    let (token_tx, token_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut pipeline = Pipeline::default();
    let id = pipeline
        .start(ObserveCancellation(token_tx, release_rx))
        .unwrap();
    pipeline.submit(id, &[0.0], false).unwrap();
    let token = token_rx.recv_timeout(DEADLINE).unwrap();
    assert!(!token.is_cancelled());
    pipeline.request_stop();
    assert!(token.is_cancelled());
    release_tx.send(()).unwrap();
    pipeline.stop().unwrap();
    assert!(pipeline.try_result().unwrap().is_none());
}
