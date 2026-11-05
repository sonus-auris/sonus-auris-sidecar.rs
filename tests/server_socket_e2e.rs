#![forbid(unsafe_code)]

// Real process + real TCP health traffic, not a probe stub. Exercises the same
// HTTP endpoints the kubelet invokes while the application handles concurrency.
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

struct RunningSidecar {
    child: Child,
    port: u16,
}

impl Drop for RunningSidecar {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start() -> RunningSidecar {
    let reserve = TcpListener::bind("127.0.0.1:0").expect("reserve test port");
    let port = reserve.local_addr().expect("port").port();
    drop(reserve);
    let child = Command::new(env!("CARGO_BIN_EXE_sonus-auris-sidecar"))
        .env("SONUS_AURIS_SIDECAR_BIND", format!("127.0.0.1:{port}"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start real sidecar process");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return RunningSidecar { child, port };
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("real sidecar failed to bind its configured loopback port");
}

fn request(port: u16, path: &str) -> String {
    let mut client = TcpStream::connect(("127.0.0.1", port)).expect("connect to sidecar");
    client
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("bounded response deadline");
    write!(
        client,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .expect("write HTTP request");
    let mut received = String::new();
    client
        .read_to_string(&mut received)
        .expect("complete HTTP response");
    assert!(
        received.contains("\r\n\r\n"),
        "expected complete HTTP headers"
    );
    received
}

#[test]
fn real_sidecar_handles_concurrent_health_and_readiness_requests() {
    let sidecar = start();
    let handlers: Vec<_> = (0..16)
        .map(|i| {
            let port = sidecar.port;
            thread::spawn(move || {
                let path = if i % 2 == 0 { "/healthz" } else { "/readyz" };
                let response = request(port, path);
                assert!(
                    response.starts_with("HTTP/1.1 200 "),
                    "{path} did not return an OK response"
                );
                assert!(!response.to_ascii_lowercase().contains("\r\nlocation:"));
            })
        })
        .collect();
    for handle in handlers {
        handle.join().expect("all real HTTP clients completed");
    }
}

#[test]
fn shutting_down_real_sidecar_removes_its_tcp_listener() {
    let sidecar = start();
    assert!(request(sidecar.port, "/healthz").starts_with("HTTP/1.1 200 "));
    let port = sidecar.port;
    drop(sidecar);
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_err() {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("sidecar listener remained open after process termination");
}
