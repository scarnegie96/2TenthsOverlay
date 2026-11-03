//! Twitch emote images: fetched once each from Twitch's CDN on background
//! threads, decoded to RGBA, and handed to the widget (which uploads them as
//! textures and keeps those bounded). Failures are remembered, so a missing
//! emote is asked for once and then shown as its code.

use std::collections::HashMap;
use std::io::Read;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

pub const BASE: &str = "https://static-cdn.jtvnw.net/emoticons/v2";
const WORKERS: usize = 3;
/// Bigger downloads or pictures are refused (real emotes are a few KB, at most 112 px).
const MAX_BYTES: u64 = 1 << 20;
const MAX_SIDE: u32 = 512;
/// Failed ids remembered before the memory of them is cleared.
const MAX_FAILED: usize = 2000;

/// An emote id and the CDN scale (1 or 2).
pub type Key = (String, u8);

pub struct Image {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

pub enum Poll {
    /// Asked for (now or earlier), not here yet.
    Pending,
    /// Here; given out once, the caller keeps it.
    Ready(Image),
    Failed,
}

enum Slot {
    Queued,
    Ready(Image),
    /// Handed out already.
    Taken,
    Failed,
}

type Shared = Arc<Mutex<HashMap<Key, Slot>>>;

pub struct Emotes {
    shared: Shared,
    tx: Sender<Key>,
}

/// 1x (28 px) normally, 2x when the emote is drawn bigger than that on screen.
pub fn size_for(on_screen_scale: f32) -> u8 {
    if on_screen_scale > 1.25 {
        2
    } else {
        1
    }
}

/// Ids are letters, digits and underscores (`emotesv2_...`); anything else never reaches a URL.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

pub fn url(base: &str, id: &str, size: u8) -> String {
    format!("{}/{id}/static/dark/{size}.0", base.trim_end_matches('/'))
}

pub fn decode(bytes: &[u8]) -> Option<Image> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?;
    if img.width() == 0 || img.height() == 0 || img.width() > MAX_SIDE || img.height() > MAX_SIDE {
        return None;
    }
    let rgba = img.to_rgba8();
    Some(Image { w: rgba.width() as usize, h: rgba.height() as usize, rgba: rgba.into_raw() })
}

fn fetch(base: &str, id: &str, size: u8) -> Option<Image> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).build();
    let resp = agent.get(&url(base, id, size)).call().ok()?;
    let mut bytes = Vec::new();
    resp.into_reader().take(MAX_BYTES).read_to_end(&mut bytes).ok()?;
    decode(&bytes)
}

