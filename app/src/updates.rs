//! Updating Tenths itself: find a newer release, download its installer,
//! check it, run it.
//!
//! Releases (`.github/workflows/release.yml`) carry the installer, its
//! signature and `latest.json`, in the format Tauri's updater used, so the
//! 0.4.x Tauri builds can update into this one and the other way round:
//!
//! * `latest.json` is read from the newest GitHub release
//!   ([`ENDPOINT`]); the Windows entry is `platforms["windows-x86_64-nsis"]` or
//!   `["windows-x86_64"]`, each with a `url` and a `signature`.
//! * The signature is the base64 of a minisign signature file, made with the key
//!   whose public half is [`PUBKEY`] (base64 of a minisign public key file, the
//!   one in the Tauri app's `tauri.conf.json`). Its trusted comment must carry
//!   `version:<x>` equal to the announced version, so an old signed installer
//!   can't be passed off as a new one.
//! * The installer is the NSIS `Tenths_<x>_x64-setup.exe`; it's run with `/P /R`
//!   (progress window only, start Tenths afterwards) and Tenths quits.
//!
//! Nothing installs without a click: a check only records what it found. Checks
//! run in the background, 20 seconds after start and then every six hours while
//! `general.checkUpdates` is on, and on "Check now". The UI reads
//! [`Updates::status`].
//!
//! `TENTHS_UPDATE_URL` replaces the endpoint (to try a release by hand against a
//! local server); the signature is checked all the same.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::Deserialize;

use crate::shell::Request;

/// The updater's public key: base64 of a minisign public key file (`plugins.updater.pubkey` in the Tauri app's `tauri.conf.json`).
pub const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEU1OTY4NDQzNUMxQUI2RTcKUldUbnRocGNRNFNXNVNsRkdtaVZTeTNPL09RcjR3UFE3T0RSQlN5SmtmTTM2S1htY3RuWVVKakQK";
/// The newest release's `latest.json`. (The 0.4 Tauri app reads the repository's old name, which GitHub redirects here.)
pub const ENDPOINT: &str = "https://github.com/scarnegie96/2TenthsOverlay/releases/latest/download/latest.json";
/// This build's version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Wait after starting before the first check, so it doesn't slow startup.
const FIRST_CHECK: Duration = Duration::from_secs(20);
/// Then check this often while Tenths runs.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// Nothing bigger than this is downloaded (the installer is about 10 MB).
const MAX_DOWNLOAD: u64 = 300 * 1024 * 1024;
/// Platform keys to try, in order (Tauri's: the installer type first).
const PLATFORM_KEYS: [&str; 2] = ["windows-x86_64-nsis", "windows-x86_64"];

// --- The release feed -------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Manifest {
    version: String,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    pub_date: Option<String>,
    #[serde(default)]
    platforms: HashMap<String, Platform>,
}

#[derive(Debug, Deserialize)]
struct Platform {
    signature: String,
    url: String,
}

/// A release newer than this build, as `latest.json` announces it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub notes: String,
    /// `2026-11-02` from `pub_date`, when it has one.
    pub published: Option<String>,
    pub url: String,
    pub signature: String,
}

/// `latest.json` to the release it announces (newer or not), for this platform.
fn parse_manifest(text: &str) -> Result<Release, String> {
    let m: Manifest = serde_json::from_str(text).map_err(|e| format!("The update information couldn't be read ({e})."))?;
    let platform = PLATFORM_KEYS
        .iter()
        .find_map(|k| m.platforms.get(*k))
        .ok_or("The newest release has no Windows installer.")?;
    Ok(Release {
        version: m.version.trim().trim_start_matches('v').to_string(),
        notes: m.notes.unwrap_or_default().trim().to_string(),
        published: m.pub_date.as_deref().and_then(|d| d.get(..10)).filter(|d| d.as_bytes().get(4) == Some(&b'-')).map(str::to_string),
        url: platform.url.clone(),
        signature: platform.signature.clone(),
    })
}

/// Whether `candidate` is a newer version than `current` (semver).
pub fn is_newer(current: &str, candidate: &str) -> Result<bool, String> {
    let parse = |v: &str| semver::Version::parse(v.trim().trim_start_matches('v')).map_err(|_| format!("\"{v}\" isn't a version number."));
    Ok(parse(candidate)? > parse(current)?)
}

