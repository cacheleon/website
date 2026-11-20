use curl_blackhole::ansi::{CLEAR, RESET_END};
use curl_blackhole::clip::{Clip, Spec};
use curl_blackhole::server::{Config, serve};
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

const CURL: &str = "curl/8.7.1";

fn clip() -> Arc<Clip> {
    static CLIP: OnceLock<Arc<Clip>> = OnceLock::new();
    CLIP.get_or_init(|| {
        Arc::new(Clip::render(&Spec {
            cols: 8,
            rows: 4,
            frames: 6,
            intro_frames: 3,
            fps: 20.0,
            fade_secs: 0.1,
        }))
    })
    .clone()
}

fn config() -> Config {
    Config {
        fps: 20.0,
        max_duration: Duration::from_millis(500),
        max_streams: 1,
        header_timeout: Duration::from_millis(200),
        write_timeout: Duration::from_millis(200),
    }
}

fn start(config: Config) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let clip = clip();
    thread::spawn(move || serve(listener, clip, config));
    addr
}

fn get(addr: SocketAddr, method: &str, ua: &str) -> TcpStream {
    let mut s = TcpStream::connect(addr).unwrap();
    write!(
        s,
        "{method} / HTTP/1.1\r\nHost: x\r\nUser-Agent: {ua}\r\n\r\n"
    )
    .unwrap();
    s
}

fn read_for(s: &mut TcpStream, dur: Duration) -> (String, bool) {
    let end = Instant::now() + dur;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return (String::from_utf8(buf).unwrap(), false);
        }
        s.set_read_timeout(Some(left)).unwrap();
        match s.read(&mut chunk) {
            Ok(0) => return (String::from_utf8(buf).unwrap(), true),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => panic!("read failed: {e}"),
        }
    }
}

fn split(resp: &str) -> (&str, &str) {
    resp.split_once("\r\n\r\n").expect("a full response head")
}

#[test]
fn curl_gets_headers_and_animation() {
    let addr = start(config());
    let (resp, _) = read_for(&mut get(addr, "GET", CURL), Duration::from_millis(400));
    let (head, body) = split(&resp);
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert!(head.contains("Cache-Control: no-store"));
    assert!(head.contains("Content-Type: text/plain; charset=utf-8"));
    assert!(
        body.starts_with(&format!("{CLEAR}{}", clip().intro[0])),
        "intro plays first"
    );
    assert!(
        body.matches("\x1b[H").count() >= 4,
        "only {} frames",
        body.matches("\x1b[H").count()
    );
}

#[test]
fn stream_ends_after_max_duration() {
    let addr = start(config());
    let (resp, eof) = read_for(&mut get(addr, "GET", CURL), Duration::from_secs(2));
    assert!(eof, "stream never ended");
    assert!(split(&resp).1.ends_with(RESET_END));
}

#[test]
fn wget_gets_still_frame() {
    let addr = start(config());
    let (resp, eof) = read_for(&mut get(addr, "GET", "Wget/1.21.4"), Duration::from_secs(1));
    assert!(eof);
    assert_eq!(split(&resp).1, clip().still);
}

#[test]
fn powershell_gets_still_frame() {
    let addr = start(config());
    let ua = "Mozilla/5.0 (Windows NT; Windows NT 10.0; en-US) WindowsPowerShell/5.1.19041.1";
    let (resp, eof) = read_for(&mut get(addr, "GET", ua), Duration::from_secs(1));
    assert!(eof);
    assert_eq!(split(&resp).1, clip().still);
}

#[test]
fn over_cap_gets_still_frame() {
    let addr = start(config());
    let mut first = get(addr, "GET", CURL);
    read_for(&mut first, Duration::from_millis(100));
    let (resp, eof) = read_for(&mut get(addr, "GET", CURL), Duration::from_secs(1));
    assert!(eof);
    assert_eq!(split(&resp).1, clip().still);
    drop(first);
}

#[test]
fn disconnect_frees_slot() {
    let addr = start(Config {
        max_duration: Duration::from_secs(10),
        ..config()
    });
    let mut first = get(addr, "GET", CURL);
    read_for(&mut first, Duration::from_millis(100));
    drop(first);
    thread::sleep(Duration::from_millis(300));
    let (resp, eof) = read_for(&mut get(addr, "GET", CURL), Duration::from_millis(300));
    assert!(!eof, "got a still frame: slot was not freed");
    assert!(split(&resp).1.matches("\x1b[H").count() >= 2);
}

#[test]
fn head_gets_headers_only() {
    let addr = start(config());
    let (resp, eof) = read_for(&mut get(addr, "HEAD", CURL), Duration::from_secs(1));
    assert!(eof);
    assert_eq!(split(&resp).1, "");
}

#[test]
fn silent_client_is_dropped() {
    let addr = start(config());
    let mut s = TcpStream::connect(addr).unwrap();
    let (_, eof) = read_for(&mut s, Duration::from_secs(1));
    assert!(eof);
}

#[test]
fn client_that_never_reads_frees_slot() {
    // fast enough to fill the socket buffers, then the write has to time out
    let addr = start(Config {
        fps: 2000.0,
        max_duration: Duration::from_secs(60),
        ..config()
    });
    let _stuck = get(addr, "GET", CURL);
    thread::sleep(Duration::from_secs(3));
    let (resp, eof) = read_for(&mut get(addr, "GET", CURL), Duration::from_millis(300));
    assert!(!eof, "got a still frame: the stuck client still holds the slot");
    assert!(split(&resp).1.matches("\x1b[H").count() >= 2);
}
