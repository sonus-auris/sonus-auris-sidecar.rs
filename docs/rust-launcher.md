# ores-otel Rust container launcher

The default `runtime` image is still `gcr.io/distroless/cc-debian12:nonroot` and
still runs as UID/GID 65532. It now uses the shared Rust entrypoint:

```dockerfile
ENTRYPOINT ["/ores-launcher", "/sonus-auris-sidecar"]
CMD []
```

The executable is fixed by the image; arguments supplied after the image name are
forwarded to the sidecar without shell expansion or another option parser. The
launcher logs a bounded, escaped/redacted display copy through **ores-otel's Rust
Logger/LogRecord/Transport**, then replaces itself using Unix exec. The sidecar
becomes PID 1; there is no wrapper parent to forward signals or translate its exit
status. Application argument validation, signal handling and child reaping remain
application responsibilities.

## Shared source, not a copied implementation

`docker/ores-launcher.rev` is the full immutable source revision of
[ores-otel/ores.otel.log](https://github.com/ores-otel/ores.otel.log). The independent
`launcher-build` stage uses `cargo install --locked`, package
`oresoftware-next-loggers`, feature `launcher`, and binary `ores-launcher`. It
compiles natively for the target architecture against Bookworm-compatible runtime
libraries. The existing Sonus Cargo.lock and upstream sidecar pin are unchanged.

The logger package's zed-pkg identity is `oresoftware/next-loggers`, Rust target
`next-loggers-rust`; its native Cargo package is `oresoftware-next-loggers`.
This Docker build pins its source revision directly and does not invent a published
zed-pkg version for an unreleased binary. Upgrade the one revision file only after
reviewing and testing the upstream commit, then rerun this repository's image CI.

The `shell-runtime` target and `docker/entrypoint.sh` are deliberately retained for
existing shell-wrapper conformance tests, not silently added to the production
image. A direct binary override remains available:

```sh
docker run --rm --entrypoint /sonus-auris-sidecar IMAGE
```

## Telemetry and secret boundaries

The startup record is `next-loggers/v1` JSON on stderr, with
`appName=sonus-auris-sidecar`, logger name `ores-launcher`, event
`process.exec.attempt`, process PID and display argv. stdout belongs to the
application. Existing application logs keep their existing format; this change is
not a claim that every old application log has migrated.

The launcher synchronously flushes its local ores-otel transport and execs. It does
not connect to an OTLP endpoint, Supabase, or another server, initialize a global
provider, start a worker thread, or wait for remote delivery. External collection
still belongs to the deployed ores-otel pipeline. Failed log writes do not block
execution, although ordinary stderr backpressure can delay a synchronous write.

Credential-like options and URLs are redacted only in the logging copy; execution
arguments are untouched. Generic redaction cannot identify arbitrary positional
secrets. Keep credentials in the approved environment/secret-store channels and
never argv. The launcher never dumps environment values. It performs no SOPS
operation and must not replace required decryption in other application images.

## Validation and known boundary

The shared launcher has real Rust/native amd64/arm64 distroless tests for PID 1,
nonroot execution, native argv (including non-UTF8), redaction, failing stderr,
application exit status, and SIGTERM ownership.

Here, `container-entrypoint-multiarch.yml` retains ordinary Docker and amd64/arm64
Buildx builds for both runtime targets. The app's locked Cargo tests run inside the
build stage. `docker/test-rust-launcher.sh` additionally tests actual Sonus HTTP
startup, literal command records, application PID 1, UID/GID 65532, no shell, and a
missing-command error. It uses Linux host `nsenter`/`curl` to reach the private
loopback interface; those tools are not installed in the runtime image. The smoke
container has a read-only filesystem, no external network and no capabilities.

**Do not interpret these tests as a deployment or kubelet-probe certification.**
The existing `probe` command mismatch is tracked in
[issue #7](https://github.com/sonus-auris/sonus-auris-sidecar.rs/issues/7): main.rs and
the pinned sidecar runtime currently start the listener without a probe argv
branch. The launcher preserves the absolute application path and existing k8s
configuration; it does not silently fix or conceal that separate issue.

Implementation tracking: [issue #6](https://github.com/sonus-auris/sonus-auris-sidecar.rs/issues/6),
[shared PR #59](https://github.com/ores-otel/ores.otel.log/pull/59), and DEN-3175.
