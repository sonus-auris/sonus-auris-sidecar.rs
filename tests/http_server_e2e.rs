#![forbid(unsafe_code)]

// External-process, real TCP tests for the inherited sidecar HTTP service.
// The existing probe CLI tests exercise an HTTP client from another process;
// these tests exercise the *server* boundary, status routing, and concurrency.
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

struct Sidecar(Child);

impl Drop for Sidecar {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start() -> (Sidecar, u16) {
    let reserved = TcpListener::bind("127.0.0.1:0").expect("reserve ephemeral port");
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);

    let child = Command::new(env!("CARGO_BIN_EXE_sonus-auris-sidecar"))
        .env("SONUS_AURIS_SIDECAR_BIND", format!("127.0.0.1:{port}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start the real sidecar binary");

    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline, "sidecar never bound its HTTP port");
        thread::sleep(Duration::from_millis(25));
    }
    (Sidecar(child), port)
}

fn get(port: u16, path: &str) -> String {
    let mut stream = TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().unwrap(),
        Duration::from_secs(2),
    )
    .expect("connect to real sidecar HTTP server");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    )
    .expect("send complete HTTP request");
    stream.flush().unwrap();
    let mut result = Vec::new();
    stream.read_to_end(&mut result).expect("read response to EOF");
    String::from_utf8(result).expect("HTTP response is UTF-8")
}

fn assert_status(response: &str, code: u16) {
    let first_line = response.lines().next().expect("HTTP status line");
    assert!(
        first_line.starts_with(&format!("HTTP/1.1 {code} ")),
        "expected HTTP {code}, received {first_line}"
    );
    assert!(
        response.contains("\r\n\r\n"),
        "HTTP response must terminate its header block"
    );
}

#[test]
fn real_http_server_distinguishes_liveness_readiness_and_missing_route() {
    let (_server, port) = start();
    for route in ["/healthz", "/readyz"] {
        assert_status(&get(port, route), 200);
    }
    assert_status(&get(port, "/this-route-is-not-a-health-probe"), 404);
}

#[test]
fn concurrent_kubelet_clients_observe_consistent_success_over_real_sockets() {
    let (_server, port) = start();
    let workers = (0..16)
        .map(|index| {
            thread::spawn(move || {
                let path = if index % 2 == 0 { "/healthz" } else { "/readyz" };
                let response = get(port, path);
                assert_status(&response, 200);
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().expect("parallel HTTP probe thread");
    }
}
