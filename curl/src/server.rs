// just enough http for curl. nginx only sends terminal clients here.

use crate::ansi::{CLEAR, RESET_END};
use crate::clip::Clip;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const MAX_REQUEST_BYTES: usize = 8 * 1024;

const HEAD: &str = "HTTP/1.1 200 OK\r\n\
    Content-Type: text/plain; charset=utf-8\r\n\
    Cache-Control: no-store\r\n\
    X-Accel-Buffering: no\r\n\
    Connection: close\r\n\
    \r\n";

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub fps: f32,
    pub max_duration: Duration,
    pub max_streams: usize,
    pub header_timeout: Duration,
    pub write_timeout: Duration,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            fps: 12.0,
            max_duration: Duration::from_secs(120),
            max_streams: 50,
            header_timeout: Duration::from_secs(5),
            write_timeout: Duration::from_secs(10),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientKind {
    Animate,
    Still,
}

// wget saves to a file and powershell waits for the whole body
pub fn client_kind(user_agent: &str) -> ClientKind {
    let ua = user_agent.to_ascii_lowercase();
    if ua.contains("wget") || ua.contains("powershell") {
        ClientKind::Still
    } else {
        ClientKind::Animate
    }
}

pub fn serve(listener: TcpListener, clip: Arc<Clip>, config: Config) -> ! {
    let streams = Arc::new(AtomicUsize::new(0));
    loop {
        let Ok((conn, _)) = listener.accept() else {
            continue;
        };
        let (clip, streams) = (clip.clone(), streams.clone());
        thread::spawn(move || {
            // errors here are just people pressing ctrl-c
            let _ = handle(conn, &clip, &config, &streams);
        });
    }
}

fn handle(
    mut conn: TcpStream,
    clip: &Clip,
    config: &Config,
    streams: &AtomicUsize,
) -> io::Result<()> {
    conn.set_read_timeout(Some(config.header_timeout))?;
    // a client that stops reading would otherwise hold its slot forever
    conn.set_write_timeout(Some(config.write_timeout))?;
    conn.set_nodelay(true)?;
    let Some(request) = read_request(&mut conn) else {
        return Ok(());
    };

    conn.write_all(HEAD.as_bytes())?;
    if request.method == "HEAD" {
        return Ok(());
    }
    if client_kind(&request.user_agent) == ClientKind::Still {
        return conn.write_all(clip.still.as_bytes());
    }
    match Slot::take(streams, config.max_streams) {
        Some(_slot) => play(&mut conn, clip, config),
        None => conn.write_all(clip.still.as_bytes()),
    }
}

struct Request {
    method: String,
    user_agent: String,
}

fn read_request(conn: &mut TcpStream) -> Option<Request> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = conn.read(&mut chunk).ok().filter(|&n| n > 0)?;
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() > MAX_REQUEST_BYTES {
            return None;
        }
    }

    let text = String::from_utf8_lossy(&buf);
    let mut lines = text.lines();
    let method = lines.next()?.split(' ').next()?.to_string();
    let user_agent = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("user-agent"))
        .map(|(_, value)| value.trim().to_string())
        .unwrap_or_default();
    Some(Request { method, user_agent })
}

// fixed schedule so slow writes don't make it drift
fn play(conn: &mut TcpStream, clip: &Clip, config: &Config) -> io::Result<()> {
    conn.write_all(CLEAR.as_bytes())?;
    let start = Instant::now();
    let frame_time = Duration::from_secs_f32(1.0 / config.fps);

    let frames = clip.intro.iter().chain(clip.frames.iter().cycle());
    for (n, frame) in frames.enumerate() {
        let due = start + frame_time * n as u32;
        if due - start >= config.max_duration {
            break;
        }
        thread::sleep(due.saturating_duration_since(Instant::now()));
        conn.write_all(frame.as_bytes())?;
    }
    conn.write_all(RESET_END.as_bytes())
}

// freed on drop, which also covers the client hanging up
struct Slot<'a>(&'a AtomicUsize);

impl<'a> Slot<'a> {
    fn take(streams: &'a AtomicUsize, max: usize) -> Option<Slot<'a>> {
        streams
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < max).then_some(n + 1)
            })
            .ok()
            .map(|_| Slot(streams))
    }
}

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
