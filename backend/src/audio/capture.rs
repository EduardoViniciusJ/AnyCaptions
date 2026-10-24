use std::sync::{
    Arc, Mutex,
    mpsc::{self, Receiver, Sender, SyncSender},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use wasapi::{DeviceEnumerator, DeviceState, Direction, StreamMode, WasapiError};

use super::convert::PcmFormat;

const PACKET_QUEUE_CAPACITY: usize = 8;
const MAX_PACKET_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureError {
    InvalidDevice,
    DeviceUnavailable,
    UnsupportedFormat,
    StreamFailed,
    WorkerFailed,
    AlreadyRunning,
}

#[derive(Debug)]
pub struct AudioPacket {
    pub bytes: Vec<u8>,
    pub silent: bool,
    pub discontinuity: bool,
}

#[derive(Debug)]
pub enum CaptureEvent {
    Packet(AudioPacket),
    Stopped(CaptureError),
}

#[cfg_attr(test, mockall::automock)]
pub trait CaptureBackend: Send + Sync {
    fn run(
        &self,
        device_id: &str,
        stop: Receiver<()>,
        ready: Sender<Result<PcmFormat, CaptureError>>,
        events: SyncSender<CaptureEvent>,
    ) -> Result<(), CaptureError>;
}

pub struct CaptureSession {
    format: PcmFormat,
    events: Receiver<CaptureEvent>,
    stop: Option<Sender<()>>,
    worker: Option<JoinHandle<Result<(), CaptureError>>>,
}

impl CaptureSession {
    pub fn start(
        backend: Arc<dyn CaptureBackend>,
        device_id: String,
    ) -> Result<Self, CaptureError> {
        if device_id.is_empty() || device_id.len() > 512 {
            return Err(CaptureError::InvalidDevice);
        }
        let (stop_tx, stop_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::sync_channel(PACKET_QUEUE_CAPACITY);
        let worker = thread::Builder::new()
            .name("audio-capture".into())
            .spawn(move || {
                let result = backend.run(&device_id, stop_rx, ready_tx, events_tx.clone());
                if let Err(error) = result {
                    let _ = events_tx.try_send(CaptureEvent::Stopped(error));
                }
                result
            })
            .map_err(|_| CaptureError::WorkerFailed)?;
        let format = match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(format)) => format,
            Ok(Err(error)) => {
                let _ = stop_tx.send(());
                let _ = worker.join();
                return Err(error);
            }
            Err(_) => {
                let _ = stop_tx.send(());
                let _ = worker.join();
                return Err(CaptureError::WorkerFailed);
            }
        };
        Ok(Self {
            format,
            events: events_rx,
            stop: Some(stop_tx),
            worker: Some(worker),
        })
    }

    pub fn format(&self) -> PcmFormat {
        self.format
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<CaptureEvent, mpsc::RecvTimeoutError> {
        self.events.recv_timeout(timeout)
    }

    pub fn try_recv(&self) -> Result<CaptureEvent, mpsc::TryRecvError> {
        self.events.try_recv()
    }

    pub fn stop(&mut self) -> Result<(), CaptureError> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        match self.worker.take() {
            Some(worker) => worker.join().map_err(|_| CaptureError::WorkerFailed)?,
            None => Ok(()),
        }
    }

    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }
}

impl Drop for CaptureSession {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub struct CaptureManager {
    backend: Arc<dyn CaptureBackend>,
    session: Mutex<Option<CaptureSession>>,
}

impl CaptureManager {
    pub fn new(backend: Arc<dyn CaptureBackend>) -> Self {
        Self {
            backend,
            session: Mutex::new(None),
        }
    }

