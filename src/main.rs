#![forbid(unsafe_code)]

#[path = "../generated/rust/env.rs"]
mod env;

use ores_otel_sidecar::{cli, runtime, SidecarConfig, SidecarIdentity};

fn main() {
    let identity = SidecarIdentity::new(env::SERVICE, env::BIND);
    let invocation = match cli::resolve_process(runtime::DEFAULT_CLI_CONFIG_PATH) {
        Ok(invocation) => invocation,
        Err(_) => runtime::exit_invalid_cli(identity),
    };
    let command = invocation.command;
    let bind = invocation
        .value(env::BIND)
        .unwrap_or_else(|| env::BIND_DEFAULT.to_owned());

    let cfg = match SidecarConfig::from_bind(identity, &bind, false) {
        Ok(cfg) => cfg,
        Err(_) => runtime::exit_invalid_config(identity),
    };

    runtime::run_command(&cfg, command);
}
