//! Streaming (experimental): the widgets as web pages for OBS.
//!
//! When it's on (Settings → Streaming), a small web server listens on
//! 127.0.0.1 only, so nothing outside this PC can reach it. It serves the app's
//! own pages (`stream.html`) and pushes the same live data the overlays get as
//! Server-Sent Events on `/events`, so an OBS Browser Source shows a widget, or
//! a whole layout, on a transparent background without capturing the screen.
//!
//! - `GET /stream.html?...` and its scripts: from the app's bundled files.
//! - `GET /events`: `event: <name>` / `data: <json>` for status, session,
//!   reference, field, tick (at most 30 a second), config-changed,
//!   displays-changed and driver-ratings.
//! - `GET /api/<name>`: what a page needs to start (snapshot, config,
//!   displays, profiles, driver_ratings). Read-only: nothing here changes
//!   anything in the app.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};

pub const DEFAULT_PORT: u16 = 7457;
/// Ticks are 60 a second; OBS doesn't need more than this.
const TICK_EVERY: Duration = Duration::from_millis(33);
/// Messages a slow page can fall behind by before it misses some.
const BACKLOG: usize = 512;
/// A comment this often keeps idle connections open.
const KEEPALIVE: Duration = Duration::from_secs(15);

struct Running {
    port: u16,
    stop: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct StreamHub {
    clients: Mutex<Vec<SyncSender<Arc<str>>>>,
    last_tick: Mutex<Option<Instant>>,
    server: Mutex<Option<Running>>,
}

/// The `general.streaming` part of the config.
fn wanted(config: &Value) -> Option<u16> {
    let s = &config["general"]["streaming"];
    if s["enabled"].as_bool() != Some(true) {
        return None;
    }
    Some(s["port"].as_u64().and_then(|p| u16::try_from(p).ok()).filter(|p| *p >= 1024).unwrap_or(DEFAULT_PORT))
}

impl StreamHub {
    /// Sends an event to every connected page. Cheap when nobody's connected.
    pub fn publish<T: Serialize>(&self, event: &str, payload: &T) {
        let mut clients = self.clients.lock().unwrap();
        if clients.is_empty() {
            return;
        }
        if event == "tick" {
            let mut last = self.last_tick.lock().unwrap();
            let now = Instant::now();
            if last.is_some_and(|t| now - t < TICK_EVERY) {
                return;
            }
            *last = Some(now);
        }
        let Ok(json) = serde_json::to_string(payload) else { return };
        let msg: Arc<str> = format!("event: {event}\ndata: {json}\n\n").into();
        clients.retain(|c| match c.try_send(msg.clone()) {
            Ok(()) | Err(TrySendError::Full(_)) => true,
            Err(TrySendError::Disconnected(_)) => false,
        });
    }

    fn subscribe(&self) -> Receiver<Arc<str>> {
        let (tx, rx) = mpsc::sync_channel(BACKLOG);
        self.clients.lock().unwrap().push(tx);
        rx
    }

    /// Starts, stops or moves the server to match the config.
    pub fn apply(&self, app: &AppHandle, config: &Value) {
        let want = wanted(config);
        let mut server = self.server.lock().unwrap();
        if server.as_ref().map(|r| r.port) == want {
            return;
        }
        if let Some(r) = server.take() {
            r.stop.store(true, Ordering::Relaxed);
            // Wake the accept loop so it sees the flag.
            let _ = TcpStream::connect(("127.0.0.1", r.port));
        }
        if let Some(port) = want {
            match TcpListener::bind(("127.0.0.1", port)) {
                Ok(listener) => {
                    let stop = Arc::new(AtomicBool::new(false));
                    let (app, flag) = (app.clone(), stop.clone());
                    thread::spawn(move || serve(app, listener, port, flag));
                    *server = Some(Running { port, stop });
                }
                Err(e) => eprintln!("[stream] couldn't listen on 127.0.0.1:{port}: {e}"),
            }
        }
    }

    pub fn port(&self) -> Option<u16> {
        self.server.lock().unwrap().as_ref().map(|r| r.port)
    }
}

fn serve(app: AppHandle, listener: TcpListener, port: u16, stop: Arc<AtomicBool>) {
    for conn in listener.incoming() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let Ok(stream) = conn else { continue };
        let app = app.clone();
        thread::spawn(move || {
            let _ = handle(&app, stream, port);
        });
    }
}

struct Request {
    method: String,
    path: String,
    host: String,
}

fn read_request(stream: &TcpStream) -> std::io::Result<Request> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.take(16 * 1024));
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();
    let mut host = String::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h == "\r\n" || h == "\n" {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            if k.eq_ignore_ascii_case("host") {
                host = v.trim().to_string();
            }
        }
    }
    Ok(Request { method, path, host })
}

/// Only this PC, by name: stops a web page elsewhere reaching it through DNS rebinding.
fn host_ok(host: &str, port: u16) -> bool {
    host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}")
}