    pub fn start(&self, device_id: String) -> Result<PcmFormat, CaptureError> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| CaptureError::WorkerFailed)?;
        if session
            .as_ref()
            .is_some_and(|current| !current.is_finished())
        {
            return Err(CaptureError::AlreadyRunning);
        }
        if let Some(mut previous) = session.take() {
            let _ = previous.stop();
        }
        let current = CaptureSession::start(Arc::clone(&self.backend), device_id)?;
        let format = current.format();
        *session = Some(current);
        Ok(format)
    }

    pub fn stop(&self) -> Result<(), CaptureError> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| CaptureError::WorkerFailed)?;
        match session.take() {
            Some(mut current) => current.stop(),
            None => Ok(()),
        }
    }

    pub fn try_recv(&self) -> Result<Option<CaptureEvent>, CaptureError> {
        let session = self
            .session
            .lock()
            .map_err(|_| CaptureError::WorkerFailed)?;
        match session.as_ref() {
            Some(current) => current.try_recv().map(Some).or_else(|error| match error {
                mpsc::TryRecvError::Empty => Ok(None),
                mpsc::TryRecvError::Disconnected => Err(CaptureError::StreamFailed),
            }),
            None => Ok(None),
        }
    }
}

/// Captura WASAPI de um endpoint pelo id: um dispositivo de render vira
/// loopback (áudio do sistema) e um de captura vira microfone — a distinção
/// é feita pelo próprio dispositivo, não por este backend.
#[derive(Default)]
pub struct WasapiCapture;

impl CaptureBackend for WasapiCapture {
    fn run(
        &self,
        device_id: &str,
        stop: Receiver<()>,
        ready: Sender<Result<PcmFormat, CaptureError>>,
        events: SyncSender<CaptureEvent>,
    ) -> Result<(), CaptureError> {
        let setup = (|| {
            wasapi::initialize_mta()
                .ok()
                .map_err(|_| CaptureError::StreamFailed)?;
            let result = run_with_com(device_id, &stop, &ready, &events);
            wasapi::deinitialize();
            result
        })();
        if let Err(error) = setup {
            let _ = ready.send(Err(error));
        }
        setup
    }
}

fn run_with_com(
    device_id: &str,
    stop: &Receiver<()>,
    ready: &Sender<Result<PcmFormat, CaptureError>>,
    events: &SyncSender<CaptureEvent>,
) -> Result<(), CaptureError> {
    let enumerator = DeviceEnumerator::new().map_err(|_| CaptureError::StreamFailed)?;
    let device = enumerator
        .get_device(device_id)
        .map_err(|_| CaptureError::DeviceUnavailable)?;
    if device
        .get_state()
        .map_err(|_| CaptureError::DeviceUnavailable)?
        != DeviceState::Active
    {
        return Err(CaptureError::DeviceUnavailable);
    }
    let mut client = device
        .get_iaudioclient()
        .map_err(|_| CaptureError::StreamFailed)?;
    let wave = client
        .get_mixformat()
        .map_err(|_| CaptureError::UnsupportedFormat)?;
    let format = PcmFormat::from_wave_format(&wave).map_err(|_| CaptureError::UnsupportedFormat)?;
    let (period, _) = client
        .get_device_period()
        .map_err(|_| CaptureError::StreamFailed)?;
    client
        .initialize_client(
            &wave,
            &Direction::Capture,
            &StreamMode::EventsShared {
                autoconvert: false,
                buffer_duration_hns: period,
            },
        )
        .map_err(|_| CaptureError::StreamFailed)?;
    let event = client
        .set_get_eventhandle()
        .map_err(|_| CaptureError::StreamFailed)?;
    let frames = client
        .get_buffer_size()
        .map_err(|_| CaptureError::StreamFailed)? as usize;
    let size = frames
        .checked_mul(format.frame_bytes())
        .filter(|size| *size > 0 && *size <= MAX_PACKET_BYTES)
        .ok_or(CaptureError::UnsupportedFormat)?;
    let mut buffer = vec![0_u8; size];
    let capture = client
        .get_audiocaptureclient()
        .map_err(|_| CaptureError::StreamFailed)?;
    client
        .start_stream()
        .map_err(|_| CaptureError::StreamFailed)?;
    let _guard = StreamGuard(&client);
    let _ = ready.send(Ok(format));
    let mut last_device_check = Instant::now();
    let mut dropped = false;
    loop {
        if stop.try_recv().is_ok() {
            return Ok(());
        }
        if last_device_check.elapsed() >= Duration::from_secs(1) {
            if device
                .get_state()
                .map_err(|_| CaptureError::DeviceUnavailable)?
                != DeviceState::Active
            {
                return Err(CaptureError::DeviceUnavailable);
            }
            last_device_check = Instant::now();
        }
        match event.wait_for_event(100) {
            Ok(()) | Err(WasapiError::EventTimeout) => (),
            Err(_) => return Err(CaptureError::StreamFailed),
        }
        loop {
            let Some(frames) = capture
                .get_next_packet_size()
                .map_err(|_| CaptureError::DeviceUnavailable)?
            else {
                return Err(CaptureError::StreamFailed);
            };
            if frames == 0 {
                break;
            }
            let (count, info) = capture
                .read_from_device(&mut buffer)
                .map_err(|_| CaptureError::DeviceUnavailable)?;
            let used = count as usize * format.frame_bytes();
            let packet = AudioPacket {
                bytes: buffer[..used].to_vec(),
                silent: info.flags.silent,
                discontinuity: info.flags.data_discontinuity || dropped,
            };
            match events.try_send(CaptureEvent::Packet(packet)) {
                Ok(()) => dropped = false,
                Err(mpsc::TrySendError::Full(_)) => dropped = true,
                Err(mpsc::TrySendError::Disconnected(_)) => return Ok(()),
            }
        }
    }
}

