//! Coordination after conversion to mono f32 / 16 kHz.
//!
//! One owner controls the session; producers never wait for processing capacity.
//! Capture/normalization and IPC wiring remain separate from this worker boundary.

mod queue;
#[cfg(test)]
mod tests;

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use queue::Queue;

pub const QUEUE_CAPACITY: usize = 8;
pub const MAX_BLOCK_SAMPLES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineError {
    AlreadyRunning,
    Stopped,
    StaleSession,
    InvalidAudio,
    CounterExhausted,
    ProcessorFailed,
    WorkerFailed,
}

#[derive(Debug)]
pub struct Message<T> {
    pub session_id: u64,
    pub sequence: u64,
    pub discontinuity: bool,
    pub value: T,
}

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
}

/// A processor owns its mutable state on one thread (`Send`, no `Sync` needed).
/// Implementations must bound their output size and return promptly on cancellation.
/// Stateful processors must reset context when `audio.discontinuity` is true.
pub trait AudioProcessor: Send + 'static {
    type Output: Send + 'static;

    fn process(
        &mut self,
        audio: &Message<Box<[f32]>>,
        cancellation: &Cancellation,
    ) -> Result<Option<Self::Output>, PipelineError>;

    /// Produces any buffered output after the input queue has drained.
    fn finish(
        &mut self,
        _cancellation: &Cancellation,
    ) -> Result<Option<Self::Output>, PipelineError> {
        Ok(None)
    }
}

impl<T: Send + 'static> AudioProcessor for Box<dyn AudioProcessor<Output = T>> {
    type Output = T;

    fn process(
        &mut self,
        audio: &Message<Box<[f32]>>,
        cancellation: &Cancellation,
    ) -> Result<Option<Self::Output>, PipelineError> {
        (**self).process(audio, cancellation)
    }

    fn finish(
        &mut self,
        cancellation: &Cancellation,
    ) -> Result<Option<Self::Output>, PipelineError> {
        (**self).finish(cancellation)
    }
}

struct Shared<T> {
    input: Queue<Box<[f32]>>,
    output: Queue<T>,
    cancellation: Cancellation,
    error: Mutex<Option<PipelineError>>,
}

impl<T> Shared<T> {
    fn cancel(&self) {
        self.cancellation.cancel();
        self.input.close();
        self.output.close();
    }

    fn error(&self) -> Option<PipelineError> {
        *self.error.lock().unwrap_or_else(|error| error.into_inner())
    }
}

struct Session<T> {
    id: u64,
    sequence: u64,
    shared: Arc<Shared<T>>,
    worker: Option<JoinHandle<()>>,
}

impl<T> Session<T> {
    fn stop(&mut self) -> Result<(), PipelineError> {
        self.shared.cancel();
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| PipelineError::WorkerFailed)?;
        }
        self.shared.error().map_or(Ok(()), Err)
    }

    fn finish(&mut self) -> Result<Vec<Message<T>>, PipelineError> {
        self.shared.input.finish();
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| PipelineError::WorkerFailed)?;
        }
        self.shared.error().map_or(Ok(()), Err)?;

        let mut results = Vec::new();
        while let Some(message) = self.shared.output.try_recv() {
            results.push(message);
        }
        Ok(results)
    }
}

impl<T> Drop for Session<T> {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub struct Pipeline<T: Send + 'static> {
    last_session: u64,
    session: Option<Session<T>>,
}

impl<T: Send + 'static> Default for Pipeline<T> {
    fn default() -> Self {
        Self {
            last_session: 0,
            session: None,
        }
    }
}

