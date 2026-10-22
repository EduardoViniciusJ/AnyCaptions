use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::Duration,
};

use tauri::{Emitter, Runtime, WebviewWindow};

use crate::{
    audio::{
        capture::{CaptureBackend, CaptureEvent, CaptureSession, WasapiLoopback},
        convert::to_mono_f32,
        devices::OutputService,
        resample::MonoResampler,
    },
    domain::{AppSnapshot, CaptionMode, CaptureState, DomainError, Resources},
    ipc::{CAPTION_EVENT, CaptionUpdate, CommandError, Controller, StateEvents},
    pipeline::{AudioProcessor, Cancellation, MAX_BLOCK_SAMPLES, Pipeline},
};

use super::{
    nemotron::{LIBRARY_FILENAME, NativeNemotron, NemotronModelProfile, NemotronPaths},
    streaming::StreamingProcessor,
};

// Development build of NeMo-Speech.cpp (see backend/local-data/nemo-speech).
const DEV_NEMO_LIBRARY_DIR: &str = "local-data/nemo-speech/build-vulkan-asr/bin";
// Audio is submitted in 128 ms blocks at 16 kHz.
const SUBMIT_BLOCK_SAMPLES: usize = 2_048;
// 32 ms of silence, matching the capture receive timeout, keeps the stream paced in real time.
const IDLE_SILENCE_SAMPLES: usize = 512;

impl NemotronPaths {
    /// Model: `ANYCAPTIONS_MODELS_DIR`, `backend/models`, or `models/` next to the executable.
    /// Library: `ANYCAPTIONS_NEMO_DIR`, the local development build, or the executable folder.
    pub fn resolve() -> Self {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf));
        let first_dir = |candidates: Vec<Option<PathBuf>>, fallback: PathBuf| {
            candidates
                .into_iter()
                .flatten()
                .find(|dir| dir.is_dir())
                .unwrap_or(fallback)
        };
        let models = first_dir(
            vec![
                std::env::var_os("ANYCAPTIONS_MODELS_DIR").map(PathBuf::from),
                Some(manifest.join("models")),
                exe_dir.as_ref().map(|dir| dir.join("models")),
            ],
            manifest.join("models"),
        );
        let library_dir = first_dir(
            vec![
                std::env::var_os("ANYCAPTIONS_NEMO_DIR").map(PathBuf::from),
                Some(manifest.join(DEV_NEMO_LIBRARY_DIR)),
                exe_dir.clone(),
            ],
            manifest.join(DEV_NEMO_LIBRARY_DIR),
        );
        Self {
            library_dir,
            model: models.join(NemotronModelProfile::streaming_q8().filename),
        }
    }

    pub fn transcription_available(&self) -> bool {
        self.library_dir.join(LIBRARY_FILENAME).is_file()
            && NemotronModelProfile::streaming_q8()
                .verify(&self.model)
                .is_ok()
    }
}

pub trait SessionSink: StateEvents + Send + Sync + 'static {
    fn caption_updated(&self, update: &CaptionUpdate) -> Result<(), CommandError>;
}

impl<R: Runtime> SessionSink for WebviewWindow<R> {
    fn caption_updated(&self, update: &CaptionUpdate) -> Result<(), CommandError> {
        self.emit_to(self.label(), CAPTION_EVENT, update)
            .map_err(|_| CommandError::EventUnavailable)
    }
}

pub trait ProcessorFactory: Send + Sync {
    fn is_available(&self) -> bool;
    fn create(
        &self,
        session_id: u32,
        mode: CaptionMode,
    ) -> Result<Box<dyn AudioProcessor<Output = Vec<CaptionUpdate>>>, DomainError>;
}

pub struct NativeProcessorFactory {
    paths: NemotronPaths,
}

impl NativeProcessorFactory {
    pub fn new(paths: NemotronPaths) -> Self {
        Self { paths }
    }
}

impl ProcessorFactory for NativeProcessorFactory {
    fn is_available(&self) -> bool {
        self.paths.transcription_available()
    }

    fn create(
        &self,
        session_id: u32,
        mode: CaptionMode,
    ) -> Result<Box<dyn AudioProcessor<Output = Vec<CaptionUpdate>>>, DomainError> {
        if mode.requires_translation() {
            return Err(DomainError::TranslationUnavailable);
        }
        let recognizer =
            NativeNemotron::load(&self.paths).map_err(|_| DomainError::TranscriptionUnavailable)?;
        Ok(Box::new(StreamingProcessor::new(
            session_id, mode, recognizer,
        )))
    }
}