// --- Signatures -------------------------------------------------------------------

/// Checks `bytes` against a base64 minisign `signature` for the base64 `public_key`, and that the
/// signature's trusted comment says `version:<expected_version>`.
pub fn verify(public_key: &str, signature: &str, bytes: &[u8], expected_version: &str) -> Result<(), String> {
    let text = |b64: &str, what: &str| -> Result<String, String> {
        let raw = B64.decode(b64.trim().as_bytes()).map_err(|_| format!("The {what} isn't valid."))?;
        String::from_utf8(raw).map_err(|_| format!("The {what} isn't valid."))
    };
    let key = minisign_verify::PublicKey::decode(&text(public_key, "update key")?).map_err(|_| "The update key isn't valid.".to_string())?;
    let sig = minisign_verify::Signature::decode(&text(signature, "update signature")?).map_err(|_| "The update's signature isn't valid.".to_string())?;
    key.verify(bytes, &sig, false).map_err(|_| "The download doesn't match its signature, so it isn't from the Tenths release key.".to_string())?;
    // The trusted comment is `key:value` pairs separated by tabs; it must name this version.
    match sig.trusted_comment().split('\t').find_map(|p| p.strip_prefix("version:")) {
        Some(v) if v == expected_version => Ok(()),
        Some(v) => Err(format!("The download is signed as version {v}, not {expected_version}, so it was refused.")),
        None => Err("The download's signature doesn't say which version it is, so it was refused.".to_string()),
    }
}

// --- Network ----------------------------------------------------------------------

fn endpoint() -> String {
    std::env::var("TENTHS_UPDATE_URL").ok().filter(|u| !u.trim().is_empty()).unwrap_or_else(|| ENDPOINT.to_string())
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .user_agent(&format!("Tenths/{VERSION}"))
        .build()
}

/// A failed request in words (the repository being private shows up as a 404).
fn describe(e: ureq::Error, what: &str) -> String {
    match e {
        ureq::Error::Status(404, _) => format!("{what}: GitHub has nothing at that address (404). There may be no release yet, or the repository isn't public."),
        ureq::Error::Status(code, _) => format!("{what}: GitHub answered with an error ({code})."),
        ureq::Error::Transport(t) => format!("{what}: couldn't reach GitHub ({t}). Check your internet connection."),
    }
}

/// Reads `latest.json` and returns the release it announces.
fn fetch_release(url: &str) -> Result<Release, String> {
    let resp = agent().get(url).call().map_err(|e| describe(e, "Couldn't check for updates"))?;
    let mut text = String::new();
    resp.into_reader().take(2 * 1024 * 1024).read_to_string(&mut text).map_err(|e| format!("Couldn't check for updates: {e}."))?;
    parse_manifest(&text)
}

/// Downloads `url` into memory, telling `progress(done, total)` as it goes.
fn download(url: &str, mut progress: impl FnMut(u64, Option<u64>)) -> Result<Vec<u8>, String> {
    let resp = agent().get(url).call().map_err(|e| describe(e, "Couldn't download the update"))?;
    let total = resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok());
    if total.is_some_and(|t| t > MAX_DOWNLOAD) {
        return Err("The update is far bigger than expected, so it wasn't downloaded.".into());
    }
    let mut reader = resp.into_reader();
    let mut bytes = Vec::with_capacity(total.unwrap_or(0).min(MAX_DOWNLOAD) as usize);
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut chunk).map_err(|e| format!("The download stopped half way: {e}."))?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n]);
        if bytes.len() as u64 > MAX_DOWNLOAD {
            return Err("The update is far bigger than expected, so the download was stopped.".into());
        }
        progress(bytes.len() as u64, total);
    }
    if total.is_some_and(|t| t != bytes.len() as u64) {
        return Err("The download stopped half way.".into());
    }
    Ok(bytes)
}

/// The file name to keep the installer under: the last part of its URL, made safe.
fn installer_name(url: &str) -> String {
    let last = url.split(['?', '#']).next().unwrap_or("").rsplit('/').next().unwrap_or("");
    let safe: String = last.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')).collect();
    if safe.to_lowercase().ends_with(".exe") && safe.len() > 4 { safe } else { "Tenths-setup.exe".into() }
}

