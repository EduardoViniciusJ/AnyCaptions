use std::{
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::Duration,
};

use tauri::{Emitter, Runtime, WebviewWindow};

use crate::{
    audio::{
        capture::{CaptureBackend, CaptureEvent, CaptureSession, WasapiCapture},
        convert::to_mono_f32,
        devices::OutputService,
        resample::MonoResampler,
    },
    domain::{
        AppSnapshot, CaptionMode, CaptureState, DomainError, Resources, SourceLanguage,
        TranslationEngine,
    },
    ipc::{CAPTION_EVENT, CaptionUpdate, CommandError, Controller, StateEvents},
    models::{catalog::NEMOTRON_STREAMING_Q8, models_root},
    pipeline::{AudioProcessor, Cancellation, MAX_BLOCK_SAMPLES, Pipeline},
};

use super::{
    nemotron::{LIBRARY_FILENAME, NativeNemotron, NemotronModel, NemotronPaths},
    streaming::StreamingProcessor,
};
use crate::translation::{
    Translator, TranslatorError,
    opus::{NativeOpusTranslator, OpusPaths},
    riva::{NativeRivaTranslator, RivaPaths},
    stage::{StageEvent, TranslationStage},
};

// Audio is submitted in 128 ms blocks at 16 kHz.
const SUBMIT_BLOCK_SAMPLES: usize = 2_048;
// 32 ms of silence, matching the capture receive timeout, keeps the stream paced in real time.
const IDLE_SILENCE_SAMPLES: usize = 512;

impl NemotronPaths {
    /// Model: the catalog layout under the models root (see `crate::models::models_root`).
    /// Library: the shared NeMo-Speech folder (see `crate::native::nemo_library_dir`).
    pub fn resolve() -> Self {
        Self {
            library_dir: crate::native::nemo_library_dir(),
            model: NEMOTRON_STREAMING_Q8.path_in(&models_root()),
        }
    }

    pub fn transcription_available(&self) -> bool {
        self.library_dir.join(LIBRARY_FILENAME).is_file()
            && NEMOTRON_STREAMING_Q8.verify(&self.model).is_ok()
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
    /// `source_language` optionally pins the spoken language on the stream
    /// (the model's prompt dictionary); `None` keeps auto-detection.
    fn create(
        &self,
        session_id: u32,
        mode: CaptionMode,
        source_language: Option<&str>,
    ) -> Result<Box<dyn AudioProcessor<Output = Vec<CaptionUpdate>>>, DomainError>;
    /// Processador do ditado: mesmo modelo compartilhado das legendas, com o
    /// idioma opcionalmente fixado no stream (condiciona o decodificador e
    /// melhora a precisão quando o usuário sabe em que idioma vai falar).
    fn create_dictation(
        &self,
        session_id: u32,
        language: Option<&str>,
    ) -> Result<Box<dyn AudioProcessor<Output = Vec<CaptionUpdate>>>, DomainError> {
        self.create(session_id, CaptionMode::Auto, language)
    }
    fn translation_available(&self, _engine: TranslationEngine) -> bool {
        false
    }
    fn create_translator(
        &self,
        _engine: TranslationEngine,
    ) -> Result<Box<dyn Translator>, DomainError> {
        Err(DomainError::TranslationUnavailable)
    }
}

pub struct NativeProcessorFactory {
    paths: NemotronPaths,
    riva: RivaPaths,
    opus: OpusPaths,
    // Loaded once on first use and reused by every later session: reloading per
    // session cost seconds on each language switch and accumulated leaked GPU
    // memory in the native library until allocations failed. The ASR cache is
    // keyed by the right-context preset, because that is chosen at model load
    // and the modes use different values.
    asr_models: Mutex<Vec<(Option<i32>, Arc<NemotronModel>)>>,
    opus_translator: Mutex<Option<Arc<Mutex<NativeOpusTranslator>>>>,
    riva_translator: Mutex<Option<Arc<Mutex<NativeRivaTranslator>>>>,
}

impl NativeProcessorFactory {
    pub fn new(paths: NemotronPaths, riva: RivaPaths, opus: OpusPaths) -> Self {
        Self {
            paths,
            riva,
            opus,
            asr_models: Mutex::new(Vec::new()),
            opus_translator: Mutex::new(None),
            riva_translator: Mutex::new(None),
        }
    }