struct ActiveCoordinator {
    cancel: Cancellation,
    worker: Option<JoinHandle<()>>,
}

impl ActiveCoordinator {
    fn cancel_and_join(&mut self) {
        self.cancel.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub struct CaptionSessionManager {
    capture_backend: Arc<dyn CaptureBackend>,
    factory: Arc<dyn ProcessorFactory>,
    start_lock: Mutex<()>,
    active: Mutex<Option<ActiveCoordinator>>,
}

impl CaptionSessionManager {
    pub fn new(
        capture_backend: Arc<dyn CaptureBackend>,
        factory: Arc<dyn ProcessorFactory>,
    ) -> Self {
        Self {
            capture_backend,
            factory,
            start_lock: Mutex::new(()),
            active: Mutex::new(None),
        }
    }

    pub fn default_native() -> Self {
        Self::new(
            Arc::new(WasapiLoopback),
            Arc::new(NativeProcessorFactory::new(NemotronPaths::resolve())),
        )
    }

    pub fn probe_resources(&self, output_service: &OutputService) -> Resources {
        let audio_capture = output_service
            .selection()
            .is_ok_and(|selection| selection.selected_id.is_some() && selection.selected_available);
        let transcription = self.factory.is_available();
        Resources {
            audio_capture,
            transcription,
            translation: false,
        }
    }

    pub fn refresh_resources(
        &self,
        controller: &Controller,
        output_service: &OutputService,
        events: &impl StateEvents,
    ) -> Result<AppSnapshot, CommandError> {
        let resources = self.probe_resources(output_service);
        controller.set_resources(resources, events)
    }

    pub fn stop_session(&self) {
        let Ok(_start_guard) = self.start_lock.lock() else {
            return;
        };
        let active = self.active.lock().ok().and_then(|mut guard| guard.take());
        if let Some(mut coordinator) = active {
            coordinator.cancel_and_join();
        }
    }

    pub fn stop_and_join(&self) {
        let Ok(_start_guard) = self.start_lock.lock() else {
            return;
        };
        self.join_active();
    }

    fn join_active(&self) {
        let active = self.active.lock().ok().and_then(|mut guard| guard.take());
        if let Some(mut coordinator) = active {
            coordinator.cancel_and_join();
        }
    }

    pub fn start_session(
        &self,
        session_id: u32,
        mode: CaptionMode,
        device_id: String,
        controller: Controller,
        sink: Arc<dyn SessionSink>,
    ) -> Result<(), CommandError> {
        let _start_guard = self
            .start_lock
            .lock()
            .map_err(|_| CommandError::StateUnavailable)?;
        // NeMo inference runs in native code and cannot be interrupted safely.
        // Join the cancelled coordinator before loading another model/session.
        self.join_active();
        let cancel = Cancellation::default();
        let worker_cancel = cancel.clone();
        let capture_backend = Arc::clone(&self.capture_backend);
        let factory = Arc::clone(&self.factory);

        let worker = thread::Builder::new()
            .name("caption-coordinator".into())
            .spawn(move || {
                run_coordinator(
                    session_id,
                    mode,
                    device_id,
                    capture_backend,
                    factory,
                    controller,
                    sink,
                    worker_cancel,
                );
            })
            .map_err(|_| CommandError::StateUnavailable)?;

        if let Ok(mut guard) = self.active.lock() {
            *guard = Some(ActiveCoordinator {
                cancel,
                worker: Some(worker),
            });
        }
        Ok(())
    }
}

impl Drop for CaptionSessionManager {
    fn drop(&mut self) {
        self.stop_and_join();
    }
}

#[allow(clippy::too_many_arguments)]
fn run_coordinator(
    session_id: u32,
    mode: CaptionMode,
    device_id: String,
    capture_backend: Arc<dyn CaptureBackend>,
    factory: Arc<dyn ProcessorFactory>,
    controller: Controller,
    sink: Arc<dyn SessionSink>,
    cancel: Cancellation,
) {
    if cancel.is_cancelled() || !controller.is_session_active(session_id) {
        return;
    }

    let processor = match factory.create(session_id, mode) {
        Ok(proc) => proc,
        Err(error) => {
            let _ = controller.fail_session(session_id, error, sink.as_ref());
            return;
        }
    };

    if cancel.is_cancelled() || !controller.is_session_active(session_id) {
        return;
    }

    let mut pipeline = Pipeline::default();
    let Ok(pipeline_id) = pipeline.start(processor) else {
        let _ = controller.fail_session(
            session_id,
            DomainError::TranscriptionUnavailable,
            sink.as_ref(),
        );
        return;
    };

    let mut capture = match CaptureSession::start(capture_backend, device_id) {
        Ok(session) => session,
        Err(_) => {
            let _ = pipeline.stop();
            let _ =
                controller.fail_session(session_id, DomainError::CaptureUnavailable, sink.as_ref());
            return;
        }
    };

    let format = capture.format();
    let mut resampler = match MonoResampler::new(format.sample_rate) {
        Ok(resampler) => resampler,
        Err(_) => {
            let _ = capture.stop();
            let _ = pipeline.stop();
            let _ =
                controller.fail_session(session_id, DomainError::CaptureUnavailable, sink.as_ref());
            return;
        }
    };

    if controller
        .transition_session(session_id, CaptureState::Listening, sink.as_ref())
        .is_err()
        || cancel.is_cancelled()
    {
        pipeline.request_stop();
        let _ = capture.stop();
        let _ = pipeline.stop();
        return;
    }

    let mut mono_buf = Vec::new();
    let mut resampled_buf = Vec::new();
    let mut pending_samples = Vec::with_capacity(MAX_BLOCK_SAMPLES);
    let mut pending_discontinuity = false;

    while !cancel.is_cancelled() && controller.is_session_active(session_id) {
        match capture.recv_timeout(Duration::from_millis(32)) {
            Ok(CaptureEvent::Packet(packet)) => {
                if to_mono_f32(&packet.bytes, format, packet.silent, &mut mono_buf).is_err() {
                    let _ = controller.fail_session(
                        session_id,
                        DomainError::CaptureUnavailable,
                        sink.as_ref(),
                    );
                    break;
                }
                if packet.discontinuity {
                    resampler.reset();
                    pending_samples.clear();
                    pending_discontinuity = true;
                }
                resampled_buf.clear();
                if resampler.push(&mono_buf, &mut resampled_buf).is_err() {
                    let _ = controller.fail_session(
                        session_id,
                        DomainError::CaptureUnavailable,
                        sink.as_ref(),
                    );
                    break;
                }
                pending_samples.extend_from_slice(&resampled_buf);
                while pending_samples.len() >= SUBMIT_BLOCK_SAMPLES {
                    let take = pending_samples.len().min(MAX_BLOCK_SAMPLES);
                    if pipeline
                        .submit(pipeline_id, &pending_samples[..take], pending_discontinuity)
                        .is_err()
                    {
                        break;
                    }
                    pending_samples.drain(..take);
                    pending_discontinuity = false;
                }
            }
            Ok(CaptureEvent::Stopped(_)) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = controller.fail_session(
                    session_id,
                    DomainError::CaptureUnavailable,
                    sink.as_ref(),
                );
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // WASAPI sends nothing while the output is idle; silence lets NeMo end the utterance.
                pending_samples.extend_from_slice(&[0.0_f32; IDLE_SILENCE_SAMPLES]);
                let take = pending_samples.len().min(MAX_BLOCK_SAMPLES);
                let _ =
                    pipeline.submit(pipeline_id, &pending_samples[..take], pending_discontinuity);
                pending_samples.drain(..take);
                pending_discontinuity = false;
            }
        }

        loop {
            if cancel.is_cancelled() || !controller.is_session_active(session_id) {
                break;
            }
            match pipeline.try_result() {
                Ok(Some(message)) => {
                    for update in message.value {
                        if cancel.is_cancelled() || !controller.is_session_active(session_id) {
                            break;
                        }
                        if !update.is_final {
                            let _ = controller.transition_session(
                                session_id,
                                CaptureState::Processing,
                                sink.as_ref(),
                            );
                        }
                        let _ = sink.caption_updated(&update);
                        if update.is_final {
                            let _ = controller.transition_session(
                                session_id,
                                CaptureState::Listening,
                                sink.as_ref(),
                            );
                        }
                    }
                }
                Ok(None) => break,
                Err(_) => {
                    let _ = controller.fail_session(
                        session_id,
                        DomainError::TranscriptionUnavailable,
                        sink.as_ref(),
                    );
                    pipeline.request_stop();
                    let _ = capture.stop();
                    let _ = pipeline.stop();
                    return;
                }
            }
        }
    }

    let _ = capture.stop();
    if controller.is_session_active(session_id) {
        if !pending_samples.is_empty() {
            let _ = pipeline.submit(pipeline_id, &pending_samples, pending_discontinuity);
        }
        match pipeline.finish() {
            Ok(messages) => {
                for message in messages {
                    for update in message.value {
                        if !controller.is_session_active(session_id) {
                            break;
                        }
                        if !update.is_final {
                            let _ = controller.transition_session(
                                session_id,
                                CaptureState::Processing,
                                sink.as_ref(),
                            );
                        }
                        let _ = sink.caption_updated(&update);
                        if update.is_final {
                            let _ = controller.transition_session(
                                session_id,
                                CaptureState::Listening,
                                sink.as_ref(),
                            );
                        }
                    }
                }
            }
            Err(_) => {
                let _ = controller.fail_session(
                    session_id,
                    DomainError::TranscriptionUnavailable,
                    sink.as_ref(),
                );
            }
        }
    } else {
        pipeline.request_stop();
        let _ = pipeline.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        audio::{
            capture::{AudioPacket, CaptureError, MockCaptureBackend},
            convert::{PcmEncoding, PcmFormat},
        },
        transcription::streaming::{MockStreamingRecognizer, SpeechResult},
    };
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::Instant,
    };

