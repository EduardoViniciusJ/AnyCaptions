//! Self-update: replaces only the app's own files and never touches `models/`.
//!
//! A release publishes `update.json` (version, package name, size and SHA-256),
//! its minisign signature and a zip with the executable and its DLLs. The
//! manifest is verified against the public key built into the app before
//! anything else is trusted; the package is then checked against the signed
//! hash. Files are swapped by renaming: Windows allows renaming a running
//! executable or loaded DLL, so the old ones become `.old` and any failure puts
//! them back. It only runs when the user clicks "Update".

use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::update::{CURRENT_VERSION, newer};

/// Verifies every update. The private key never enters the repository.
const PUBLIC_KEY: &str = "RWTcbjUZlwuKjzVjLhU49QKNIuIxHQ+8lu4VoAvJZgwDho+Mqf48H8Ap";
const LATEST_BASE_URL: &str =
    "https://github.com/EduardoViniciusJ/AnyCaptions/releases/latest/download";
const RELEASE_BASE_URL: &str = "https://github.com/EduardoViniciusJ/AnyCaptions/releases/download";
const MANIFEST_NAME: &str = "update.json";
const SIGNATURE_NAME: &str = "update.json.minisig";
const STAGING_DIR: &str = ".update";
const OLD_SUFFIX: &str = ".old";
const MAX_METADATA_BYTES: u64 = 64 * 1024;
const MAX_PACKAGE_BYTES: u64 = 1024 * 1024 * 1024;
/// Registry entry the NSIS installer creates for "Installed apps".
const UNINSTALL_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\AnyCaptions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateFailure {
    Network,
    Signature,
    Corrupted,
    Disk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdatePhase {
    Downloading,
    Installing,
    Restarting,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub phase: UpdatePhase,
    pub received_bytes: u64,
    pub total_bytes: u64,
    pub reason: Option<UpdateFailure>,
}

/// Receives progress; the IPC layer forwards it to the overlay.
pub trait UpdateSink: Send + Sync + 'static {
    fn update_changed(&self, progress: &UpdateProgress);
}

/// Allows one installation at a time.
#[derive(Default)]
pub struct UpdateInstaller {
    running: AtomicBool,
}

impl UpdateInstaller {
    pub fn begin(&self) -> bool {
        !self.running.swap(true, Ordering::SeqCst)
    }

    pub fn finish(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

/// Debug builds run from `target/`, where replacing files would make no sense.
pub fn installable() -> bool {
    !cfg!(debug_assertions)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Manifest {
    version: String,
    asset: String,
    size: u64,
    sha256: String,
}

/// Flat names only, so a package can never write outside the app folder.
fn safe_file_name(name: &str, extensions: &[&str]) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && extensions.iter().any(|extension| {
            name.len() > extension.len()
                && name[name.len() - extension.len()..].eq_ignore_ascii_case(extension)
        })
}

fn verify_manifest(
    manifest: &[u8],
    signature: &str,
    public_key: &str,
    current: &str,
) -> Result<Manifest, UpdateFailure> {
    let key = PublicKey::from_base64(public_key).map_err(|_| UpdateFailure::Signature)?;
    let signature = Signature::decode(signature).map_err(|_| UpdateFailure::Signature)?;
    key.verify(manifest, &signature, false)
        .map_err(|_| UpdateFailure::Signature)?;
    let manifest: Manifest =
        serde_json::from_slice(manifest).map_err(|_| UpdateFailure::Corrupted)?;
    let valid = newer(&manifest.version, current)
        && safe_file_name(&manifest.asset, &[".zip"])
        && manifest.size > 0
        && manifest.size <= MAX_PACKAGE_BYTES
        && manifest.sha256.len() == 64
        && manifest.sha256.chars().all(|c| c.is_ascii_hexdigit());
    // An older signed manifest is refused too, so nobody can replay a downgrade.
    valid.then_some(manifest).ok_or(UpdateFailure::Corrupted)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .build()
        .into()
}

fn fetch_text(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, UpdateFailure> {
    let mut response = agent.get(url).call().map_err(|_| UpdateFailure::Network)?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_METADATA_BYTES)
        .read_to_vec()
        .map_err(|_| UpdateFailure::Network)
}

/// Streams the package to `path`, hashing as it arrives.
fn download_package(
    agent: &ureq::Agent,
    url: &str,
    path: &Path,
    manifest: &Manifest,
    report: &mut dyn FnMut(u64),
) -> Result<(), UpdateFailure> {
    let mut response = agent.get(url).call().map_err(|_| UpdateFailure::Network)?;
    let mut reader = response
        .body_mut()
        .with_config()
        .limit(manifest.size + 1)
        .reader();
    let mut file = File::create(path).map_err(|_| UpdateFailure::Disk)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];
    let mut received = 0u64;
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|_| UpdateFailure::Network)?;
        if count == 0 {
            break;
        }
        received += count as u64;
        if received > manifest.size {
            return Err(UpdateFailure::Corrupted);
        }
        hasher.update(&buffer[..count]);
        file.write_all(&buffer[..count])
            .map_err(|_| UpdateFailure::Disk)?;
        report(received);
    }
    file.flush().map_err(|_| UpdateFailure::Disk)?;
    let hash = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if received != manifest.size || !hash.eq_ignore_ascii_case(&manifest.sha256) {
        return Err(UpdateFailure::Corrupted);
    }
    Ok(())
}

