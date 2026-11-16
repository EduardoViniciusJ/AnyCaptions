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

/// Exact size of the Nemotron 3 Diarization GGUF (the `Content-Length` of the
/// pinned revision), shown on its download button.
pub const DIARIZATION_DOWNLOAD_BYTES: u64 = 107_012_128;

/// Where the Riva GGUF is served from; `ANYCAPTIONS_RIVA_URL` overrides it
/// during development.
const RIVA_DOWNLOAD_URL: &str = "https://huggingface.co/EduardoVinicius/anycaptions-riva-translate/resolve/main/riva-translate-4b-instruct-v2.q4_k_m.gguf";

/// Where the diarization GGUF is served from: NVIDIA's own model repository,
/// pinned to the revision the catalog hash was taken from, so a later upload
/// can never change what the app installs. `ANYCAPTIONS_DIARIZATION_URL`
/// overrides it during development.
const DIARIZATION_DOWNLOAD_URL: &str = "https://huggingface.co/nvidia/Nemotron-3-Diarization/resolve/f667ed73aee57d40cc39428eb768b4fd87a0a29e/Nemotron-3-Diarization.q8_0.gguf";

/// Exact size of the M2M100 `model.bin` of the pinned revision.
pub const M2M100_DOWNLOAD_BYTES: u64 = 490_667_713;

/// Where the M2M100 files are served from: the converted repository, pinned
/// to the revision the catalog hashes were taken from.
/// `ANYCAPTIONS_M2M100_URL` overrides the `model.bin` URL during development;
/// the other files are always fetched from the same folder.
const M2M100_DOWNLOAD_URL: &str = "https://huggingface.co/jncraton/m2m100_418M-ct2-int8/resolve/7c1b2620a4e58dacecbd8bf89cfd6da7eb9eb7b0/model.bin";

/// A catalog model the app offers for download, with its pinned source.
pub struct DownloadSource {
    pub profile: &'static ModelProfile,
    pub url: String,
    /// Small files the model needs in the same folder, each with its URL.
    /// They are fetched first, so the model only counts as installed once
    /// its main file (the last one) is verified in place.
    pub companions: Vec<(&'static ModelProfile, String)>,
}

/// Every model the app can fetch: the catalog profile, its download size and
/// the environment variable that overrides its URL during development.
struct DownloadableModel {
    profile: &'static ModelProfile,
    url: &'static str,
    url_override: &'static str,
    /// Files served next to `url` that belong in the model's folder.
    companions: &'static [&'static ModelProfile],
}

const DOWNLOADABLE_MODELS: &[DownloadableModel] = &[
    DownloadableModel {
        profile: &catalog::RIVA_TRANSLATE_Q4,
        url: RIVA_DOWNLOAD_URL,
        url_override: "ANYCAPTIONS_RIVA_URL",
        companions: &[],
    },
    DownloadableModel {
        profile: &catalog::M2M100_418M,
        url: M2M100_DOWNLOAD_URL,
        url_override: "ANYCAPTIONS_M2M100_URL",
        companions: &[
            &catalog::M2M100_CONFIG,
            &catalog::M2M100_VOCABULARY,
            &catalog::M2M100_TOKENIZER,
        ],
    },
    DownloadableModel {
        profile: &catalog::NEMOTRON_3_DIARIZATION_Q8,
        url: DIARIZATION_DOWNLOAD_URL,
        url_override: "ANYCAPTIONS_DIARIZATION_URL",
        companions: &[],
    },
];

/// Resolves a model id the frontend asked for; only catalog models with a
/// configured source are downloadable, so the webview never supplies a URL.
pub fn source_for(model_id: &str) -> Option<DownloadSource> {
    let model = DOWNLOADABLE_MODELS
        .iter()
        .find(|model| model.profile.id == model_id)?;
    model.url().map(|url| DownloadSource {
        profile: model.profile,
        companions: model
            .companions
            .iter()
            .map(|companion| (*companion, sibling_url(&url, companion.filename)))
            .collect(),
        url,
    })
}

/// The URL of `filename` in the same folder as `url`.
fn sibling_url(url: &str, filename: &str) -> String {
    match url.rsplit_once('/') {
        Some((folder, _)) => format!("{folder}/{filename}"),
        None => filename.to_owned(),
    }
}