    #[derive(Default)]
    struct RecordedSink {
        states: Mutex<Vec<AppSnapshot>>,
        captions: Mutex<Vec<CaptionUpdate>>,
    }

    impl StateEvents for RecordedSink {
        fn state_changed(&self, state: &AppSnapshot) -> Result<(), CommandError> {
            self.states.lock().unwrap().push(state.clone());
            Ok(())
        }
    }

    impl SessionSink for RecordedSink {
        fn caption_updated(&self, update: &CaptionUpdate) -> Result<(), CommandError> {
            self.captions.lock().unwrap().push(update.clone());
            Ok(())
        }
    }

    struct FakeFactory;

    impl ProcessorFactory for FakeFactory {
        fn is_available(&self) -> bool {
            true
        }

        fn create(
            &self,
            session_id: u32,
            mode: CaptionMode,
        ) -> Result<Box<dyn AudioProcessor<Output = Vec<CaptionUpdate>>>, DomainError> {
            // Speech yields a finished utterance; silence yields nothing.
            let mut recognizer = MockStreamingRecognizer::new();
            recognizer.expect_push().returning(|samples| {
                let voiced = samples.iter().any(|sample| sample.abs() > 0.1);
                Ok(voiced
                    .then(|| SpeechResult {
                        text: "Live English caption".into(),
                        is_final: true,
                    })
                    .into_iter()
                    .collect())
            });
            recognizer.expect_finish().returning(|| Ok(Vec::new()));
            Ok(Box::new(StreamingProcessor::new(
                session_id, mode, recognizer,
            )))
        }
    }

