//! One running copy: a second launch tells the first to open Settings and
//! exits. A loopback port is the lock (it's released when the process ends,
//! however it ends, so there's never a stale lock file) and the way in.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::time::Duration;

use super::Request;

/// Not the streaming port (7457) the Tauri app uses.
pub const PORT: u16 = 47_457;

pub enum Instance {
    /// This is the first copy; keep this alive for as long as the app runs.
    First(Listener),
    /// Another copy was running and has been asked to open Settings.
    AlreadyRunning,
}

pub struct Listener(TcpListener);

/// Claims the port, or hands the other copy a message.
pub fn claim() -> Instance {
    claim_on(PORT)
}

fn claim_on(port: u16) -> Instance {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    match TcpListener::bind(addr) {
        Ok(l) => Instance::First(Listener(l)),
        Err(_) => match TcpStream::connect_timeout(&addr, Duration::from_millis(500)) {
            Ok(mut s) => {
                let _ = s.write_all(b"settings\n");
                Instance::AlreadyRunning
            }
            // Something else has the port and isn't Tenths: run anyway.
            Err(_) => Instance::First(Listener(TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("any port"))),
        },
    }
}

impl Listener {
    /// Listens in the background; each "settings" line becomes a request and a repaint.
    pub fn serve(self, requests: Sender<Request>, ctx: egui::Context) {
        let _ = std::thread::Builder::new().name("single-instance".into()).spawn(move || {
            for stream in self.0.incoming().flatten() {
                let mut line = String::new();
                let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                if BufReader::new(stream).read_line(&mut line).is_ok() && line.trim() == "settings" {
                    let _ = requests.send(Request::OpenSettings);
                    ctx.request_repaint();
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn a_second_launch_asks_the_first_to_open_settings() {
        let port = 47_900 + (std::process::id() % 90) as u16;
        let Instance::First(listener) = claim_on(port) else { panic!("first") };
        let (tx, rx) = channel();
        listener.serve(tx, egui::Context::default());
        assert!(matches!(claim_on(port), Instance::AlreadyRunning));
        assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap(), Request::OpenSettings);
    }
}
