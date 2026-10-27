//! Download of catalog models the installer does not ship: streamed to a
//! `.part` file next to the destination, hashed while it arrives and moved
//! into place only after the catalog SHA-256 matches. One download at a time;
//! the worker thread never blocks capture or the UI.

use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{ModelProfile, catalog};

/// Exact size of the Riva GGUF, shown on the download button before it starts.
pub const RIVA_DOWNLOAD_BYTES: u64 = 2_759_302_368;

/// Where the Riva GGUF is served from; `ANYCAPTIONS_RIVA_URL` overrides it
/// during development.
const RIVA_DOWNLOAD_URL: &str = "https://huggingface.co/EduardoVinicius/anycaptions-riva-translate/resolve/main/riva-translate-4b-instruct-v2.q4_k_m.gguf";

/// A catalog model the app offers for download, with its pinned source.
pub struct DownloadSource {
    pub profile: &'static ModelProfile,
    pub url: String,
}

/// Resolves a model id the frontend asked for; only catalog models with a
/// configured source are downloadable, so the webview never supplies a URL.
pub fn source_for(model_id: &str) -> Option<DownloadSource> {
    if model_id != catalog::RIVA_TRANSLATE_Q4.id {
        return None;
    }
    riva_url().map(|url| DownloadSource {
        profile: &catalog::RIVA_TRANSLATE_Q4,
        url,
    })
}

pub fn riva_source_configured() -> bool {
    riva_url().is_some()
}

