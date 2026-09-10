#![forbid(unsafe_code)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_sonus-auris-sidecar")
}

fn reserve_loopback_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve loopback port");
    listener.local_addr().expect("reserved address").port()
}

fn run(args: &[&str], bind: &str) -> Output {
    Command::new(binary())
        .args(args)
        .env("SONUS_AURIS_SIDECAR_BIND", bind)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run Sonus sidecar command")
}

fn start_sidecar(port: u16) -> Child {
    Command::new(binary())
        .env("SONUS_AURIS_SIDECAR_BIND", format!("127.0.0.1:{port}"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start Sonus sidecar")
}

fn wait_until_listening(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("Sonus sidecar did not listen on the reserved port");
}

fn assert_quiet_success(output: &Output) {
    assert!(
        output.status.success(),
        "unexpected failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty(), "probe wrote to stdout");
}

#[test]
fn kubelet_probe_commands_hit_the_live_product_binary() {
    let port = reserve_loopback_port();
    let bind = format!("127.0.0.1:{port}");
    let mut server = start_sidecar(port);
    wait_until_listening(port);

    for command in ["probe", "probe-healthz", "probe-readyz"] {
        assert_quiet_success(&run(&[command], &bind));
    }

    let _ = server.kill();
    let _ = server.wait();
}

#[test]
fn unhealthy_refused_and_timed_out_probes_fail_closed() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind unhealthy fixture");
    let port = listener.local_addr().expect("fixture address").port();
    let unhealthy = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept unhealthy probe");
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request);
        stream
            .write_all(
                b"HTTP/1.1 503 Service Unavailable\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
            )
            .expect("write unhealthy response");
    });
    let output = run(&["probe"], &format!("127.0.0.1:{port}"));
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    unhealthy.join().expect("join unhealthy fixture");

    let refused_port = reserve_loopback_port();
    let refused = run(&["probe"], &format!("127.0.0.1:{refused_port}"));
    assert_eq!(refused.status.code(), Some(1));
    assert!(refused.stdout.is_empty());

    let timeout_listener = TcpListener::bind("127.0.0.1:0").expect("bind timeout fixture");
    let timeout_port = timeout_listener
        .local_addr()
        .expect("timeout address")
        .port();
    let timeout = thread::spawn(move || {
        let (mut stream, _) = timeout_listener.accept().expect("accept timeout probe");
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request);
        thread::sleep(Duration::from_millis(2_200));
    });
    let started = Instant::now();
    let output = run(&["probe"], &format!("127.0.0.1:{timeout_port}"));
    assert_eq!(output.status.code(), Some(1));
    assert!(started.elapsed() < Duration::from_secs(4));
    timeout.join().expect("join timeout fixture");
}

#[test]
fn invalid_cli_and_bind_values_do_not_fall_back_to_server_startup() {
    let argument_secret = "Authorization=Bearer-synthetic-argv-secret";
    let unknown = run(&[argument_secret], "127.0.0.1:19090");
    assert_eq!(unknown.status.code(), Some(2));
    assert!(unknown.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&unknown.stderr).contains(argument_secret));

    let bind_secret = "not-a-bind-Bearer-synthetic-env-secret";
    let invalid_bind = run(&[], bind_secret);
    assert_eq!(invalid_bind.status.code(), Some(1));
    assert!(invalid_bind.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&invalid_bind.stderr).contains(bind_secret));
}