/// Runs the installer: progress window only (`/P`), start Tenths afterwards (`/R`). On Windows only.
#[cfg(windows)]
fn launch(installer: &std::path::Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP: it carries on after Tenths quits.
    std::process::Command::new(installer).args(["/P", "/R"]).creation_flags(0x0000_0008 | 0x0000_0200).spawn().map(|_| ()).map_err(|e| format!("The installer couldn't be started: {e}."))
}

#[cfg(not(windows))]
fn launch(_installer: &std::path::Path) -> Result<(), String> {
    Err("Updates install on Windows only.".into())
}

// --- State the UI reads -------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    Downloading { done: u64, total: Option<u64> },
    Verifying,
    /// The installer is running; Tenths is about to quit.
    Launching,
}

/// What the About page and the tray show.
#[derive(Debug, Clone, Default)]
pub struct Status {
    pub phase: Phase,
    /// Seconds since 1970 of the last check that worked.
    pub last_checked: Option<u64>,
    /// A release newer than this build, from the last check.
    pub available: Option<Release>,
    /// Why the last check or install failed, in words.
    pub error: Option<String>,
}

impl Status {
    pub fn busy(&self) -> bool {
        self.phase != Phase::Idle
    }
}

/// The updater: owned by the app, which calls [`tick`](Updates::tick) every frame; the UI calls
/// [`check_now`](Updates::check_now) and [`install`](Updates::install). Work runs on threads.
pub struct Updates {
    state: Arc<Mutex<Status>>,
    quit: Sender<Request>,
    next_auto: Option<Instant>,
    /// `general.checkUpdates`: kept here (the config's own copy is the raw file), set at start,
    /// when the config file changes and when the About page's switch is used.
    auto: Arc<AtomicBool>,
}

fn lock(state: &Mutex<Status>) -> MutexGuard<'_, Status> {
    state.lock().unwrap_or_else(|e| e.into_inner())
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Updates {
    /// `quit`: where to ask the app to quit once the installer is running.
    pub fn new(quit: Sender<Request>) -> Self {
        Self { state: Arc::default(), quit, next_auto: None, auto: Arc::new(AtomicBool::new(true)) }
    }

    pub fn auto(&self) -> bool {
        self.auto.load(Ordering::Relaxed)
    }

    pub fn set_auto(&self, on: bool) {
        self.auto.store(on, Ordering::Relaxed);
    }

    /// Reads `general.checkUpdates` from the config file (on unless switched off).
    pub fn load_auto(&self, config: Option<&Path>) {
        let value: Option<serde_json::Value> = config.and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok());
        self.set_auto(value.and_then(|v| v["general"]["checkUpdates"].as_bool()).unwrap_or(true));
    }

    pub fn status(&self) -> Status {
        lock(&self.state).clone()
    }

    /// The automatic checks: the first 20 s after the first call, then every six hours, while
    /// `general.checkUpdates` is on and `allowed` (off for `--offline` and screenshots).
    pub fn tick(&mut self, ctx: &egui::Context, allowed: bool) {
        if !allowed {
            return;
        }
        let now = Instant::now();
        let due = *self.next_auto.get_or_insert(now + FIRST_CHECK);
        if now >= due {
            if self.auto() && !self.status().busy() {
                self.next_auto = Some(now + CHECK_EVERY);
                self.check_now(ctx);
            }
        } else {
            ctx.request_repaint_after(due - now);
        }
    }

    /// Looks for a newer release in the background (does nothing while another check or an install is running).
    pub fn check_now(&self, ctx: &egui::Context) {
        {
            let mut s = lock(&self.state);
            if s.phase != Phase::Idle {
                return;
            }
            s.phase = Phase::Checking;
            s.error = None;
        }
        let (state, ctx) = (self.state.clone(), ctx.clone());
        std::thread::spawn(move || {
            let result = fetch_release(&endpoint()).and_then(|r| is_newer(VERSION, &r.version).map(|newer| newer.then_some(r)));
            let mut s = lock(&state);
            s.phase = Phase::Idle;
            match result {
                Ok(found) => {
                    s.last_checked = Some(now_secs());
                    s.available = found;
                }
                Err(e) => s.error = Some(e),
            }
            drop(s);
            ctx.request_repaint();
        });
    }

    /// Downloads the release the last check found, verifies it, runs the installer and asks the app to quit.
    pub fn install(&self, ctx: &egui::Context) {
        let release = {
            let mut s = lock(&self.state);
            let Some(release) = s.available.clone().filter(|_| s.phase == Phase::Idle) else { return };
            if !cfg!(windows) {
                s.error = Some("Updates install on Windows only.".into());
                return;
            }
            s.phase = Phase::Downloading { done: 0, total: None };
            s.error = None;
            release
        };
        let (state, ctx, quit) = (self.state.clone(), ctx.clone(), self.quit.clone());
        std::thread::spawn(move || {
            match run_install(&release, &state, &ctx) {
                Ok(()) => {
                    lock(&state).phase = Phase::Launching;
                    let _ = quit.send(Request::Quit);
                }
                Err(e) => {
                    let mut s = lock(&state);
                    s.phase = Phase::Idle;
                    s.error = Some(e);
                }
            }
            ctx.request_repaint();
        });
    }
}

