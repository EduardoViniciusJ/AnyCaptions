//! Tempo de execução do ditado: atalho global, worker de gravação e eventos.
//!
//! O atalho alimenta a [`DictationMachine`]; cada gravação vive em uma thread
//! própria (microfone → mono → 16 kHz → Nemotron → texto → colagem). O áudio e
//! o texto ficam apenas em memória.

use std::{
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::{
    audio::{
        capture::{CaptureEvent, CaptureSession, WasapiCapture},
        convert::to_mono_f32,
        devices::InputService,
        resample::MonoResampler,
    },
    domain::{CaptionMode, TargetLanguage, TranslationEngine},
    pipeline::{MAX_BLOCK_SAMPLES, Pipeline},
    transcription::session::ProcessorFactory,
    translation::{
        Translator,
        riva::model_tag,
        stage::{resolve_text_source, split_sentences},
    },
};

use super::{
    ActivationMode, DictationSettings, inject,
    machine::{DictationAction, DictationMachine, DictationPhase},
};

pub const DICTATION_EVENT: &str = "dictation-changed";
const ESC_SHORTCUT: &str = "Escape";
const SUBMIT_BLOCK_SAMPLES: usize = 2_048;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictationEventPayload {
    pub phase: &'static str,
    pub text: String,
    pub error: Option<&'static str>,
    pub sound_cue: bool,
    pub activation: ActivationMode,
    pub show_live_text: bool,
    pub max_duration_secs: u16,
}

enum WorkerCommand {
    Finish,
    Discard,
}

struct WorkerHandle {
    commands: mpsc::Sender<WorkerCommand>,
    thread: Option<JoinHandle<()>>,
}

pub struct DictationState {
    factory: Arc<dyn ProcessorFactory>,
    inputs: Arc<InputService>,
    settings: Mutex<DictationSettings>,
    machine: Mutex<DictationMachine>,
    worker: Mutex<Option<WorkerHandle>>,
}

impl DictationState {
    pub fn new(
        factory: Arc<dyn ProcessorFactory>,
        inputs: Arc<InputService>,
        settings: DictationSettings,
    ) -> Self {
        let machine = machine_for(&settings);
        Self {
            factory,
            inputs,
            settings: Mutex::new(settings),
            machine: Mutex::new(machine),
            worker: Mutex::new(None),
        }
    }

    pub fn settings(&self) -> DictationSettings {
        self.settings
            .lock()
            .map(|settings| settings.clone())
            .unwrap_or_default()
    }

    pub fn available(&self) -> bool {
        self.factory.is_available()
    }

    pub fn inputs(&self) -> &Arc<InputService> {
        &self.inputs
    }

    pub fn phase_name(&self) -> &'static str {
        let phase = self
            .machine
            .lock()
            .map(|machine| machine.phase())
            .unwrap_or(DictationPhase::Idle);
        phase_name(phase)
    }
}

fn machine_for(settings: &DictationSettings) -> DictationMachine {
    let max_duration = settings
        .limit_duration
        .then(|| Duration::from_secs(u64::from(settings.max_duration_secs)));
    DictationMachine::new(settings.activation, max_duration)
}

fn phase_name(phase: DictationPhase) -> &'static str {
    match phase {
        DictationPhase::Idle => "idle",
        DictationPhase::Recording => "recording",
        DictationPhase::Finalizing => "finalizing",
    }
}

fn emit<R: Runtime>(
    app: &AppHandle<R>,
    phase: &'static str,
    text: String,
    error: Option<&'static str>,
) {
    let settings = app.state::<DictationState>().settings();
    let payload = DictationEventPayload {
        phase,
        text,
        error,
        sound_cue: settings.sound_cue,
        activation: settings.activation,
        show_live_text: settings.show_live_text,
        max_duration_secs: settings.max_duration_secs,
    };
    let _ = app.emit_to("overlay", DICTATION_EVENT, payload);
}

/// Registra o atalho do ditado. Devolve `false` quando o sistema o recusa.
pub fn register_hotkey<R: Runtime>(app: &AppHandle<R>, shortcut: &str) -> bool {
    app.global_shortcut()
        .on_shortcut(shortcut, |app, _, event| {
            handle_hotkey(app, event.state == ShortcutState::Pressed);
        })
        .is_ok()
}

pub fn unregister_hotkey<R: Runtime>(app: &AppHandle<R>, shortcut: &str) {
    let _ = app.global_shortcut().unregister(shortcut);
}