/// Unpacks into `staging`, refusing anything but flat `.exe`/`.dll` entries.
/// The package must contain the running executable's name.
fn extract_package(
    package: &Path,
    staging: &Path,
    exe_name: &str,
) -> Result<Vec<String>, UpdateFailure> {
    let file = File::open(package).map_err(|_| UpdateFailure::Disk)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| UpdateFailure::Corrupted)?;
    let mut names = Vec::with_capacity(archive.len());
    let mut remaining = MAX_PACKAGE_BYTES;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| UpdateFailure::Corrupted)?;
        let name = entry.name().to_owned();
        if entry.is_dir() || !safe_file_name(&name, &[".exe", ".dll"]) {
            return Err(UpdateFailure::Corrupted);
        }
        let mut output = File::create(staging.join(&name)).map_err(|_| UpdateFailure::Disk)?;
        let mut limited = entry.take(remaining + 1);
        let written =
            std::io::copy(&mut limited, &mut output).map_err(|_| UpdateFailure::Corrupted)?;
        if written > remaining {
            return Err(UpdateFailure::Corrupted);
        }
        remaining -= written;
        names.push(name);
    }
    if !names.iter().any(|name| name.eq_ignore_ascii_case(exe_name)) {
        return Err(UpdateFailure::Corrupted);
    }
    Ok(names)
}

fn old_path(install_dir: &Path, name: &str) -> PathBuf {
    install_dir.join(format!("{name}{OLD_SUFFIX}"))
}

/// Moves the staged files into place. On any failure, what was already moved
/// goes back and the app stays on its current version.
fn swap_files(install_dir: &Path, staging: &Path, names: &[String]) -> Result<(), UpdateFailure> {
    let mut done: Vec<(&String, bool)> = Vec::with_capacity(names.len());
    let result = names.iter().try_for_each(|name| {
        let target = install_dir.join(name);
        let old = old_path(install_dir, name);
        let _ = fs::remove_file(&old);
        let existed = target.exists();
        if existed {
            fs::rename(&target, &old)?;
        }
        done.push((name, existed));
        fs::rename(staging.join(name), &target)
    });
    if result.is_err() {
        for (name, existed) in done.iter().rev() {
            let target = install_dir.join(name);
            if *existed {
                // The new file may be missing if this was the entry that failed.
                let _ = fs::remove_file(&target);
                let _ = fs::rename(old_path(install_dir, name), &target);
            } else {
                let _ = fs::remove_file(&target);
            }
        }
        return Err(UpdateFailure::Disk);
    }
    Ok(())
}