/// Download, verify, write, verify the file again, launch.
fn run_install(release: &Release, state: &Mutex<Status>, ctx: &egui::Context) -> Result<(), String> {
    // https only (the override is for trying a release by hand): the signature is the real protection.
    if !release.url.starts_with("https://") && std::env::var_os("TENTHS_UPDATE_URL").is_none() {
        return Err("The update isn't offered over a secure connection, so it was refused.".into());
    }
    let mut last = Instant::now();
    let bytes = download(&release.url, |done, total| {
        if last.elapsed() > Duration::from_millis(50) || Some(done) == total {
            last = Instant::now();
            lock(state).phase = Phase::Downloading { done, total };
            ctx.request_repaint();
        }
    })?;
    lock(state).phase = Phase::Verifying;
    ctx.request_repaint();
    verify(PUBKEY, &release.signature, &bytes, &release.version)?;
    let dir = std::env::temp_dir().join("tenths-update");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't save the installer: {e}."))?;
    let path = dir.join(installer_name(&release.url));
    std::fs::write(&path, &bytes).map_err(|e| format!("Couldn't save the installer: {e}."))?;
    // Again on what is on disk, just before it runs.
    let saved = std::fs::read(&path).map_err(|e| format!("Couldn't read the saved installer: {e}."))?;
    verify(PUBKEY, &release.signature, &saved, &release.version)?;
    launch(&path)
}

// --- Words for the UI ---------------------------------------------------------------

/// "just now", "5 minutes ago", "2 hours ago", "3 days ago".
pub fn ago(then: u64, now: u64) -> String {
    let s = now.saturating_sub(then);
    let plural = |n: u64, unit: &str| format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" });
    match s {
        0..=59 => "just now".into(),
        60..=3599 => plural(s / 60, "minute"),
        3600..=86_399 => plural(s / 3600, "hour"),
        _ => plural(s / 86_400, "day"),
    }
}

/// "3.2 of 10.4 MB", or "3.2 MB" when the total isn't known.
pub fn progress_text(done: u64, total: Option<u64>) -> String {
    let mb = |b: u64| b as f64 / 1_048_576.0;
    match total {
        Some(t) => format!("{:.1} of {:.1} MB", mb(done), mb(t)),
        None => format!("{:.1} MB", mb(done)),
    }
}

