#![forbid(unsafe_code)]

use std::process::{Command, Output, Stdio};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_sonus-auris-sidecar")
}

fn run(args: &[&str], bind: Option<&str>) -> Output {
    let mut command = Command::new(binary());
    command
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(bind) = bind {
        command.env("SONUS_AURIS_SIDECAR_BIND", bind);
    }
    command.output().expect("run Sonus sidecar preflight")
}

#[test]
fn preflight_validates_default_and_argv_over_env_precedence_without_listening() {
    let default = run(&["preflight"], None);
    assert!(
        default.status.success(),
        "default preflight failed: {}",
        String::from_utf8_lossy(&default.stderr)
    );

    let invalid_env = run(&["preflight"], Some("not-a-socket"));
    assert!(
        !invalid_env.status.success(),
        "invalid environment bind unexpectedly passed preflight"
    );

    let argv_override = run(
        &["preflight", "--bind=127.0.0.1:19090"],
        Some("not-a-socket"),
    );
    assert!(
        argv_override.status.success(),
        "argv did not override invalid environment bind: {}",
        String::from_utf8_lossy(&argv_override.stderr)
    );
}

#[test]
fn preflight_rejects_unknown_argv_without_reflecting_values() {
    let secret = "synthetic-secret-never-reflect";
    let argument = format!("--definitely-not-declared={secret}");
    let output = run(&["preflight", &argument], None);

    assert!(
        !output.status.success(),
        "unknown option unexpectedly passed"
    );
    assert!(
        output.stdout.is_empty(),
        "invalid invocation wrote to stdout"
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains(secret),
        "rejected argv value leaked into stderr"
    );
}
