//! Exercise the *built sidecar process* over actual HTTP/1.1 sockets.
//! No providers, tokens, Kubernetes access or shared test ports are required.
#![forbid(unsafe_code)]

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_sonus-auris-sidecar")
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("ephemeral address")
        .port()
}

struct Sidecar {
    child: Child,
    port: u16,
}

impl Sidecar {
    fn spawn() -> Self {
        let port = free_port();
        let child = Command::new(binary())
            .env("SONUS_AURIS_SIDECAR_BIND", format!("127.0.0.1:{port}"))
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn real sidecar process");
        let mut fixture = Self { child, port };
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                return fixture;
            }
            assert!(
                fixture.child.try_wait().expect("poll sidecar").is_none(),
                "sidecar exited before accepting HTTP"
            );
            thread::sleep(Duration::from_millis(25));
        }
        panic!("sidecar did not listen within 8 seconds");
    }

    fn address(&self) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], self.port))
    }
}

impl Drop for Sidecar {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn request(address: SocketAddr, method: &str, path: &str) -> (u16, String) {
    let mut stream =
        TcpStream::connect_timeout(&address, Duration::from_secs(3)).expect("connect to sidecar");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("read deadline");
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .expect("write deadline");
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nAccept: application/json\r\n\r\n"
    )
    .expect("send complete HTTP request");
    stream.flush().expect("flush request");

    let mut bytes = Vec::new();
    stream
        .read_to_end(&mut bytes)
        .expect("bounded HTTP response");
    assert!(
        bytes.len() <= 128 * 1024,
        "sidecar response exceeded test ceiling"
    );
    let response = String::from_utf8(bytes).expect("HTTP response UTF-8");
    let (status_line, _) = response.split_once("\r\n").expect("HTTP status line");
    let code = status_line
        .split_whitespace()
        .nth(1)
        .expect("HTTP status code")
        .parse::<u16>()
        .expect("numeric HTTP status");
    (code, response)
}

#[test]
fn sidecar_exposes_health_and_readiness_over_real_http() {
    let server = Sidecar::spawn();
    for path in ["/healthz", "/readyz"] {
        let (status, response) = request(server.address(), "GET", path);
        assert_eq!(status, 200, "{path} did not report healthy: {status}");
        assert!(
            response.contains("sonus-auris-sidecar"),
            "{path} response lost its Sonus service identity"
        );
        assert!(
            !response.contains("Bearer "),
            "{path} unexpectedly emitted bearer material"
        );
    }
}

#[test]
fn concurrent_clients_and_bad_routes_do_not_break_subsequent_health() {
    let server = Sidecar::spawn();
    let address = server.address();
    let workers: Vec<_> = (0..24)
        .map(|index| {
            thread::spawn(move || {
                let path = if index % 2 == 0 {
                    "/healthz"
                } else {
                    "/readyz"
                };
                let (status, _) = request(address, "GET", path);
                assert_eq!(status, 200, "concurrent {path}: {status}");
            })
        })
        .collect();
    for worker in workers {
        worker.join().expect("concurrent HTTP client");
    }

    let (missing, _) = request(address, "GET", "/private-fixture-not-a-route");
    assert_eq!(
        missing, 404,
        "a missing path must not serve a healthy payload"
    );
    let (after, _) = request(address, "GET", "/healthz");
    assert_eq!(after, 200, "a rejected request broke the running sidecar");
}

#[test]
fn probe_commands_and_http_clients_agree_on_live_readiness() {
    let server = Sidecar::spawn();
    let bind = format!("127.0.0.1:{}", server.port);
    for (command, path) in [("probe-healthz", "/healthz"), ("probe-readyz", "/readyz")] {
        let status = Command::new(binary())
            .arg(command)
            .env("SONUS_AURIS_SIDECAR_BIND", &bind)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("run sidecar probe subprocess");
        assert!(
            status.success(),
            "{command} rejected an HTTP-healthy sidecar"
        );
        assert_eq!(request(server.address(), "GET", path).0, 200);
    }
}