pub fn now() -> u64 {
    now_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as B64;
    use std::io::Write;
    use std::net::TcpListener;

    fn key_pair() -> (String, String) {
        let pair = minisign::KeyPair::generate_encrypted_keypair(Some("pw".into())).unwrap();
        (B64.encode(pair.sk.to_box(Some("test")).unwrap().into_string()), B64.encode(pair.pk.to_box().unwrap().into_string()))
    }

    fn sign(sk: &str, bytes: &[u8], version: &str) -> String {
        tenths_release::sign(sk, "pw", bytes, "Tenths_x64-setup.exe", version, 1_700_000_000).unwrap()
    }

    #[test]
    fn the_baked_in_key_is_a_minisign_public_key() {
        let text = String::from_utf8(B64.decode(PUBKEY).unwrap()).unwrap();
        assert!(text.starts_with("untrusted comment: minisign public key"), "{text}");
        minisign_verify::PublicKey::decode(&text).unwrap();
    }

    #[test]
    fn the_manifest_is_read_under_either_platform_key() {
        let both = r#"{ "version": "v0.5.1", "notes": " - a\n - b \n", "pub_date": "2026-11-02T12:00:00Z",
            "platforms": { "windows-x86_64": { "signature": "S1", "url": "https://x/one.exe" },
                           "windows-x86_64-nsis": { "signature": "S2", "url": "https://x/two.exe" } } }"#;
        let r = parse_manifest(both).unwrap();
        assert_eq!((r.version.as_str(), r.url.as_str(), r.signature.as_str()), ("0.5.1", "https://x/two.exe", "S2"));
        assert_eq!(r.notes, "- a\n - b");
        assert_eq!(r.published.as_deref(), Some("2026-11-02"));
        let plain = r#"{ "version": "0.5.1", "platforms": { "windows-x86_64": { "signature": "S1", "url": "https://x/one.exe" } } }"#;
        let r = parse_manifest(plain).unwrap();
        assert_eq!((r.url.as_str(), r.notes.as_str(), r.published), ("https://x/one.exe", "", None));
        // Other platforms only, or not a manifest at all.
        assert!(parse_manifest(r#"{ "version": "1.0.0", "platforms": { "linux-x86_64": { "signature": "s", "url": "u" } } }"#).unwrap_err().contains("no Windows installer"));
        assert!(parse_manifest("<html>").unwrap_err().contains("couldn't be read"));
    }

    #[test]
    fn versions_compare_as_semver() {
        assert!(is_newer("0.4.13", "0.5.0").unwrap());
        assert!(is_newer("0.5.0", "0.5.1").unwrap());
        assert!(is_newer("0.9.0", "0.10.0").unwrap(), "numbers, not text");
        assert!(!is_newer("0.5.0", "0.5.0").unwrap());
        assert!(!is_newer("0.5.0", "0.4.13").unwrap());
        assert!(!is_newer("0.5.0", "0.5.0-beta.1").unwrap(), "a pre-release isn't newer than its release");
        assert!(is_newer("0.5.0-beta.1", "0.5.0").unwrap());
        assert!(is_newer("0.5.0", "v0.5.1").unwrap());
        assert!(is_newer("0.5.0", "soon").is_err());
    }

    #[test]
    fn signatures_are_checked_on_the_bytes_the_key_and_the_version() {
        let (sk, pk) = key_pair();
        let (_, other_pk) = key_pair();
        let bytes = b"the installer";
        let sig = sign(&sk, bytes, "0.5.1");
        verify(&pk, &sig, bytes, "0.5.1").unwrap();
        // One byte changed.
        assert!(verify(&pk, &sig, b"the installeR", "0.5.1").unwrap_err().contains("doesn't match its signature"));
        // Another key.
        assert!(verify(&other_pk, &sig, bytes, "0.5.1").is_err());
        // Signed as another version: the old installer announced as the new one.
        assert!(verify(&pk, &sig, bytes, "0.6.0").unwrap_err().contains("signed as version 0.5.1, not 0.6.0"));
        // No version in the trusted comment at all.
        let key = minisign::SecretKeyBox::from_string(&String::from_utf8(B64.decode(&sk).unwrap()).unwrap()).unwrap().into_secret_key(Some("pw".into())).unwrap();
        let bare = minisign::sign(None, &key, std::io::Cursor::new(&bytes[..]), Some("timestamp:1"), None).unwrap().into_string();
        assert!(verify(&pk, &B64.encode(bare), bytes, "0.5.1").unwrap_err().contains("doesn't say which version"));
        // Garbage.
        assert!(verify(&pk, "!!!", bytes, "0.5.1").is_err());
        assert!(verify("!!!", &sig, bytes, "0.5.1").is_err());
        assert!(verify(&pk, &B64.encode("not a signature"), bytes, "0.5.1").is_err());
        // The Tenths key doesn't accept a signature from some other key.
        assert!(verify(PUBKEY, &sig, bytes, "0.5.1").is_err());
    }

    /// Serves `routes` (path, body) over HTTP on a local port for as many requests as come.
    fn serve(routes: Vec<(&'static str, Vec<u8>)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]);
                let path = head.split_whitespace().nth(1).unwrap_or("/");
                match routes.iter().find(|(p, _)| *p == path) {
                    Some((_, body)) => {
                        let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                        let _ = stream.write_all(body);
                    }
                    None => {
                        let _ = write!(stream, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    }
                }
            }
        });
        base
    }

    #[test]
    fn a_release_is_fetched_downloaded_and_verified_end_to_end() {
        let (sk, pk) = key_pair();
        let installer = vec![7u8; 200_000];
        let sig = sign(&sk, &installer, "0.5.1");
        let base = serve(vec![("/setup.exe", installer.clone()), ("/latest.json", Vec::new())]);
        // The manifest the release tool writes, pointing at the local server.
        let manifest = tenths_release::manifest("0.5.1", "- notes", "2026-11-02T12:00:00Z", &[("windows-x86_64", &sig, &format!("{base}/setup.exe")), ("windows-x86_64-nsis", &sig, &format!("{base}/setup.exe"))]);
        let base = serve(vec![("/setup.exe", installer.clone()), ("/latest.json", manifest.into_bytes())]);
        let release = fetch_release(&format!("{base}/latest.json")).unwrap();
        assert_eq!(release.version, "0.5.1");
        assert!(release.url.ends_with("/setup.exe"));
        let mut seen = Vec::new();
        let bytes = download(&release.url, |d, t| seen.push((d, t))).unwrap();
        assert_eq!(bytes, installer);
        assert_eq!(seen.last(), Some(&(200_000, Some(200_000))));
        verify(&pk, &release.signature, &bytes, &release.version).unwrap();
        // A missing file is explained, not a stack of jargon.
        let err = fetch_release(&format!("{base}/missing.json")).unwrap_err();
        assert!(err.contains("404") && err.contains("Couldn't check for updates"), "{err}");
        // Nothing listening.
        assert!(fetch_release("http://127.0.0.1:1/latest.json").unwrap_err().contains("couldn't reach"));
    }

    #[test]
    fn installer_names_are_safe() {
        assert_eq!(installer_name("https://github.com/x/releases/download/v0.5.0/Tenths_0.5.0_x64-setup.exe"), "Tenths_0.5.0_x64-setup.exe");
        assert_eq!(installer_name("https://x/a/b.exe?token=1"), "b.exe");
        assert_eq!(installer_name("https://x/a/..%2F..%2Fevil.cmd"), "Tenths-setup.exe");
        assert_eq!(installer_name("https://x/"), "Tenths-setup.exe");
    }

    #[test]
    fn words_for_times_and_sizes() {
        assert_eq!(ago(100, 130), "just now");
        assert_eq!(ago(0, 60), "1 minute ago");
        assert_eq!(ago(0, 3 * 3600 + 5), "3 hours ago");
        assert_eq!(ago(0, 2 * 86_400), "2 days ago");
        assert_eq!(progress_text(3_355_443, Some(10_905_190)), "3.2 of 10.4 MB");
        assert_eq!(progress_text(1_048_576, None), "1.0 MB");
    }

    #[test]
    fn the_auto_check_setting_is_read_from_the_config() {
        let dir = std::env::temp_dir().join(format!("tenths-egui-upd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        let (tx, _rx) = std::sync::mpsc::channel();
        let u = Updates::new(tx);
        u.load_auto(Some(&path));
        assert!(u.auto());
        std::fs::write(&path, r#"{ "general": { "checkUpdates": false } }"#).unwrap();
        u.load_auto(Some(&path));
        assert!(!u.auto());
        std::fs::write(&path, r#"{ "general": {} }"#).unwrap();
        u.load_auto(Some(&path));
        assert!(u.auto());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn checks_are_scheduled_and_never_overlap() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut u = Updates::new(tx);
        let ctx = egui::Context::default();
        // Not allowed (offline, screenshots): never starts a clock.
        u.tick(&ctx, false);
        assert!(u.next_auto.is_none());
        assert!(u.auto(), "on unless switched off");
        // The first call schedules 20 s ahead and checks nothing yet.
        u.tick(&ctx, true);
        assert!(u.next_auto.unwrap() > Instant::now() + Duration::from_secs(15));
        assert!(!u.status().busy());
        // While a check or install runs, another isn't started.
        lock(&u.state).phase = Phase::Checking;
        u.check_now(&ctx);
        assert_eq!(u.status().phase, Phase::Checking);
        // Installing needs a release found first.
        lock(&u.state).phase = Phase::Idle;
        u.install(&ctx);
        assert_eq!(u.status().phase, Phase::Idle);
    }
}