/// Chamado no setup do app: registra o atalho salvo e aquece o modelo.
pub fn setup<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<DictationState>();
    let settings = state.settings();
    if settings.enabled {
        let _ = register_hotkey(app, &settings.shortcut);
        warm_up(app, &state);
    }
}

/// Mecanismo de tradução escolhido nas configurações das legendas; o ditado
/// usa o mesmo (e o mesmo tradutor em cache).
fn translation_engine<R: Runtime>(app: &AppHandle<R>) -> TranslationEngine {
    app.try_state::<crate::ipc::Controller>()
        .and_then(|controller| controller.snapshot().ok())
        .map(|snapshot| snapshot.translation_engine)
        .unwrap_or_default()
}

/// Carrega o modelo ASR (e o tradutor, quando o ditado traduz) em segundo
/// plano, para o primeiro ditado não perder a fala inicial durante a carga.
fn warm_up<R: Runtime>(app: &AppHandle<R>, state: &DictationState) {
    let factory = Arc::clone(&state.factory);
    if !factory.is_available() {
        return;
    }
    let engine = translation_engine(app);
    let translates = state.settings().translate_to.is_some();
    let _ = thread::Builder::new()
        .name("dictation-warmup".into())
        .spawn(move || {
            let _ = factory.create(0, CaptionMode::Auto, None);
            if translates {
                let _ = factory.create_translator(engine);
            }
        });
}

fn handle_hotkey<R: Runtime>(app: &AppHandle<R>, pressed: bool) {
    let state = app.state::<DictationState>();
    let Ok(mut machine) = state.machine.lock() else {
        return;
    };
    let action = if pressed {
        machine.key_pressed(Instant::now())
    } else {
        machine.key_released()
    };
    drop(machine);
    match action {
        Some(DictationAction::Start { session }) => start_worker(app, session),
        Some(DictationAction::Finish { .. }) => send_command(&state, WorkerCommand::Finish),
        Some(DictationAction::Discard { .. }) => send_command(&state, WorkerCommand::Discard),
        None => {}
    }
}

fn cancel_recording<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<DictationState>();
    let action = state
        .machine
        .lock()
        .ok()
        .and_then(|mut machine| machine.cancel());
    if action.is_some() {
        send_command(&state, WorkerCommand::Discard);
    }
}

fn send_command(state: &DictationState, command: WorkerCommand) {
    if let Ok(worker) = state.worker.lock()
        && let Some(handle) = worker.as_ref()
    {
        let _ = handle.commands.send(command);
    }
}

fn start_worker<R: Runtime>(app: &AppHandle<R>, session: u64) {
    let state = app.state::<DictationState>();
    // A gravação anterior já terminou (a máquina só inicia do ocioso); o join
    // garante que a thread antiga liberou microfone e stream.
    join_worker(&state);
    let (commands_tx, commands_rx) = mpsc::channel();
    let worker_app = app.clone();
    emit(app, "recording", String::new(), None);
    // Esc global só existe enquanto a gravação está viva.
    let _ = app
        .global_shortcut()
        .on_shortcut(ESC_SHORTCUT, |app, _, event| {
            if event.state == ShortcutState::Pressed {
                cancel_recording(app);
            }
        });
    let thread = thread::Builder::new()
        .name("dictation-worker".into())
        .spawn(move || run_worker(worker_app, session, commands_rx));
    match thread {
        Ok(thread) => {
            if let Ok(mut worker) = state.worker.lock() {
                *worker = Some(WorkerHandle {
                    commands: commands_tx,
                    thread: Some(thread),
                });
            }
        }
        Err(_) => fail(app, session, "dictation-failed"),
    }
}