fn riva_url() -> Option<String> {
    std::env::var("ANYCAPTIONS_RIVA_URL")
        .ok()
        .filter(|url| !url.is_empty())
        .or_else(|| (!RIVA_DOWNLOAD_URL.is_empty()).then(|| RIVA_DOWNLOAD_URL.to_owned()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DownloadPhase {
    Downloading,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DownloadFailure {
    Network,
    Disk,
    Corrupted,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadUpdate {
    pub model: &'static str,
    pub phase: DownloadPhase,
    pub received_bytes: u64,
    pub total_bytes: Option<u64>,
    pub reason: Option<DownloadFailure>,
}

/// Receives progress updates; the IPC layer forwards them to the overlay.
pub trait DownloadSink: Send + Sync + 'static {
    fn download_changed(&self, update: &DownloadUpdate);
}

/// The one download the app allows at a time.
#[derive(Default)]
pub struct DownloadManager {
    active: Mutex<Option<ActiveDownload>>,
}

struct ActiveDownload {
    model: &'static str,
    cancel: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadBusy;

impl DownloadManager {
    /// Model currently downloading, if the worker is still running.
    pub fn active_model(&self) -> Option<&'static str> {
        let guard = self.active.lock().ok()?;
        let active = guard.as_ref()?;
        active
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
            .then_some(active.model)
    }

    /// Asks the running download to stop; it cleans up its partial file.
    pub fn cancel(&self) {
        if let Ok(guard) = self.active.lock()
            && let Some(active) = guard.as_ref()
        {
            active.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Starts downloading `source` into `root` on a worker thread. Progress
    /// goes to `sink`; `on_installed` runs once the file is verified in place.
    pub fn start(
        &self,
        source: DownloadSource,
        root: PathBuf,
        sink: Arc<dyn DownloadSink>,
        on_installed: Box<dyn FnOnce() + Send>,
    ) -> Result<(), DownloadBusy> {
        let Ok(mut guard) = self.active.lock() else {
            return Err(DownloadBusy);
        };
        if let Some(active) = guard.as_mut() {
            match active.worker.take() {
                Some(worker) if worker.is_finished() => drop(worker.join()),
                Some(worker) => {
                    active.worker = Some(worker);
                    return Err(DownloadBusy);
                }
                None => {}
            }
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        let model = source.profile.id;
        let worker = thread::spawn(move || {
            let result = run(
                source.profile,
                &source.url,
                &root,
                &flag,
                &mut |received, total| {
                    sink.download_changed(&DownloadUpdate {
                        model,
                        phase: DownloadPhase::Downloading,
                        received_bytes: received,
                        total_bytes: total,
                        reason: None,
                    });
                },
            );
            match result {
                Ok(received) => {
                    sink.download_changed(&DownloadUpdate {
                        model,
                        phase: DownloadPhase::Ready,
                        received_bytes: received,
                        total_bytes: Some(received),
                        reason: None,
                    });
                    on_installed();
                }
                Err(reason) => sink.download_changed(&DownloadUpdate {
                    model,
                    phase: DownloadPhase::Failed,
                    received_bytes: 0,
                    total_bytes: None,
                    reason: Some(reason),
                }),
            }
        });
        *guard = Some(ActiveDownload {
            model,
            cancel,
            worker: Some(worker),
        });
        Ok(())
    }
}

const CHUNK_BYTES: usize = 64 * 1024;

/// Downloads and installs one model; returns the byte count on success.
/// Kept free of Tauri types so tests can drive it against a local server.
fn run(
    profile: &'static ModelProfile,
    url: &str,
    root: &Path,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(u64, Option<u64>),
) -> Result<u64, DownloadFailure> {
    let final_path = profile.path_in(root);
    let dir = final_path.parent().ok_or(DownloadFailure::Disk)?;
    fs::create_dir_all(dir).map_err(|_| DownloadFailure::Disk)?;
    let part_path = dir.join(format!("{}.part", profile.filename));

    let agent = ureq::Agent::new_with_defaults();
    let mut response = agent
        .get(url)
        .call()
        .map_err(|_| DownloadFailure::Network)?;
    let total = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    if total.is_some_and(|bytes| bytes == 0 || bytes > profile.max_bytes) {
        return Err(DownloadFailure::Corrupted);
    }

    let received = stream_to_part(&mut response, &part_path, profile, cancel, total, report)
        .inspect_err(|_| {
            let _ = fs::remove_file(&part_path);
        })?;
    fs::rename(&part_path, &final_path).map_err(|_| DownloadFailure::Disk)?;
    Ok(received)
}

fn stream_to_part(
    response: &mut ureq::http::Response<ureq::Body>,
    part_path: &Path,
    profile: &ModelProfile,
    cancel: &AtomicBool,
    total: Option<u64>,
    report: &mut dyn FnMut(u64, Option<u64>),
) -> Result<u64, DownloadFailure> {
    let mut file = File::create(part_path).map_err(|_| DownloadFailure::Disk)?;
    let mut reader = response.body_mut().as_reader();
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; CHUNK_BYTES];
    let mut received: u64 = 0;
    let mut last_reported: u64 = 0;
    // About one update per percent; a fixed step when the size is unknown.
    let step = total.map_or(16 * 1024 * 1024, |bytes| (bytes / 100).max(1));
    report(0, total);
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(DownloadFailure::Cancelled);
        }
        let count = reader
            .read(&mut buffer)
            .map_err(|_| DownloadFailure::Network)?;
        if count == 0 {
            break;
        }
        received += count as u64;
        if received > profile.max_bytes {
            return Err(DownloadFailure::Corrupted);
        }
        file.write_all(&buffer[..count])
            .map_err(|_| DownloadFailure::Disk)?;
        hasher.update(&buffer[..count]);
        if received - last_reported >= step {
            last_reported = received;
            report(received, total);
        }
    }
    file.sync_all().map_err(|_| DownloadFailure::Disk)?;
    drop(file);
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if !digest.eq_ignore_ascii_case(profile.sha256) {
        return Err(DownloadFailure::Corrupted);
    }
    report(received, total);
    Ok(received)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ModelKind;

    // SHA-256 of the bytes "model".
    const TEST_PROFILE: ModelProfile = ModelProfile {
        kind: ModelKind::Translation,
        id: "test-download",
        directory: "test-download",
        filename: "model.bin",
        revision: "test",
        source: "local test server",
        license: "test",
        sha256: "9372c470eeadd5ecd9c3c74c2b3cb633f8e2f2fad799250a0f70d652b6b825e4",
        max_bytes: 1024,
    };

    /// Serves one HTTP response with `body` and closes the connection.
    fn serve(body: &'static [u8], content_length: Option<usize>) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0_u8; 1024];
                let _ = stream.read(&mut request);
                let header = match content_length {
                    Some(length) => format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n"
                    ),
                    None => "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_owned(),
                };
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(body);
            }
        });
        format!("http://{addr}/model.bin")
    }

    fn isolated_root(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("anycaptions-download-{tag}-{}", std::process::id()))
    }

    #[test]
    fn downloads_verifies_and_installs_the_model() {
        let root = isolated_root("ok");
        let url = serve(b"model", Some(5));
        let mut updates = Vec::new();
        let never = AtomicBool::new(false);
        let received = run(&TEST_PROFILE, &url, &root, &never, &mut |done, total| {
            updates.push((done, total));
        })
        .unwrap();
        assert_eq!(received, 5);
        let installed = TEST_PROFILE.path_in(&root);
        assert_eq!(fs::read(&installed).unwrap(), b"model");
        assert!(!installed.parent().unwrap().join("model.bin.part").exists());
        assert_eq!(updates.first(), Some(&(0, Some(5))));
        assert_eq!(updates.last(), Some(&(5, Some(5))));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_corrupted_body_is_rejected_and_leaves_no_files() {
        let root = isolated_root("corrupt");
        let url = serve(b"not-the-model", Some(13));
        let never = AtomicBool::new(false);
        let result = run(&TEST_PROFILE, &url, &root, &never, &mut |_, _| {});
        assert_eq!(result, Err(DownloadFailure::Corrupted));
        let dir = TEST_PROFILE.path_in(&root);
        assert!(!dir.exists());
        assert!(!dir.parent().unwrap().join("model.bin.part").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_oversized_content_length_is_rejected_before_reading() {
        let root = isolated_root("oversized");
        let url = serve(b"model", Some(4096));
        let never = AtomicBool::new(false);
        let result = run(&TEST_PROFILE, &url, &root, &never, &mut |_, _| {});
        assert_eq!(result, Err(DownloadFailure::Corrupted));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_cancelled_download_cleans_up_its_partial_file() {
        let root = isolated_root("cancel");
        let url = serve(b"model", Some(5));
        let cancelled = AtomicBool::new(true);
        let result = run(&TEST_PROFILE, &url, &root, &cancelled, &mut |_, _| {});
        assert_eq!(result, Err(DownloadFailure::Cancelled));
        let dir = TEST_PROFILE.path_in(&root);
        assert!(!dir.exists());
        assert!(!dir.parent().unwrap().join("model.bin.part").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn only_catalog_models_with_a_source_are_downloadable() {
        assert!(source_for("unknown-model").is_none());
        // Riva only becomes downloadable once a URL is configured.
        assert_eq!(
            source_for(catalog::RIVA_TRANSLATE_Q4.id).is_some(),
            riva_source_configured()
        );
    }
}