/// Whether `model_id` has a download URL (built in or from its override).
pub fn source_configured(model_id: &str) -> bool {
    DOWNLOADABLE_MODELS
        .iter()
        .any(|model| model.profile.id == model_id && model.url().is_some())
}

pub fn riva_source_configured() -> bool {
    source_configured(catalog::RIVA_TRANSLATE_Q4.id)
}

impl DownloadableModel {
    fn url(&self) -> Option<String> {
        std::env::var(self.url_override)
            .ok()
            .filter(|url| !url.is_empty())
            .or_else(|| (!self.url.is_empty()).then(|| self.url.to_owned()))
    }
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
            let result = run(&source, &root, &flag, &mut |received, total| {
                sink.download_changed(&DownloadUpdate {
                    model,
                    phase: DownloadPhase::Downloading,
                    received_bytes: received,
                    total_bytes: total,
                    reason: None,
                });
            });
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
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const RESPONSE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Downloads and installs one model with its companion files; returns the
/// byte count of the main file on success. Progress covers the main file: the
/// companions are a few megabytes and arrive first.
/// Kept free of Tauri types so tests can drive it against a local server.
fn run(
    source: &DownloadSource,
    root: &Path,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(u64, Option<u64>),
) -> Result<u64, DownloadFailure> {
    // Without these a server that accepts the connection and then says
    // nothing kept the download "running" forever: cancel is only seen
    // between reads, and no other download or update could start.
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_response(Some(RESPONSE_TIMEOUT))
        .build()
        .into();
    for (companion, url) in &source.companions {
        // An intact file from an earlier, interrupted attempt is kept.
        if companion.verify(&companion.path_in(root)).is_err() {
            fetch(&agent, companion, url, root, cancel, &mut |_, _| {})?;
        }
    }
    fetch(&agent, source.profile, &source.url, root, cancel, report)
}

/// Streams one catalog file to its place under `root`, verifying its hash.
fn fetch(
    agent: &ureq::Agent,
    profile: &ModelProfile,
    url: &str,
    root: &Path,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(u64, Option<u64>),
) -> Result<u64, DownloadFailure> {
    let final_path = profile.path_in(root);
    let dir = final_path.parent().ok_or(DownloadFailure::Disk)?;
    fs::create_dir_all(dir).map_err(|_| DownloadFailure::Disk)?;
    let part_path = dir.join(format!("{}.part", profile.filename));

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
    fs::rename(&part_path, &final_path).map_err(|_| {
        // Gigabytes of verified data are useless under the partial name.
        let _ = fs::remove_file(&part_path);
        DownloadFailure::Disk
    })?;
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
    if !super::digest_matches(hasher, profile.sha256) {
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

    /// A download of `TEST_PROFILE` alone, served from `url`.
    fn source(url: String) -> DownloadSource {
        DownloadSource {
            profile: &TEST_PROFILE,
            url,
            companions: Vec::new(),
        }
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
        let received = run(&source(url), &root, &never, &mut |done, total| {
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
        let result = run(&source(url), &root, &never, &mut |_, _| {});
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
        let result = run(&source(url), &root, &never, &mut |_, _| {});
        assert_eq!(result, Err(DownloadFailure::Corrupted));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_cancelled_download_cleans_up_its_partial_file() {
        let root = isolated_root("cancel");
        let url = serve(b"model", Some(5));
        let cancelled = AtomicBool::new(true);
        let result = run(&source(url), &root, &cancelled, &mut |_, _| {});
        assert_eq!(result, Err(DownloadFailure::Cancelled));
        let dir = TEST_PROFILE.path_in(&root);
        assert!(!dir.exists());
        assert!(!dir.parent().unwrap().join("model.bin.part").exists());
        let _ = fs::remove_dir_all(&root);
    }

    // SHA-256 of the bytes "vocab".
    const TEST_COMPANION: ModelProfile = ModelProfile {
        id: "test-download-vocabulary",
        filename: "vocabulary.json",
        sha256: "c1a0dc2cb0f1d0a1d5a1c8b6b2b5b0e4a6a2f0f4b0c0d6f6b2f2b6c2a8e4d0f2",
        ..TEST_PROFILE
    };

    #[test]
    fn companion_files_arrive_before_the_model_and_gate_the_install() {
        let never = AtomicBool::new(false);

        // A companion that fails its hash stops the install before the main
        // file is even requested.
        let root = isolated_root("companion-bad");
        let bad = DownloadSource {
            profile: &TEST_PROFILE,
            url: "http://127.0.0.1:1/model.bin".to_owned(),
            companions: vec![(&TEST_COMPANION, serve(b"vocab", Some(5)))],
        };
        assert_eq!(
            run(&bad, &root, &never, &mut |_, _| {}),
            Err(DownloadFailure::Corrupted)
        );
        assert!(!TEST_PROFILE.path_in(&root).exists());
        assert!(!TEST_COMPANION.path_in(&root).exists());
        let _ = fs::remove_dir_all(&root);

        // With a matching companion both files land in the same folder.
        let root = isolated_root("companion-ok");
        let companion = ModelProfile {
            sha256: "9372c470eeadd5ecd9c3c74c2b3cb633f8e2f2fad799250a0f70d652b6b825e4",
            ..TEST_COMPANION
        };
        let companion: &'static ModelProfile = Box::leak(Box::new(companion));
        let good = DownloadSource {
            profile: &TEST_PROFILE,
            url: serve(b"model", Some(5)),
            companions: vec![(companion, serve(b"model", Some(5)))],
        };
        assert_eq!(run(&good, &root, &never, &mut |_, _| {}), Ok(5));
        assert_eq!(
            TEST_PROFILE.path_in(&root).parent(),
            companion.path_in(&root).parent()
        );
        assert_eq!(fs::read(companion.path_in(&root)).unwrap(), b"model");

        // An intact companion from an earlier attempt is not fetched again:
        // its URL now points nowhere and the install still succeeds.
        fs::remove_file(TEST_PROFILE.path_in(&root)).unwrap();
        let retry = DownloadSource {
            profile: &TEST_PROFILE,
            url: serve(b"model", Some(5)),
            companions: vec![(companion, "http://127.0.0.1:1/gone".to_owned())],
        };
        assert_eq!(run(&retry, &root, &never, &mut |_, _| {}), Ok(5));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_m2m100_download_fetches_every_file_from_the_pinned_folder() {
        let source = source_for(catalog::M2M100_418M.id).expect("m2m100 is downloadable");
        assert_eq!(source.companions.len(), 3);
        for (profile, url) in &source.companions {
            assert!(url.contains(catalog::M2M100_REVISION), "{url}");
            assert!(url.ends_with(&format!("/{}", profile.filename)), "{url}");
            assert_eq!(profile.directory, catalog::M2M100_418M.directory);
        }
        assert_eq!(
            sibling_url("https://host/repo/resolve/rev/model.bin", "config.json"),
            "https://host/repo/resolve/rev/config.json"
        );
    }

    #[test]
    fn only_catalog_models_with_a_source_are_downloadable() {
        assert!(source_for("unknown-model").is_none());
        // The ASR model ships with the installer and is never downloaded.
        assert!(source_for(catalog::NEMOTRON_STREAMING_Q8.id).is_none());
        assert!(!source_configured(catalog::NEMOTRON_STREAMING_Q8.id));
        // Riva only becomes downloadable once a URL is configured.
        assert_eq!(
            source_for(catalog::RIVA_TRANSLATE_Q4.id).is_some(),
            riva_source_configured()
        );
        // The diarization model is pinned to the revision the catalog hash
        // was taken from, straight from NVIDIA's repository.
        let diarization = source_for(catalog::NEMOTRON_3_DIARIZATION_Q8.id).unwrap();
        assert_eq!(
            diarization.profile.id,
            catalog::NEMOTRON_3_DIARIZATION_Q8.id
        );
        assert!(source_configured(catalog::NEMOTRON_3_DIARIZATION_Q8.id));
        if std::env::var_os("ANYCAPTIONS_DIARIZATION_URL").is_none() {
            assert!(
                diarization
                    .url
                    .contains(catalog::NEMOTRON_3_DIARIZATION_Q8.revision)
            );
            assert!(
                diarization
                    .url
                    .starts_with("https://huggingface.co/nvidia/")
            );
            assert!(
                diarization
                    .url
                    .ends_with(catalog::NEMOTRON_3_DIARIZATION_Q8.filename)
            );
        }
    }
}
