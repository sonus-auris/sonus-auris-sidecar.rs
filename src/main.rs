#![forbid(unsafe_code)]

#[path = "../generated/rust/env.rs"]
mod env;
#[path = "../generated/rust/runtime.rs"]
mod env_runtime;

use ores_otel_sidecar::{cli, runtime, SidecarConfig, SidecarIdentity};

fn main() {
    let identity = SidecarIdentity::new(env::SERVICE, env::BIND);
    let invocation = match cli::resolve_process(cli::DEFAULT_CONFIG_PATH) {
        Ok(invocation) => invocation,
        Err(_) => runtime::exit_invalid_cli(identity),
    };
    let command = invocation.command;
    let values = env_runtime::load_from(|key| invocation.value(key));
    let cfg = match SidecarConfig::from_bind(identity, &values.bind, false) {
        Ok(cfg) => cfg,
        Err(_) => runtime::exit_invalid_cli(identity),
    };
    runtime::run_command(&cfg, command);
}