struct StreamGuard<'a>(&'a wasapi::AudioClient);

impl Drop for StreamGuard<'_> {
    fn drop(&mut self) {
        let _ = self.0.stop_stream();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::convert::PcmEncoding;

    fn format() -> PcmFormat {
        PcmFormat {
            sample_rate: 48_000,
            channels: 2,
            encoding: PcmEncoding::Float32,
        }
    }

    #[test]
    fn start_failure_is_reported_and_can_retry() {
        let mut backend = MockCaptureBackend::new();
        backend.expect_run().times(1).returning(|_, _, ready, _| {
            ready.send(Err(CaptureError::DeviceUnavailable)).unwrap();
            Err(CaptureError::DeviceUnavailable)
        });
        let manager = CaptureManager::new(Arc::new(backend));
        assert_eq!(
            manager.start("missing".into()),
            Err(CaptureError::DeviceUnavailable)
        );
        assert_eq!(manager.stop(), Ok(()));
    }

    #[test]
    fn duplicate_start_is_rejected_and_stop_is_idempotent() {
        let mut backend = MockCaptureBackend::new();
        backend
            .expect_run()
            .times(1)
            .returning(|_, stop, ready, _| {
                ready.send(Ok(format())).unwrap();
                let _ = stop.recv();
                Ok(())
            });
        let manager = CaptureManager::new(Arc::new(backend));
        assert_eq!(manager.start("output".into()), Ok(format()));
        assert_eq!(
            manager.start("output".into()),
            Err(CaptureError::AlreadyRunning)
        );
        assert_eq!(manager.stop(), Ok(()));
        assert_eq!(manager.stop(), Ok(()));
    }

    #[test]
    fn drop_stops_and_joins_worker() {
        let finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let marker = Arc::clone(&finished);
        let mut backend = MockCaptureBackend::new();
        backend
            .expect_run()
            .times(1)
            .returning(move |_, stop, ready, _| {
                ready.send(Ok(format())).unwrap();
                let _ = stop.recv();
                marker.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            });
        let session = CaptureSession::start(Arc::new(backend), "output".into()).unwrap();
        drop(session);
        assert!(finished.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn stopped_session_can_restart_without_reusing_the_worker() {
        let mut backend = MockCaptureBackend::new();
        backend
            .expect_run()
            .times(2)
            .returning(|_, stop, ready, _| {
                ready.send(Ok(format())).unwrap();
                let _ = stop.recv();
                Ok(())
            });
        let manager = CaptureManager::new(Arc::new(backend));
        for _ in 0..2 {
            assert_eq!(manager.start("output".into()), Ok(format()));
            assert_eq!(manager.stop(), Ok(()));
        }
    }
}