    fn asr_model(
        &self,
        rnnt_right_context: Option<i32>,
    ) -> Result<Arc<NemotronModel>, DomainError> {
        let mut cached = self
            .asr_models
            .lock()
            .map_err(|_| DomainError::TranscriptionUnavailable)?;
        if let Some((_, model)) = cached
            .iter()
            .find(|(context, _)| *context == rnnt_right_context)
        {
            return Ok(Arc::clone(model));
        }
        let model = NemotronModel::load(&self.paths, rnnt_right_context)
            .map_err(|_| DomainError::TranscriptionUnavailable)?;
        cached.push((rnnt_right_context, Arc::clone(&model)));
        Ok(model)
    }

    /// The cached OPUS engine, loaded on first use; `None` when its files are
    /// missing or it fails to load.
    fn opus_translator(&self) -> Option<Arc<Mutex<NativeOpusTranslator>>> {
        if !self.opus.translation_available() {
            return None;
        }
        load_cached(&self.opus_translator, || {
            NativeOpusTranslator::load(&self.opus).ok()
        })
    }

    /// The cached Riva engine, loaded on first use so GPU memory is only taken
    /// when a session actually chose it.
    fn riva_translator(&self) -> Option<Arc<Mutex<NativeRivaTranslator>>> {
        if !self.riva.translation_available() {
            return None;
        }
        load_cached(&self.riva_translator, || {
            NativeRivaTranslator::load(&self.riva).ok()
        })
    }
}

/// Returns the cached engine, loading it on first use. The factory outlives
/// every session, so each engine is loaded at most once per app run.
fn load_cached<T>(
    cache: &Mutex<Option<Arc<Mutex<T>>>>,
    load: impl FnOnce() -> Option<T>,
) -> Option<Arc<Mutex<T>>> {
    let mut cached = cache.lock().ok()?;
    if let Some(loaded) = cached.as_ref() {
        return Some(Arc::clone(loaded));
    }
    let loaded = Arc::new(Mutex::new(load()?));
    *cached = Some(Arc::clone(&loaded));
    Some(loaded)
}

/// A session's handle to one cached engine. Translation is stateless per call
/// and sessions never overlap, so the lock is uncontended.
struct SharedTranslator<T: Translator>(Arc<Mutex<T>>);

impl<T: Translator> Translator for SharedTranslator<T> {
    fn translate(
        &mut self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<String, TranslatorError> {
        self.0
            .lock()
            .map_err(|_| TranslatorError::TranslationFailed)?
            .translate(text, source, target)
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
        source_language: Option<&str>,
    ) -> Result<Box<dyn AudioProcessor<Output = Vec<CaptionUpdate>>>, DomainError> {
        // Auto keeps the recognizer untouched; translate mode closes finals sooner.
        let tuning = if mode.requires_translation() {
            super::tuning::TRANSLATION
        } else {
            super::tuning::TRANSCRIPTION
        };
        let recognizer = NativeNemotron::open(
            self.asr_model(tuning.rnnt_right_context)?,
            source_language,
            tuning,
        )
        .map_err(|_| DomainError::TranscriptionUnavailable)?;
        Ok(Box::new(StreamingProcessor::new(
            session_id, mode, recognizer,
        )))
    }

    fn translation_available(&self, engine: TranslationEngine) -> bool {
        match engine {
            TranslationEngine::Opus => self.opus.translation_available(),
            TranslationEngine::Riva => self.riva.translation_available(),
        }
    }

    /// Only the chosen engine serves the session; an unsupported pair is an
    /// error, never a silent switch to the other engine.
    fn create_translator(
        &self,
        engine: TranslationEngine,
    ) -> Result<Box<dyn Translator>, DomainError> {
        match engine {
            TranslationEngine::Opus => self
                .opus_translator()
                .map(|cached| Box::new(SharedTranslator(cached)) as Box<dyn Translator>),
            TranslationEngine::Riva => self
                .riva_translator()
                .map(|cached| Box::new(SharedTranslator(cached)) as Box<dyn Translator>),
        }
        .ok_or(DomainError::TranslationUnavailable)
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

    /// A mesma fábrica serve as legendas e o ditado: modelos carregados uma
    /// vez por execução (T08) são compartilhados pelos dois recursos.
    pub fn factory(&self) -> Arc<dyn ProcessorFactory> {
        Arc::clone(&self.factory)
    }

    pub fn default_native() -> Self {
        Self::new(
            Arc::new(WasapiCapture),
            Arc::new(NativeProcessorFactory::new(
                NemotronPaths::resolve(),
                RivaPaths::resolve(),
                OpusPaths::resolve(),
            )),
        )
    }

    pub fn probe_resources(
        &self,
        output_service: &OutputService,
        engine: TranslationEngine,
    ) -> Resources {
        let audio_capture = output_service
            .selection()
            .is_ok_and(|selection| selection.selected_id.is_some() && selection.selected_available);
        let transcription = self.factory.is_available();
        Resources {
            audio_capture,
            transcription,
            translation: self.factory.translation_available(engine),
        }
    }

    /// Engine the app opens with: the saved choice, unless its model is
    /// missing while the default engine's is installed (the installer ships
    /// OPUS; Riva only exists after its separate download).
    pub fn startup_engine(&self, saved: TranslationEngine) -> TranslationEngine {
        let fallback = TranslationEngine::default();
        if saved != fallback
            && !self.factory.translation_available(saved)
            && self.factory.translation_available(fallback)
        {
            fallback
        } else {
            saved
        }
    }

    pub fn refresh_resources(
        &self,
        controller: &Controller,
        output_service: &OutputService,
        events: &impl StateEvents,
    ) -> Result<AppSnapshot, CommandError> {
        // Availability follows the engine the user chose in the settings.
        let engine = controller.snapshot()?.translation_engine;
        let resources = self.probe_resources(output_service, engine);
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
        plan: SessionPlan,
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
                    plan,
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

/// Everything the domain decided for one capture session: identity, mode,
/// translation engine and the pinned spoken language (`None` = auto-detect).
#[derive(Debug, Clone, Copy)]
pub struct SessionPlan {
    pub session_id: u32,
    pub mode: CaptionMode,
    pub engine: TranslationEngine,
    pub source_language: Option<SourceLanguage>,
}

/// Where finished captions go: straight to the UI, or through the translation
/// stage first (translate mode shows only the translated text).
enum CaptionRoute {
    Direct,
    Translate(TranslationStage),
}

/// Forwards one caption update along the route. Returns `false` when the
/// translation stage is gone (its worker already failed) and the session must
/// end; a merely overloaded stage drops backlog and keeps going.
fn deliver(
    update: CaptionUpdate,
    route: &CaptionRoute,
    session_id: u32,
    controller: &Controller,
    sink: &dyn SessionSink,
) -> bool {
    if !update.is_final {
        let _ = controller.transition_session(session_id, CaptureState::Processing, sink);
        // Translate mode shows only Portuguese, so the original partials stay off screen.
        if matches!(route, CaptionRoute::Direct) {
            let _ = sink.caption_updated(&update);
        }
        return true;
    }
    match route {
        CaptionRoute::Direct => {
            let _ = sink.caption_updated(&update);
            let _ = controller.transition_session(session_id, CaptureState::Listening, sink);
            true
        }
        CaptionRoute::Translate(stage) => stage.submit(update).is_ok(),
    }
}

fn spawn_translation_route(
    session_id: u32,
    target: crate::domain::TargetLanguage,
    engine: TranslationEngine,
    source_language: Option<SourceLanguage>,
    factory: &Arc<dyn ProcessorFactory>,
    controller: &Controller,
    sink: &Arc<dyn SessionSink>,
) -> Result<CaptionRoute, DomainError> {
    let translator = factory.create_translator(engine)?;
    let stage_controller = controller.clone();
    let stage_sink = Arc::clone(sink);
    let source = source_language.map(SourceLanguage::tag);
    TranslationStage::spawn(translator, target, source, move |event| match event {
        StageEvent::Translated(update) => {
            if !stage_controller.is_session_active(update.session_id) {
                return false;
            }
            let _ = stage_sink.caption_updated(&update);
            let _ = stage_controller.transition_session(
                update.session_id,
                CaptureState::Listening,
                stage_sink.as_ref(),
            );
            true
        }
        StageEvent::Failed => {
            let _ = stage_controller.fail_session(
                session_id,
                DomainError::TranslationUnavailable,
                stage_sink.as_ref(),
            );
            false
        }
    })
    .map(CaptionRoute::Translate)
    .map_err(|_| DomainError::TranslationUnavailable)
}

fn run_coordinator(
    plan: SessionPlan,
    device_id: String,
    capture_backend: Arc<dyn CaptureBackend>,
    factory: Arc<dyn ProcessorFactory>,
    controller: Controller,
    sink: Arc<dyn SessionSink>,
    cancel: Cancellation,
) {
    let SessionPlan {
        session_id,
        mode,
        engine,
        source_language,
    } = plan;
    if cancel.is_cancelled() || !controller.is_session_active(session_id) {
        return;
    }

    // Checked before the ASR model loads, so a missing translator fails fast.
    if mode.requires_translation() && !factory.translation_available(engine) {
        let _ = controller.fail_session(
            session_id,
            DomainError::TranslationUnavailable,
            sink.as_ref(),
        );
        return;
    }

    let processor = match factory.create(session_id, mode, source_language.map(SourceLanguage::tag))
    {
        Ok(proc) => proc,
        Err(error) => {
            let _ = controller.fail_session(session_id, error, sink.as_ref());
            return;
        }
    };

    let route = if let Some(target) = mode.translation_target() {
        match spawn_translation_route(
            session_id,
            target,
            engine,
            source_language,
            &factory,
            &controller,
            &sink,
        ) {
            Ok(route) => route,
            Err(error) => {
                let _ = controller.fail_session(session_id, error, sink.as_ref());
                return;
            }
        }
    } else {
        CaptionRoute::Direct
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
                        if !deliver(update, &route, session_id, &controller, sink.as_ref()) {
                            let _ = controller.fail_session(
                                session_id,
                                DomainError::TranslationUnavailable,
                                sink.as_ref(),
                            );
                            pipeline.request_stop();
                            let _ = capture.stop();
                            let _ = pipeline.stop();
                            return;
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
                'flush: for message in messages {
                    for update in message.value {
                        if !controller.is_session_active(session_id) {
                            break 'flush;
                        }
                        if !deliver(update, &route, session_id, &controller, sink.as_ref()) {
                            let _ = controller.fail_session(
                                session_id,
                                DomainError::TranslationUnavailable,
                                sink.as_ref(),
                            );
                            break 'flush;
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
        // Graceful end: wait for queued translations to reach the screen.
        if let CaptionRoute::Translate(stage) = route {
            stage.finish();
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
            _source_language: Option<&str>,
        ) -> Result<Box<dyn AudioProcessor<Output = Vec<CaptionUpdate>>>, DomainError> {
            // Speech yields a finished utterance; silence yields nothing.
            let mut recognizer = MockStreamingRecognizer::new();
            recognizer.expect_push().returning(|samples| {
                let voiced = samples.iter().any(|sample| sample.abs() > 0.1);
                Ok(voiced
                    .then(|| SpeechResult {
                        text: "Live English caption".into(),
                        is_final: true,
                        language: Some("en-US".into()),
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

    struct OpusOnlyFactory;

    impl ProcessorFactory for OpusOnlyFactory {
        fn is_available(&self) -> bool {
            true
        }

        fn create(
            &self,
            session_id: u32,
            mode: CaptionMode,
            source_language: Option<&str>,
        ) -> Result<Box<dyn AudioProcessor<Output = Vec<CaptionUpdate>>>, DomainError> {
            FakeFactory.create(session_id, mode, source_language)
        }

        fn translation_available(&self, engine: TranslationEngine) -> bool {
            engine == TranslationEngine::Opus
        }
    }

    #[test]
    fn startup_falls_back_to_the_default_engine_only_when_the_saved_one_is_missing() {
        let manager = CaptionSessionManager::new(
            Arc::new(MockCaptureBackend::new()),
            Arc::new(OpusOnlyFactory),
        );
        assert_eq!(
            manager.startup_engine(TranslationEngine::Riva),
            TranslationEngine::Opus
        );
        assert_eq!(
            manager.startup_engine(TranslationEngine::Opus),
            TranslationEngine::Opus
        );

        // FakeFactory reports no translation at all: keep the saved choice so
        // the overlay explains the missing engine instead of lying about OPUS.
        let unavailable =
            CaptionSessionManager::new(Arc::new(MockCaptureBackend::new()), Arc::new(FakeFactory));
        assert_eq!(
            unavailable.startup_engine(TranslationEngine::Riva),
            TranslationEngine::Riva
        );
    }

    fn ready_controller(sink: &RecordedSink) -> Controller {
        controller_with_translation(sink, false)
    }

    fn controller_with_translation(sink: &RecordedSink, translation: bool) -> Controller {
        let controller = Controller::default();
        controller
            .set_resources(
                Resources {
                    audio_capture: true,
                    transcription: true,
                    translation,
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
                SessionPlan {
                    session_id: started.session_id,
                    mode: started.mode,
                    engine: started.translation_engine,
                    source_language: started.source_language,
                },
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

    struct TranslatingFakeFactory;

    impl ProcessorFactory for TranslatingFakeFactory {
        fn is_available(&self) -> bool {
            true
        }

        fn translation_available(&self, _engine: TranslationEngine) -> bool {
            true
        }

        fn create(
            &self,
            session_id: u32,
            mode: CaptionMode,
            source_language: Option<&str>,
        ) -> Result<Box<dyn AudioProcessor<Output = Vec<CaptionUpdate>>>, DomainError> {
            FakeFactory.create(session_id, mode, source_language)
        }

        fn create_translator(
            &self,
            _engine: TranslationEngine,
        ) -> Result<Box<dyn Translator>, DomainError> {
            let mut translator = crate::translation::MockTranslator::new();
            translator
                .expect_translate()
                .withf(|text, source, target| {
                    text == "Live English caption" && source == "en-US" && target == "pt-br"
                })
                .returning(|_, _, _| Ok("Legenda em português".to_owned()));
            Ok(Box::new(translator))
        }
    }

    #[test]
    fn translate_mode_shows_only_translated_finals() {
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
        let controller = controller_with_translation(&sink, true);
        let preferences = crate::preferences::Preferences {
            mode: CaptionMode::Translate(crate::domain::PT_BR),
            ..Default::default()
        };
        controller.restore_preferences(&preferences).unwrap();
        let started = controller.start_session(sink.as_ref()).unwrap();
        assert_eq!(started.mode, CaptionMode::Translate(crate::domain::PT_BR));

        let manager =
            CaptionSessionManager::new(Arc::new(backend), Arc::new(TranslatingFakeFactory));
        manager
            .start_session(
                SessionPlan {
                    session_id: started.session_id,
                    mode: started.mode,
                    engine: started.translation_engine,
                    source_language: started.source_language,
                },
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
        // Only Portuguese reaches the screen; the original English text never does.
        assert!(
            captions
                .iter()
                .all(|update| update.text == "Legenda em português")
        );
        assert!(captions.iter().all(|update| update.is_final));
        assert!(
            captions
                .iter()
                .all(|update| update.language.as_deref() == Some("pt-br"))
        );
        assert_eq!(captions[0].session_id, started.session_id);
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
                SessionPlan {
                    session_id: started.session_id,
                    mode: started.mode,
                    engine: started.translation_engine,
                    source_language: started.source_language,
                },
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
                SessionPlan {
                    session_id: started.session_id,
                    mode: started.mode,
                    engine: started.translation_engine,
                    source_language: started.source_language,
                },
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