    fn ready_controller(sink: &RecordedSink) -> Controller {
        let controller = Controller::default();
        controller
            .set_resources(
                Resources {
                    audio_capture: true,
                    transcription: true,
                    translation: false,
                },
                sink,
            )
            .unwrap();
        controller
    }

    #[test]
    fn live_session_streams_audio_into_captions_and_flushes_on_idle_timeout() {
        let mut backend = MockCaptureBackend::new();
        backend
            .expect_run()
            .times(1)
            .returning(|_, stop, ready, events| {
                ready
                    .send(Ok(PcmFormat {
                        sample_rate: 16_000,
                        channels: 1,
                        encoding: PcmEncoding::Float32,
                    }))
                    .unwrap();
                // One block of voiced audio, then idle so the timeout flushes it.
                let mut bytes = Vec::with_capacity(SUBMIT_BLOCK_SAMPLES * 4);
                for _ in 0..SUBMIT_BLOCK_SAMPLES {
                    bytes.extend_from_slice(&0.25_f32.to_le_bytes());
                }
                let _ = events.try_send(CaptureEvent::Packet(AudioPacket {
                    bytes,
                    silent: false,
                    discontinuity: false,
                }));
                let _ = stop.recv();
                Ok(())
            });

        let sink = Arc::new(RecordedSink::default());
        let controller = ready_controller(&sink);
        let started = controller.start_session(sink.as_ref()).unwrap();
        let manager = CaptionSessionManager::new(Arc::new(backend), Arc::new(FakeFactory));
        manager
            .start_session(
                started.session_id,
                started.mode,
                "speaker-1".into(),
                controller.clone(),
                Arc::clone(&sink) as Arc<dyn SessionSink>,
            )
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(2);
        while sink.captions.lock().unwrap().is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        controller.stop_session(sink.as_ref()).unwrap();
        manager.stop_and_join();

        let captions = sink.captions.lock().unwrap();
        assert!(!captions.is_empty());
        assert_eq!(captions[0].session_id, started.session_id);
        assert_eq!(captions[0].text, "Live English caption");
        assert!(captions[0].is_final);
        assert_eq!(
            controller.snapshot().unwrap().capture,
            CaptureState::Stopped
        );
    }