fn join_worker(state: &DictationState) {
    let handle = state
        .worker
        .lock()
        .ok()
        .and_then(|mut worker| worker.take());
    if let Some(mut handle) = handle {
        let _ = handle.commands.send(WorkerCommand::Discard);
        if let Some(thread) = handle.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Encerra qualquer gravação ativa e aguarda a thread (troca de configurações
/// e saída do app).
pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<DictationState>();
    if let Ok(mut machine) = state.machine.lock() {
        let _ = machine.cancel();
    }
    join_worker(&state);
}

/// Aplica configurações novas: para a gravação, troca o atalho registrado,
/// reconstrói a máquina e aquece o modelo quando o ditado está habilitado.
pub fn apply_settings<R: Runtime>(
    app: &AppHandle<R>,
    settings: DictationSettings,
) -> Result<(), super::DictationError> {
    let state = app.state::<DictationState>();
    shutdown(app);
    let previous = state.settings();
    if previous.enabled && previous.shortcut != settings.shortcut {
        unregister_hotkey(app, &previous.shortcut);
    }
    if settings.enabled {
        let changed = !previous.enabled || previous.shortcut != settings.shortcut;
        if changed && !register_hotkey(app, &settings.shortcut) {
            // Devolve o atalho anterior para o ditado não ficar sem controle.
            if previous.enabled {
                let _ = register_hotkey(app, &previous.shortcut);
            }
            return Err(super::DictationError::InvalidShortcut);
        }
    } else if previous.enabled {
        unregister_hotkey(app, &previous.shortcut);
    }
    if let Ok(mut machine) = state.machine.lock() {
        *machine = machine_for(&settings);
    }
    if let Ok(mut stored) = state.settings.lock() {
        *stored = settings.clone();
    }
    if settings.enabled {
        warm_up(app, &state);
    }
    emit(app, "idle", String::new(), None);
    Ok(())
}

/// Conclusão de qualquer caminho do worker: devolve a máquina ao ocioso,
/// remove o Esc global e avisa a interface.
fn complete<R: Runtime>(app: &AppHandle<R>, session: u64, error: Option<&'static str>) {
    let state = app.state::<DictationState>();
    if let Ok(mut machine) = state.machine.lock() {
        let _ = machine.finished(session);
    }
    let _ = app.global_shortcut().unregister(ESC_SHORTCUT);
    emit(app, "idle", String::new(), error);
}

fn fail<R: Runtime>(app: &AppHandle<R>, session: u64, code: &'static str) {
    let state = app.state::<DictationState>();
    if let Ok(mut machine) = state.machine.lock() {
        let _ = machine.cancel();
    }
    complete(app, session, Some(code));
}

enum Outcome {
    Finish,
    Discard,
    Error(&'static str),
}

fn run_worker<R: Runtime>(
    app: AppHandle<R>,
    session: u64,
    commands: mpsc::Receiver<WorkerCommand>,
) {
    let state = app.state::<DictationState>();
    let settings = state.settings();
    let device_id = settings.input_id.clone().or_else(|| {
        state.inputs.selection().ok().and_then(|selection| {
            selection
                .selected_id
                .filter(|_| selection.selected_available)
        })
    });
    let Some(device_id) = device_id else {
        fail(&app, session, "dictation-capture");
        return;
    };
    // O id da sessão da máquina (u64) identifica o trabalho; o processador só
    // usa o número para etiquetar atualizações que este worker mesmo consome.
    let processor = match state
        .factory
        .create_dictation(session as u32, settings.language.as_deref())
    {
        Ok(processor) => processor,
        Err(_) => {
            fail(&app, session, "dictation-model");
            return;
        }
    };
    let mut pipeline = Pipeline::default();
    let Ok(pipeline_id) = pipeline.start(processor) else {
        fail(&app, session, "dictation-model");
        return;
    };
    let mut capture = match CaptureSession::start(Arc::new(WasapiCapture), device_id) {
        Ok(capture) => capture,
        Err(_) => {
            pipeline.request_stop();
            let _ = pipeline.stop();
            fail(&app, session, "dictation-capture");
            return;
        }
    };
    let format = capture.format();
    let Ok(mut resampler) = MonoResampler::new(format.sample_rate) else {
        let _ = capture.stop();
        pipeline.request_stop();
        let _ = pipeline.stop();
        fail(&app, session, "dictation-capture");
        return;
    };

    let mut mono_buf = Vec::new();
    let mut resampled_buf = Vec::new();
    let mut pending_samples = Vec::with_capacity(MAX_BLOCK_SAMPLES);
    let mut pending_discontinuity = false;
    let mut finals: Vec<String> = Vec::new();
    let mut partial = String::new();

    let outcome = 'main: loop {
        match commands.try_recv() {
            Ok(WorkerCommand::Finish) => break Outcome::Finish,
            Ok(WorkerCommand::Discard) | Err(mpsc::TryRecvError::Disconnected) => {
                break Outcome::Discard;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        // O teto de duração é decidido pela máquina (fonte única de verdade).
        let timed_out = state
            .machine
            .lock()
            .ok()
            .and_then(|mut machine| machine.tick(Instant::now()));
        if matches!(timed_out, Some(DictationAction::Finish { .. })) {
            break Outcome::Finish;
        }
        match capture.recv_timeout(Duration::from_millis(32)) {
            Ok(CaptureEvent::Packet(packet)) => {
                if to_mono_f32(&packet.bytes, format, packet.silent, &mut mono_buf).is_err() {
                    break Outcome::Error("dictation-capture");
                }
                if packet.discontinuity {
                    resampler.reset();
                    pending_samples.clear();
                    pending_discontinuity = true;
                }
                resampled_buf.clear();
                if resampler.push(&mono_buf, &mut resampled_buf).is_err() {
                    break Outcome::Error("dictation-capture");
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
                break Outcome::Error("dictation-capture");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        loop {
            match pipeline.try_result() {
                Ok(Some(message)) => {
                    let mut changed = false;
                    for update in message.value {
                        apply_update(update, &mut finals, &mut partial);
                        changed = true;
                    }
                    if changed {
                        emit(&app, "recording", compose(&finals, &partial), None);
                    }
                }
                Ok(None) => break,
                Err(_) => break 'main Outcome::Error("dictation-model"),
            }
        }
    };

    let _ = capture.stop();
    match outcome {
        Outcome::Finish => {
            if !pending_samples.is_empty() {
                let _ = pipeline.submit(pipeline_id, &pending_samples, pending_discontinuity);
            }
            emit(&app, "finalizing", compose(&finals, &partial), None);
            match pipeline.finish() {
                Ok(messages) => {
                    for message in messages {
                        for update in message.value {
                            apply_update(update, &mut finals, &mut partial);
                        }
                    }
                    let text = compose(&finals, &partial);
                    let error = if text.is_empty() {
                        None
                    } else {
                        match prepare_output(&state, &settings, translation_engine(&app), &text) {
                            Ok(output) if output.is_empty() => None,
                            Ok(output) => inject::paste_into_focused_app(&output)
                                .err()
                                .map(|_| "dictation-paste"),
                            Err(()) => {
                                // Sem fallback silencioso: nada é colado, mas o
                                // texto original fica no clipboard para uso manual.
                                let _ = inject::copy_to_clipboard(&text);
                                Some("dictation-translate")
                            }
                        }
                    };
                    complete(&app, session, error);
                }
                Err(_) => complete(&app, session, Some("dictation-model")),
            }
        }
        Outcome::Discard => {
            pipeline.request_stop();
            let _ = pipeline.stop();
            complete(&app, session, None);
        }
        Outcome::Error(code) => {
            pipeline.request_stop();
            let _ = pipeline.stop();
            fail(&app, session, code);
        }
    }
}

/// Decide se o ditado precisa de tradução: devolve a origem e o alvo, ou
/// `None` para colar como falado (sem alvo, ou origem já no idioma do alvo).
/// Origem: o idioma fixo do ditado quando definido; senão, detecção local no
/// texto final (janela única — o ditado não tem memória de sessão).
fn translation_plan(settings: &DictationSettings, text: &str) -> Option<(String, TargetLanguage)> {
    let target = settings.translate_to?;
    let source = match settings.language.as_deref() {
        Some(tag) => tag.to_owned(),
        None => resolve_text_source(text).to_owned(),
    };
    if model_tag(&source).as_deref() == Some(target.tag()) {
        return None;
    }
    Some((source, target))
}

/// Traduz o texto final sentença a sentença (sentenças curtas se agrupam, como
/// nas legendas) e junta o resultado. Qualquer falha devolve `Err`.
fn translate_text(
    translator: &mut dyn Translator,
    text: &str,
    source: &str,
    target: TargetLanguage,
) -> Result<String, ()> {
    let mut parts: Vec<String> = Vec::new();
    for sentence in split_sentences(text) {
        let translated = translator
            .translate(&sentence, source, target.tag())
            .map_err(|_| ())?;
        if !translated.is_empty() {
            parts.push(translated);
        }
    }
    Ok(parts.join(" "))
}

/// Texto que vai para a colagem: o original, ou a tradução quando configurada.
fn prepare_output(
    state: &DictationState,
    settings: &DictationSettings,
    engine: TranslationEngine,
    text: &str,
) -> Result<String, ()> {
    let Some((source, target)) = translation_plan(settings, text) else {
        return Ok(text.to_owned());
    };
    let mut translator = state.factory.create_translator(engine).map_err(|_| ())?;
    let start = std::time::Instant::now();
    let output = translate_text(translator.as_mut(), text, &source, target)?;
    // Tempos e idiomas apenas; o texto ditado nunca chega aos logs.
    eprintln!(
        "[dictation] translate_ms={} source={source} target={}",
        start.elapsed().as_millis(),
        target.tag()
    );
    Ok(output)
}

fn apply_update(update: crate::ipc::CaptionUpdate, finals: &mut Vec<String>, partial: &mut String) {
    if update.is_final {
        finals.push(update.text);
        partial.clear();
    } else {
        *partial = update.text;
    }
}

fn compose(finals: &[String], partial: &str) -> String {
    let mut text = finals.join(" ");
    if !partial.is_empty() {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(partial);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translation::{MockTranslator, TranslatorError};

    #[test]
    fn translation_plan_skips_when_disabled_or_already_in_the_target_language() {
        let mut settings = DictationSettings::default();
        let text = "Olá pessoal, hoje vamos conversar sobre o projeto novo.";
        // Sem alvo configurado: cola como falado.
        assert_eq!(translation_plan(&settings, text), None);
        // Origem fixa já no idioma do alvo: nada a traduzir.
        settings.translate_to = Some(crate::domain::PT_BR);
        settings.language = Some("pt-BR".into());
        assert_eq!(translation_plan(&settings, text), None);
        // Origem fixa diferente do alvo: traduz.
        settings.translate_to = Some(crate::domain::EN);
        assert_eq!(
            translation_plan(&settings, text),
            Some(("pt-BR".into(), crate::domain::EN))
        );
        // Sem idioma fixo, a origem vem da detecção local no texto.
        settings.language = None;
        let (source, target) = translation_plan(&settings, text).unwrap();
        assert_eq!(source, "pt-br");
        assert_eq!(target, crate::domain::EN);
        // Texto detectado como inglês com alvo EN: cola como falado.
        settings.translate_to = Some(crate::domain::EN);
        assert_eq!(
            translation_plan(
                &settings,
                "The update will be released next week for everyone."
            ),
            None
        );
    }

    #[test]
    fn dictated_text_translates_sentence_by_sentence_and_failures_bubble_up() {
        let mut translator = MockTranslator::new();
        translator
            .expect_translate()
            .withf(|_, source, target| source == "pt-BR" && target == "en")
            .times(2)
            .returning(|text, _, _| Ok(format!("EN {text}")));
        let text = "Primeira frase longa o bastante para ficar sozinha, certo? \
                    Segunda frase igualmente longa para fechar o pensamento.";
        let output = translate_text(&mut translator, text, "pt-BR", crate::domain::EN).unwrap();
        assert!(output.starts_with("EN Primeira"));
        assert!(output.contains(" EN Segunda"));

        let mut failing = MockTranslator::new();
        failing
            .expect_translate()
            .returning(|_, _, _| Err(TranslatorError::TranslationFailed));
        assert_eq!(
            translate_text(&mut failing, text, "pt-BR", crate::domain::EN),
            Err(())
        );
    }

    #[test]
    fn composed_text_joins_finished_sentences_and_the_live_partial() {
        let finals = vec!["Hello there.".to_owned(), "How are you?".to_owned()];
        assert_eq!(compose(&finals, "I was"), "Hello there. How are you? I was");
        assert_eq!(compose(&finals, ""), "Hello there. How are you?");
        assert_eq!(compose(&[], ""), "");
    }

    #[test]
    fn finals_clear_the_partial_and_partials_replace_each_other() {
        let mut finals = Vec::new();
        let mut partial = String::new();
        let update = |text: &str, is_final| crate::ipc::CaptionUpdate {
            session_id: 1,
            segment_id: 1,
            revision: 1,
            mode: CaptionMode::Auto,
            text: text.into(),
            is_final,
            language: None,
        };
        apply_update(update("Hel", false), &mut finals, &mut partial);
        apply_update(update("Hello", false), &mut finals, &mut partial);
        assert_eq!(partial, "Hello");
        apply_update(update("Hello there.", true), &mut finals, &mut partial);
        assert!(partial.is_empty());
        assert_eq!(finals, ["Hello there."]);
    }
}