/// Removes what the previous update could not delete while it was running.
pub fn cleanup_leftovers(install_dir: &Path) {
    let _ = fs::remove_dir_all(install_dir.join(STAGING_DIR));
    let Ok(entries) = fs::read_dir(install_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy().to_ascii_lowercase();
        if name.ends_with(".exe.old") || name.ends_with(".dll.old") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Keeps "Installed apps" showing the right version. Cosmetic, so best effort.
fn record_installed_version(version: &str) {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let reg = |args: &[&str]| {
        Command::new("reg")
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .is_ok_and(|status| status.success())
    };
    if reg(&["query", UNINSTALL_KEY, "/v", "DisplayVersion"]) {
        reg(&[
            "add",
            UNINSTALL_KEY,
            "/v",
            "DisplayVersion",
            "/d",
            version,
            "/f",
        ]);
    }
}

struct Sources {
    latest_base: String,
    release_base: String,
}

impl Sources {
    /// `ANYCAPTIONS_UPDATE_BASE` serves everything from one place during
    /// testing. The signature check still applies to whatever it serves.
    fn resolve() -> Self {
        match std::env::var("ANYCAPTIONS_UPDATE_BASE")
            .ok()
            .filter(|base| !base.is_empty())
        {
            Some(base) => Self {
                latest_base: base.clone(),
                release_base: base,
            },
            None => Self {
                latest_base: LATEST_BASE_URL.to_owned(),
                release_base: RELEASE_BASE_URL.to_owned(),
            },
        }
    }

    fn package_url(&self, manifest: &Manifest) -> String {
        if self.release_base == self.latest_base {
            format!("{}/{}", self.release_base, manifest.asset)
        } else {
            format!(
                "{}/v{}/{}",
                self.release_base, manifest.version, manifest.asset
            )
        }
    }
}

fn install(
    sources: &Sources,
    public_key: &str,
    current: &str,
    install_dir: &Path,
    exe_name: &str,
    report: &mut dyn FnMut(UpdatePhase, u64, u64),
) -> Result<String, UpdateFailure> {
    let agent = agent();
    let manifest_bytes = fetch_text(&agent, &format!("{}/{MANIFEST_NAME}", sources.latest_base))?;
    let signature = fetch_text(&agent, &format!("{}/{SIGNATURE_NAME}", sources.latest_base))?;
    let signature = String::from_utf8(signature).map_err(|_| UpdateFailure::Signature)?;
    let manifest = verify_manifest(&manifest_bytes, &signature, public_key, current)?;

    let staging = install_dir.join(STAGING_DIR);
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|_| UpdateFailure::Disk)?;
    let result = (|| {
        let package = staging.join(&manifest.asset);
        let total = manifest.size;
        report(UpdatePhase::Downloading, 0, total);
        download_package(
            &agent,
            &sources.package_url(&manifest),
            &package,
            &manifest,
            &mut |received| report(UpdatePhase::Downloading, received, total),
        )?;
        report(UpdatePhase::Installing, total, total);
        let unpacked = staging.join("files");
        fs::create_dir_all(&unpacked).map_err(|_| UpdateFailure::Disk)?;
        let names = extract_package(&package, &unpacked, exe_name)?;
        swap_files(install_dir, &unpacked, &names)
    })();
    let _ = fs::remove_dir_all(&staging);
    result.map(|()| manifest.version)
}

/// Runs the whole update from the app's own folder. `Ok` means the new files
/// are in place and the caller should restart.
pub fn run(sink: &dyn UpdateSink) -> Result<(), UpdateFailure> {
    let exe = std::env::current_exe().map_err(|_| UpdateFailure::Disk)?;
    let install_dir = exe.parent().ok_or(UpdateFailure::Disk)?;
    let exe_name = exe
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(UpdateFailure::Disk)?;
    let mut last_percent = None;
    let result = install(
        &Sources::resolve(),
        PUBLIC_KEY,
        CURRENT_VERSION,
        install_dir,
        exe_name,
        &mut |phase, received, total| {
            // One event per percent is plenty for a progress line.
            let percent = received.saturating_mul(100) / total.max(1);
            if phase == UpdatePhase::Downloading && last_percent == Some(percent) {
                return;
            }
            last_percent = Some(percent);
            sink.update_changed(&UpdateProgress {
                phase,
                received_bytes: received,
                total_bytes: total,
                reason: None,
            });
        },
    );
    match result {
        Ok(version) => {
            record_installed_version(&version);
            sink.update_changed(&UpdateProgress {
                phase: UpdatePhase::Restarting,
                received_bytes: 0,
                total_bytes: 0,
                reason: None,
            });
            Ok(())
        }
        Err(reason) => {
            sink.update_changed(&UpdateProgress {
                phase: UpdatePhase::Failed,
                received_bytes: 0,
                total_bytes: 0,
                reason: Some(reason),
            });
            Err(reason)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, net::TcpListener, thread};

    use minisign::KeyPair;

    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "anycaptions-updater-{tag}-{}-{:?}",
            std::process::id(),
            thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn keypair() -> (KeyPair, String) {
        let pair = KeyPair::generate_unencrypted_keypair().unwrap();
        let public = pair.pk.to_base64();
        (pair, public)
    }

    fn sign(pair: &KeyPair, data: &[u8]) -> String {
        minisign::sign(None, &pair.sk, Cursor::new(data), None, None)
            .unwrap()
            .into_string()
    }

    fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn sha256_hex(data: &[u8]) -> String {
        Sha256::digest(data)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn manifest_json(version: &str, asset: &str, package: &[u8]) -> String {
        format!(
            r#"{{"version":"{version}","asset":"{asset}","size":{},"sha256":"{}"}}"#,
            package.len(),
            sha256_hex(package)
        )
    }

    #[test]
    fn accepts_a_signed_newer_manifest() {
        let (pair, public) = keypair();
        let json = manifest_json("0.3.0", "update-0.3.0.zip", b"zip");
        let manifest = verify_manifest(
            json.as_bytes(),
            &sign(&pair, json.as_bytes()),
            &public,
            "0.2.0",
        )
        .unwrap();
        assert_eq!(manifest.version, "0.3.0");
    }

    #[test]
    fn rejects_tampered_or_foreign_signatures() {
        let (pair, public) = keypair();
        let json = manifest_json("0.3.0", "update.zip", b"zip");
        let signature = sign(&pair, json.as_bytes());
        let tampered = json.replace("0.3.0", "9.9.9");
        assert_eq!(
            verify_manifest(tampered.as_bytes(), &signature, &public, "0.2.0"),
            Err(UpdateFailure::Signature)
        );
        let (other, _) = keypair();
        assert_eq!(
            verify_manifest(
                json.as_bytes(),
                &sign(&other, json.as_bytes()),
                &public,
                "0.2.0"
            ),
            Err(UpdateFailure::Signature)
        );
        assert_eq!(
            verify_manifest(json.as_bytes(), "not a signature", &public, "0.2.0"),
            Err(UpdateFailure::Signature)
        );
    }

    #[test]
    fn refuses_downgrades_and_unsafe_package_names_even_when_signed() {
        let (pair, public) = keypair();
        for json in [
            manifest_json("0.2.0", "update.zip", b"zip"),
            manifest_json("0.1.0", "update.zip", b"zip"),
            manifest_json("0.3.0", "../update.zip", b"zip"),
            manifest_json("0.3.0", "update.exe", b"zip"),
            r#"{"version":"0.3.0","asset":"update.zip","size":0,"sha256":"00"}"#.to_owned(),
        ] {
            assert_eq!(
                verify_manifest(
                    json.as_bytes(),
                    &sign(&pair, json.as_bytes()),
                    &public,
                    "0.2.0"
                ),
                Err(UpdateFailure::Corrupted),
                "{json}"
            );
        }
    }

    #[test]
    fn built_in_public_key_is_well_formed() {
        assert!(PublicKey::from_base64(PUBLIC_KEY).is_ok());
    }

    #[test]
    fn extraction_refuses_paths_foreign_files_and_a_missing_executable() {
        let dir = temp_dir("extract");
        for (entries, expected_ok) in [
            (vec![("app.exe", &b"new"[..]), ("a.dll", b"dll")], true),
            (vec![("../evil.exe", &b"x"[..]), ("app.exe", b"new")], false),
            (
                vec![("models/model.bin", &b"x"[..]), ("app.exe", b"new")],
                false,
            ),
            (vec![("notes.txt", &b"x"[..]), ("app.exe", b"new")], false),
            (vec![("a.dll", &b"x"[..])], false),
        ] {
            let package = dir.join("package.zip");
            fs::write(&package, zip_with(&entries)).unwrap();
            let staging = dir.join("staging");
            let _ = fs::remove_dir_all(&staging);
            fs::create_dir_all(&staging).unwrap();
            assert_eq!(
                extract_package(&package, &staging, "app.exe").is_ok(),
                expected_ok,
                "{entries:?}"
            );
        }
        assert!(!dir.parent().unwrap().join("evil.exe").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn swap_replaces_files_and_leaves_models_alone() {
        let dir = temp_dir("swap");
        let staging = dir.join(STAGING_DIR);
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(dir.join("models")).unwrap();
        fs::write(dir.join("models/model.bin"), "model").unwrap();
        fs::write(dir.join("app.exe"), "old").unwrap();
        fs::write(staging.join("app.exe"), "new").unwrap();
        fs::write(staging.join("added.dll"), "added").unwrap();
        swap_files(&dir, &staging, &["app.exe".into(), "added.dll".into()]).unwrap();
        assert_eq!(fs::read_to_string(dir.join("app.exe")).unwrap(), "new");
        assert_eq!(fs::read_to_string(dir.join("added.dll")).unwrap(), "added");
        assert_eq!(fs::read_to_string(dir.join("app.exe.old")).unwrap(), "old");
        assert_eq!(
            fs::read_to_string(dir.join("models/model.bin")).unwrap(),
            "model"
        );

        cleanup_leftovers(&dir);
        assert!(!dir.join("app.exe.old").exists());
        assert!(!dir.join(STAGING_DIR).exists());
        assert!(dir.join("models/model.bin").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_failed_swap_restores_every_original_file() {
        let dir = temp_dir("rollback");
        let staging = dir.join(STAGING_DIR);
        fs::create_dir_all(&staging).unwrap();
        fs::write(dir.join("app.exe"), "old app").unwrap();
        fs::write(dir.join("a.dll"), "old dll").unwrap();
        fs::write(staging.join("app.exe"), "new app").unwrap();
        // a.dll is missing from staging, so the second move fails.
        assert_eq!(
            swap_files(&dir, &staging, &["app.exe".into(), "a.dll".into()]),
            Err(UpdateFailure::Disk)
        );
        assert_eq!(fs::read_to_string(dir.join("app.exe")).unwrap(), "old app");
        assert_eq!(fs::read_to_string(dir.join("a.dll")).unwrap(), "old dll");
        assert!(!dir.join("a.dll.old").exists());
        let _ = fs::remove_dir_all(dir);
    }

    /// Serves `(path, body)` pairs over HTTP; unknown paths get a 404.
    fn serve(files: Vec<(String, Vec<u8>)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let mut request = [0u8; 2048];
                let count = stream.read(&mut request).unwrap_or(0);
                let request = String::from_utf8_lossy(&request[..count]);
                let path = request.split_whitespace().nth(1).unwrap_or("").to_owned();
                match files.iter().find(|(name, _)| format!("/{name}") == path) {
                    Some((_, body)) => {
                        let _ = write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(body);
                    }
                    None => {
                        let _ = write!(
                            stream,
                            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        );
                    }
                }
            }
        });
        base
    }

    fn sources(base: &str) -> Sources {
        Sources {
            latest_base: base.to_owned(),
            release_base: base.to_owned(),
        }
    }

    #[test]
    fn installs_a_signed_package_end_to_end_without_touching_models() {
        let (pair, public) = keypair();
        let package = zip_with(&[("app.exe", b"new app"), ("a.dll", b"new dll")]);
        let json = manifest_json("0.3.0", "update-0.3.0.zip", &package);
        let signature = sign(&pair, json.as_bytes());
        let base = serve(vec![
            (MANIFEST_NAME.into(), json.into_bytes()),
            (SIGNATURE_NAME.into(), signature.into_bytes()),
            ("update-0.3.0.zip".into(), package),
        ]);
        let dir = temp_dir("install");
        fs::create_dir_all(dir.join("models")).unwrap();
        fs::write(dir.join("models/model.bin"), "model").unwrap();
        fs::write(dir.join("app.exe"), "old app").unwrap();
        let mut phases = Vec::new();
        let version = install(
            &sources(&base),
            &public,
            "0.2.0",
            &dir,
            "app.exe",
            &mut |p, _, _| phases.push(p),
        )
        .unwrap();
        assert_eq!(version, "0.3.0");
        assert_eq!(fs::read_to_string(dir.join("app.exe")).unwrap(), "new app");
        assert_eq!(fs::read_to_string(dir.join("a.dll")).unwrap(), "new dll");
        assert_eq!(
            fs::read_to_string(dir.join("models/model.bin")).unwrap(),
            "model"
        );
        assert!(!dir.join(STAGING_DIR).exists());
        assert!(phases.contains(&UpdatePhase::Downloading));
        assert!(phases.contains(&UpdatePhase::Installing));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_package_that_does_not_match_the_signed_hash_changes_nothing() {
        let (pair, public) = keypair();
        let package = zip_with(&[("app.exe", b"new app")]);
        let json = manifest_json("0.3.0", "update.zip", &package);
        let signature = sign(&pair, json.as_bytes());
        let mut swapped = package.clone();
        let last = swapped.len() - 1;
        swapped[last] ^= 0xff;
        let base = serve(vec![
            (MANIFEST_NAME.into(), json.into_bytes()),
            (SIGNATURE_NAME.into(), signature.into_bytes()),
            ("update.zip".into(), swapped),
        ]);
        let dir = temp_dir("corrupt");
        fs::write(dir.join("app.exe"), "old app").unwrap();
        assert_eq!(
            install(
                &sources(&base),
                &public,
                "0.2.0",
                &dir,
                "app.exe",
                &mut |_, _, _| {}
            ),
            Err(UpdateFailure::Corrupted)
        );
        assert_eq!(fs::read_to_string(dir.join("app.exe")).unwrap(), "old app");
        assert!(!dir.join(STAGING_DIR).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_release_without_an_update_package_reports_a_network_failure() {
        let dir = temp_dir("missing");
        let base = serve(Vec::new());
        assert_eq!(
            install(
                &sources(&base),
                PUBLIC_KEY,
                "0.2.0",
                &dir,
                "app.exe",
                &mut |_, _, _| {}
            ),
            Err(UpdateFailure::Network)
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_installer_runs_once_at_a_time() {
        let installer = UpdateInstaller::default();
        assert!(installer.begin());
        assert!(!installer.begin());
        installer.finish();
        assert!(installer.begin());
    }
}