impl Emotes {
    /// `wake` is called when a picture arrives (or fails), to redraw.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self::with_base(BASE, wake)
    }

    pub fn with_base(base: &str, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let shared: Shared = Arc::default();
        let (tx, rx) = channel::<Key>();
        let rx = Arc::new(Mutex::new(rx));
        let wake = Arc::new(wake);
        for n in 0..WORKERS {
            let (shared, rx, wake, base) = (shared.clone(), rx.clone(), wake.clone(), base.to_string());
            std::thread::Builder::new()
                .name(format!("twitch-emotes-{n}"))
                .spawn(move || loop {
                    // The lock is only held while waiting for the next id; the sender dropping ends the thread.
                    let next = rx.lock().ok().and_then(|r: MutexGuard<Receiver<Key>>| r.recv().ok());
                    let Some((id, size)) = next else { return };
                    let slot = match fetch(&base, &id, size) {
                        Some(img) => Slot::Ready(img),
                        None => Slot::Failed,
                    };
                    if let Ok(mut m) = shared.lock() {
                        m.insert((id, size), slot);
                    }
                    wake();
                })
                .ok();
        }
        Self { shared, tx }
    }

    /// The picture for `id` at `size`, asking for it (once) if it hasn't been.
    pub fn poll(&self, id: &str, size: u8) -> Poll {
        if !valid_id(id) {
            return Poll::Failed;
        }
        let Ok(mut m) = self.shared.lock() else { return Poll::Failed };
        let key = (id.to_string(), size);
        match m.get(&key) {
            None => {
                if m.values().filter(|s| matches!(s, Slot::Failed)).count() >= MAX_FAILED {
                    m.retain(|_, s| !matches!(s, Slot::Failed));
                }
                m.insert(key.clone(), Slot::Queued);
                if self.tx.send(key).is_err() {
                    return Poll::Failed;
                }
                Poll::Pending
            }
            Some(Slot::Queued | Slot::Taken) => Poll::Pending,
            Some(Slot::Failed) => Poll::Failed,
            Some(Slot::Ready(_)) => match m.insert(key, Slot::Taken) {
                Some(Slot::Ready(img)) => Poll::Ready(img),
                _ => Poll::Pending,
            },
        }
    }

    /// The widget dropped this picture's texture: let it be fetched again if wanted.
    pub fn forget(&self, id: &str, size: u8) {
        if let Ok(mut m) = self.shared.lock() {
            if matches!(m.get(&(id.to_string(), size)), Some(Slot::Taken)) {
                m.remove(&(id.to_string(), size));
            }
        }
    }

    /// Puts a picture in without the network (the demo chat's emote).
    pub fn seed(&self, id: &str, size: u8, png: &[u8]) {
        if let (Some(img), Ok(mut m)) = (decode(png), self.shared.lock()) {
            m.insert((id.to_string(), size), Slot::Ready(img));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([200, 100, 50, 255]));
        let mut out = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    /// A tiny HTTP server on 127.0.0.1: `/emoticons/v2/7/static/dark/1.0` is a PNG, all else 404.
    /// Counts the requests it gets.
    fn server() -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/emoticons/v2", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let mut buf = [0u8; 2048];
                let n = stream.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                h.fetch_add(1, Ordering::SeqCst);
                if head.starts_with("GET /emoticons/v2/7/static/dark/1.0 ") {
                    let body = png(28, 20);
                    let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                    let _ = stream.write_all(&body);
                } else {
                    let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                }
            }
        });
        (base, hits)
    }

    fn wait(e: &Emotes, id: &str, size: u8) -> Poll {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            match e.poll(id, size) {
                Poll::Pending if Instant::now() < end => std::thread::sleep(Duration::from_millis(10)),
                other => return other,
            }
        }
    }

    #[test]
    fn url_and_ids() {
        assert_eq!(url("https://x/v2/", "25", 2), "https://x/v2/25/static/dark/2.0");
        assert_eq!(size_for(1.0), 1);
        assert_eq!(size_for(1.25), 1);
        assert_eq!(size_for(1.5), 2);
        assert!(valid_id("emotesv2_ab12"));
        assert!(!valid_id("") && !valid_id("../x") && !valid_id("a/b") && !valid_id("a?b"));
    }

    #[test]
    fn fetch_decode_once_and_remember_failures() {
        let (base, hits) = server();
        let e = Emotes::with_base(&base, || {});
        let Poll::Ready(img) = wait(&e, "7", 1) else { panic!("should arrive") };
        assert_eq!((img.w, img.h, img.rgba.len()), (28, 20, 28 * 20 * 4));
        // Handed out once; asking again doesn't fetch again.
        assert!(matches!(e.poll("7", 1), Poll::Pending));
        assert_eq!(hits.load(Ordering::SeqCst), 1, "one request per id");
        // After the texture is dropped, it can be fetched again.
        e.forget("7", 1);
        assert!(matches!(wait(&e, "7", 1), Poll::Ready(_)));
        assert_eq!(hits.load(Ordering::SeqCst), 2);

        assert!(matches!(wait(&e, "404", 1), Poll::Failed), "404 fails");
        let n = hits.load(Ordering::SeqCst);
        for _ in 0..5 {
            assert!(matches!(e.poll("404", 1), Poll::Failed));
        }
        assert_eq!(hits.load(Ordering::SeqCst), n, "failures aren't retried");
        assert!(matches!(e.poll("../etc", 1), Poll::Failed), "bad ids never go out");
        assert_eq!(hits.load(Ordering::SeqCst), n);
    }

    #[test]
    fn bad_or_huge_pictures_are_refused() {
        assert!(decode(b"not a png").is_none());
        assert!(decode(&png(600, 10)).is_none());
        assert!(decode(&png(112, 112)).is_some());
    }

    #[test]
    fn unreachable_server_fails() {
        let e = Emotes::with_base("http://127.0.0.1:1/emoticons/v2", || {});
        assert!(matches!(wait(&e, "7", 1), Poll::Failed));
    }

    #[test]
    fn seeded_pictures_need_no_network() {
        let e = Emotes::with_base("http://127.0.0.1:1/x", || {});
        e.seed("25", 1, &png(10, 10));
        assert!(matches!(e.poll("25", 1), Poll::Ready(_)));
    }
}