    #[test]
    fn capture_disconnect_transitions_session_to_error_state() {
        let mut backend = MockCaptureBackend::new();
        backend
            .expect_run()
            .times(1)
            .returning(|_, _, ready, events| {
                ready
                    .send(Ok(PcmFormat {
                        sample_rate: 16_000,
                        channels: 1,
                        encoding: PcmEncoding::Float32,
                    }))
                    .unwrap();
                let _ = events.try_send(CaptureEvent::Stopped(CaptureError::DeviceUnavailable));
                Err(CaptureError::DeviceUnavailable)
            });

        let sink = Arc::new(RecordedSink::default());
        let controller = ready_controller(&sink);
        let started = controller.start_session(sink.as_ref()).unwrap();
        let manager = CaptionSessionManager::new(Arc::new(backend), Arc::new(FakeFactory));
        manager
            .start_session(
                started.session_id,
                started.mode,
                "speaker-1".into(),
                controller.clone(),
                Arc::clone(&sink) as Arc<dyn SessionSink>,
            )
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(2);
        while controller.snapshot().unwrap().capture != CaptureState::Error
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(10));
        }
        manager.stop_and_join();

        let snap = controller.snapshot().unwrap();
        assert_eq!(snap.capture, CaptureState::Error);
        assert_eq!(snap.error, Some(DomainError::CaptureUnavailable));
    }

    #[test]
    fn starting_a_session_joins_the_cancelled_coordinator_first() {
        let capture_started = Arc::new(AtomicBool::new(false));
        let backend_started = Arc::clone(&capture_started);
        let mut backend = MockCaptureBackend::new();
        backend
            .expect_run()
            .times(1)
            .returning(move |_, stop, ready, _| {
                ready
                    .send(Ok(PcmFormat {
                        sample_rate: 16_000,
                        channels: 1,
                        encoding: PcmEncoding::Float32,
                    }))
                    .unwrap();
                backend_started.store(true, Ordering::Release);
                let _ = stop.recv();
                Ok(())
            });

        let sink = Arc::new(RecordedSink::default());
        let controller = ready_controller(&sink);
        let started = controller.start_session(sink.as_ref()).unwrap();
        let manager = CaptionSessionManager::new(Arc::new(backend), Arc::new(FakeFactory));
        let old_worker_finished = Arc::new(AtomicBool::new(false));
        let worker_finished = Arc::clone(&old_worker_finished);
        let worker = thread::spawn(move || {
            thread::sleep(Duration::from_millis(40));
            worker_finished.store(true, Ordering::Release);
        });
        *manager.active.lock().unwrap() = Some(ActiveCoordinator {
            cancel: Cancellation::default(),
            worker: Some(worker),
        });

        manager
            .start_session(
                started.session_id,
                started.mode,
                "speaker-1".into(),
                controller,
                Arc::clone(&sink) as Arc<dyn SessionSink>,
            )
            .unwrap();

        assert!(old_worker_finished.load(Ordering::Acquire));
        let deadline = Instant::now() + Duration::from_secs(2);
        while !capture_started.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(capture_started.load(Ordering::Acquire));
        manager.stop_and_join();
    }
}
