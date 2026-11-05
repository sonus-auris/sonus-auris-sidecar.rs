#![forbid(unsafe_code)]

// Real product binary over TCP plus kubelet-style probe subprocesses.
// This is not a unit test of the shared sidecar library: the subprocess must
// accept requests, enforce its HTTP boundary, and return the correct exit code.
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn executable() -> &'static str {
    env!("CARGO_BIN_EXE_sonus-auris-sidecar")
}

struct RunningSidecar {
    child: Child,
    port: u16,
}

impl RunningSidecar {
    fn start() -> Self {
        let port = TcpListener::bind("127.0.0.1:0")
            .expect("reserve sidecar socket")
            .local_addr()
            .unwrap()
            .port();
        let child = Command::new(executable())
            .env("SONUS_AURIS_SIDECAR_BIND", format!("127.0.0.1:{port}"))
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("launch real sidecar binary");
        let mut running = Self { child, port };
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                return running;
            }
            assert!(
                running.child.try_wait().expect("check sidecar exit").is_none(),
                "sidecar exited before accepting connections"
            );
            assert!(Instant::now() < deadline, "sidecar did not bind within deadline");
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn exchange(&self, request: &str) -> String {
        exchange(self.port, request)
    }
}

impl Drop for RunningSidecar {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn exchange(port: u16, request: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to real service");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("set read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .expect("set write timeout");
    stream.write_all(request.as_bytes()).expect("send HTTP request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read HTTP response");
    response
}

fn request(path: &str) -> String {
    format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
}

#[test]
fn real_binary_serves_health_readiness_and_metrics_with_hardened_responses() {
    let server = RunningSidecar::start();
    for path in ["/healthz", "/readyz"] {
        let response = server.exchange(&request(path));
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{path}: {response}");
        assert!(response.contains("content-type: application/json"), "{path}");
        assert!(response.contains("\"ok\":true"), "{path}");
        assert!(response.contains("sonus-auris-sidecar"), "{path}");
        assert!(response.contains("cache-control: no-store"), "{path}");
        assert!(response.contains("x-content-type-options: nosniff"), "{path}");
        assert!(response.contains("content-security-policy: default-src 'none'"), "{path}");
    }
    let metrics = server.exchange(&request("/metrics"));
    assert!(metrics.starts_with("HTTP/1.1 200 OK"));
    assert!(metrics.contains("ores_otel_sidecar_up{service=\"sonus-auris-sidecar\"} 1"));

    let head = server.exchange("HEAD /readyz HTTP/1.1\r\nHost: localhost\r\n\r\n");
    assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
    let (headers, body) = head.split_once("\r\n\r\n").expect("HTTP headers");
    assert!(headers.contains("content-length: 0"));
    assert!(body.is_empty(), "HEAD must not expose a response body");
}

#[test]
fn real_binary_rejects_method_body_traversal_and_header_abuse() {
    let server = RunningSidecar::start();
    let cases = [
        ("POST /healthz HTTP/1.1\r\nHost: localhost\r\nContent-Length: 2\r\n\r\n{}", 400),
        ("GET /healthz%2e%2e HTTP/1.1\r\nHost: localhost\r\n\r\n", 404),
        ("GET /internal/retention/sweep HTTP/1.1\r\nHost: localhost\r\n\r\n", 404),
        ("GET /healthz HTTP/1.1\r\n\r\n", 400),
        ("TRACE /healthz HTTP/1.1\r\nHost: localhost\r\n\r\n", 405),
        ("GET /healthz HTTP/2.0\r\nHost: localhost\r\n\r\n", 400),
    ];
    for (request, expected) in cases {
        let response = server.exchange(request);
        assert!(
            response.starts_with(&format!("HTTP/1.1 {expected} ")),
            "expected {expected}, got {response}"
        );
        assert!(response.contains("cache-control: no-store"));
        assert!(!response.contains("\"ok\":true"));
    }

    let mut large_headers = String::from("GET /healthz HTTP/1.1\r\nHost: localhost\r\n");
    for index in 0..40 {
        large_headers.push_str(&format!("x-extra-{index}: value\r\n"));
    }
    large_headers.push_str("\r\n");
    assert!(server.exchange(&large_headers).starts_with("HTTP/1.1 431"));
}

#[test]
fn concurrent_http_clients_get_independent_product_responses() {
    let server = RunningSidecar::start();
    let port = server.port;
    let threads: Vec<_> = (0..16)
        .map(|index| {
            thread::spawn(move || {
                let path = if index % 2 == 0 { "/healthz" } else { "/readyz" };
                let response = exchange(port, &request(path));
                assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
                assert!(response.contains("\"ok\":true"));
                assert!(response.contains("sonus-auris-sidecar"));
            })
        })
        .collect();
    for client in threads {
        client.join().expect("HTTP client should succeed");
    }
}

fn fake_upstream(response: &'static [u8]) -> (u16, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("fake probe target");
    let port = listener.local_addr().unwrap().port();
    let worker = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept probe");
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("request timeout");
        let mut request = [0_u8; 512];
        let count = socket.read(&mut request).expect("read probe");
        assert!(String::from_utf8_lossy(&request[..count]).starts_with("GET /healthz HTTP/1.1"));
        socket.write_all(response).expect("send response");
    });
    (port, worker)
}

#[test]
fn probe_process_rejects_redirects_unready_services_and_malformed_http() {
    for response in [
        b"HTTP/1.1 302 Found\r\nLocation: https://example.invalid/\r\nContent-Length: 0\r\n\r\n"
            .as_slice(),
        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n".as_slice(),
        b"not-an-http-status\r\n\r\n".as_slice(),
    ] {
        let (port, worker) = fake_upstream(response);
        let output = Command::new(executable())
            .arg("probe")
            .env("SONUS_AURIS_SIDECAR_BIND", format!("127.0.0.1:{port}"))
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("invoke real kubelet probe command");
        assert_eq!(output.status.code(), Some(1), "unhealthy probe must be nonzero");
        assert!(output.stdout.is_empty(), "probes must never write to stdout");
        worker.join().expect("fake target should be contacted");
    }
}