fn respond(mut stream: TcpStream, status: &str, mime: &str, body: &[u8]) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()?;
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}

fn json<T: Serialize>(stream: TcpStream, value: &T) -> std::io::Result<()> {
    let body = serde_json::to_vec(value).unwrap_or_else(|_| b"null".to_vec());
    respond(stream, "200 OK", "application/json", &body)
}

fn handle(app: &AppHandle, stream: TcpStream, port: u16) -> std::io::Result<()> {
    let req = read_request(&stream)?;
    if !host_ok(&req.host, port) {
        return respond(stream, "403 Forbidden", "text/plain", b"Tenths only answers on 127.0.0.1");
    }
    if req.method != "GET" {
        return respond(stream, "405 Method Not Allowed", "text/plain", b"Read-only");
    }
    let path = req.path.split('?').next().unwrap_or("/").to_string();
    match path.as_str() {
        "/" => respond(stream, "200 OK", "text/html; charset=utf-8", index().as_bytes()),
        "/events" => events(app, stream),
        p if p.starts_with("/api/") => api(app, stream, &p[5..]),
        p => match app.asset_resolver().get(p.to_string()) {
            Some(asset) => respond(stream, "200 OK", &asset.mime_type, &asset.bytes),
            None => respond(stream, "404 Not Found", "text/plain", b"Not found"),
        },
    }
}

fn events(app: &AppHandle, mut stream: TcpStream) -> std::io::Result<()> {
    stream.set_read_timeout(None)?;
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\n: tenths\n\n"
    )?;
    stream.flush()?;
    let rx = app.state::<StreamHub>().subscribe();
    loop {
        match rx.recv_timeout(KEEPALIVE) {
            Ok(msg) => stream.write_all(msg.as_bytes())?,
            Err(mpsc::RecvTimeoutError::Timeout) => stream.write_all(b": keepalive\n\n")?,
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }
        stream.flush()?;
    }
}

fn api(app: &AppHandle, stream: TcpStream, name: &str) -> std::io::Result<()> {
    match name {
        "snapshot" => json(stream, &crate::snapshot(app)),
        "config" => json(stream, &crate::read_config(app)),
        "displays" => json(stream, &*app.state::<crate::displays::Displays>().list.lock().unwrap()),
        "profiles" => json(stream, &crate::profiles_dir(app).map(|d| crate::profiles::list(&d)).unwrap_or_default()),
        "driver_ratings" => {
            let all: BTreeMap<String, crate::ratings::DriverRating> =
                crate::ratings_dir(app).map(|d| app.state::<crate::ratings::Ratings>().all(&d)).unwrap_or_default();
            json(stream, &all)
        }
        _ => respond(stream, "404 Not Found", "text/plain", b"Not found"),
    }
}

/// `/`: a pointer to the pages, for anyone who opens the address in a browser.
fn index() -> String {
    "<!doctype html><meta charset=utf-8><title>Tenths for OBS</title>\
     <body style=\"font-family:system-ui;max-width:40em;margin:3em auto\">\
     <h1>Tenths for OBS</h1><p>Add a Browser Source in OBS with one of the addresses from \
     Tenths' Settings → Streaming, for example <code>/stream.html?widget=relative</code>.</p>"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_and_hosts() {
        assert_eq!(wanted(&serde_json::json!({})), None);
        assert_eq!(wanted(&serde_json::json!({"general": {"streaming": {"enabled": true}}})), Some(DEFAULT_PORT));
        assert_eq!(wanted(&serde_json::json!({"general": {"streaming": {"enabled": true, "port": 8123}}})), Some(8123));
        assert_eq!(wanted(&serde_json::json!({"general": {"streaming": {"enabled": true, "port": 80}}})), Some(DEFAULT_PORT));
        assert_eq!(wanted(&serde_json::json!({"general": {"streaming": {"enabled": false, "port": 8123}}})), None);
        assert!(host_ok("127.0.0.1:7457", 7457));
        assert!(host_ok("localhost:7457", 7457));
        assert!(!host_ok("evil.example:7457", 7457));
        assert!(!host_ok("127.0.0.1:80", 7457));
    }

    #[test]
    fn slow_or_gone_pages_dont_block() {
        let hub = StreamHub::default();
        let rx = hub.subscribe();
        hub.publish("field", &serde_json::json!({"a": 1}));
        assert_eq!(&*rx.recv().unwrap(), "event: field\ndata: {\"a\":1}\n\n");
        // Ticks are thinned out.
        hub.publish("tick", &1);
        hub.publish("tick", &2);
        assert!(rx.recv().unwrap().contains("data: 1"));
        assert!(rx.try_recv().is_err());
        drop(rx);
        hub.publish("field", &0);
        assert!(hub.clients.lock().unwrap().is_empty());
    }
}
