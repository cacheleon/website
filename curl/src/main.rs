use curl_blackhole::clip::{Clip, SPEC};
use curl_blackhole::server::{Config, serve};
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Instant;

const DEFAULT_ADDR: &str = "127.0.0.1:8090";

fn main() {
    let addr = std::env::var("CURL_BLACKHOLE_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string());

    let started = Instant::now();
    let clip = Clip::render(&SPEC);
    eprintln!(
        "rendered {} frames in {} ms",
        clip.frames.len(),
        started.elapsed().as_millis()
    );

    let listener =
        TcpListener::bind(&addr).unwrap_or_else(|e| panic!("can't listen on {addr}: {e}"));
    eprintln!("listening on {addr}");
    serve(listener, Arc::new(clip), Config::default());
}