impl<T: Send + 'static> Pipeline<T> {
    /// A new processor is supplied each time so state cannot leak across sessions.
    pub fn start(
        &mut self,
        mut processor: impl AudioProcessor<Output = T>,
    ) -> Result<u64, PipelineError> {
        if self.session.is_some() {
            return Err(PipelineError::AlreadyRunning);
        }
        let id = self
            .last_session
            .checked_add(1)
            .ok_or(PipelineError::CounterExhausted)?;
        let shared = Arc::new(Shared {
            input: Queue::new(QUEUE_CAPACITY),
            output: Queue::new(QUEUE_CAPACITY),
            cancellation: Cancellation::default(),
            error: Mutex::new(None),
        });
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("audio-processing".into())
            .spawn(move || {
                // A panic is reported as a worker failure; queued audio is released.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut discontinuity = false;
                    let mut last_sequence = 0;
                    while let Some(audio) = worker_shared.input.recv() {
                        if worker_shared.cancellation.is_cancelled() {
                            break;
                        }
                        let output = processor.process(&audio, &worker_shared.cancellation)?;
                        discontinuity |= audio.discontinuity;
                        last_sequence = audio.sequence;
                        if worker_shared.cancellation.is_cancelled() {
                            break;
                        }
                        if let Some(value) = output {
                            worker_shared.output.push(Message {
                                session_id: audio.session_id,
                                sequence: audio.sequence,
                                discontinuity,
                                value,
                            });
                            discontinuity = false;
                        }
                    }
                    if !worker_shared.cancellation.is_cancelled()
                        && let Some(value) = processor.finish(&worker_shared.cancellation)?
                    {
                        worker_shared.output.push(Message {
                            session_id: id,
                            sequence: last_sequence.saturating_add(1),
                            discontinuity,
                            value,
                        });
                    }
                    Ok::<_, PipelineError>(())
                }))
                .unwrap_or(Err(PipelineError::WorkerFailed));
                if let Err(error) = result {
                    *worker_shared
                        .error
                        .lock()
                        .unwrap_or_else(|error| error.into_inner()) = Some(error);
                    worker_shared.cancel();
                }
            })
            .map_err(|_| PipelineError::WorkerFailed)?;
        self.last_session = id;
        self.session = Some(Session {
            id,
            sequence: 0,
            shared,
            worker: Some(worker),
        });
        Ok(id)
    }

    /// Samples must already be mono f32 at 16 kHz. At most 128 KiB is queued.
    /// A short queue lock is taken, but neither capacity nor processing is awaited.
    pub fn submit(
        &mut self,
        session_id: u64,
        samples: &[f32],
        discontinuity: bool,
    ) -> Result<(), PipelineError> {
        let session = self.session.as_mut().ok_or(PipelineError::Stopped)?;
        if session_id != session.id {
            return Err(PipelineError::StaleSession);
        }
        if let Some(error) = session.shared.error() {
            return Err(error);
        }
        if samples.is_empty()
            || samples.len() > MAX_BLOCK_SAMPLES
            || samples.iter().any(|sample| !sample.is_finite())
        {
            return Err(PipelineError::InvalidAudio);
        }
        let sequence = session
            .sequence
            .checked_add(1)
            .ok_or(PipelineError::CounterExhausted)?;
        if !session.shared.input.push(Message {
            session_id,
            sequence,
            discontinuity,
            value: samples.into(),
        }) {
            return Err(PipelineError::Stopped);
        }
        session.sequence = sequence;
        Ok(())
    }

    pub fn try_result(&self) -> Result<Option<Message<T>>, PipelineError> {
        let Some(session) = &self.session else {
            return Ok(None);
        };
        if let Some(error) = session.shared.error() {
            return Err(error);
        }
        if session.shared.cancellation.is_cancelled() {
            return Ok(None);
        }
        Ok(session
            .shared
            .output
            .try_recv()
            .filter(|message| message.session_id == session.id))
    }

    /// Invalidates output immediately; `stop` must then join the worker.
    pub fn request_stop(&mut self) {
        if let Some(session) = &self.session {
            session.shared.cancel();
        }
    }

    /// Blocking cleanup for a coordinator thread. Never call on the UI thread.
    pub fn stop(&mut self) -> Result<(), PipelineError> {
        self.session
            .take()
            .map_or(Ok(()), |mut session| session.stop())
    }

    /// Closes input, drains queued audio, and returns final buffered output.
    pub fn finish(&mut self) -> Result<Vec<Message<T>>, PipelineError> {
        self.session
            .take()
            .map_or(Ok(Vec::new()), |mut session| session.finish())
    }

    /// Consumes the owner and joins off the async executor. Even if the caller
    /// drops this future after polling it, the blocking task owns the cleanup.
    pub async fn shutdown(mut self) -> Result<(), PipelineError> {
        self.request_stop();
        tauri::async_runtime::spawn_blocking(move || self.stop())
            .await
            .map_err(|_| PipelineError::WorkerFailed)?
    }
}
