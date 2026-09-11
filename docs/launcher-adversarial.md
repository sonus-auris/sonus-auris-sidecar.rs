# Launcher adversarial regression coverage

The launcher revision in `docker/ores-launcher.rev` is the only build authority.
This follow-up adopts the shared ores-otel implementation from
[ores.otel.log PR #62](https://github.com/ores-otel/ores.otel.log/pull/62), rather
than copying startup or redaction code into Sonus.

The upstream native suite reproduced and repaired three argument-display leaks,
implicit shell execution of text without a shebang, and SIGPIPE replacing a
failed-exec exit code. It retains the original argv, same-PID replacement,
canonical `next-loggers/v1` telemetry, synchronous stderr flushing, and no network
exporter. The display policy is `redacted-bounded-v2`; secrets embedded in an
apparent option name are hidden as well as its value. Unknown positional secrets
remain outside a generic redactor's guarantee and must never enter argv.

The consumer workflow runs `docker/test-launcher-adversarial.sh` against the
actual built production image on native Linux amd64 and arm64. It verifies
independent short/long credential cases, option chains, signed URLs, control
characters, ordered failure records, display count/size bounds, and executable
text rejection with exit 126 and ENOEXEC. Each adversarial Docker invocation has
a deadline and cleans up only its own named container. The text fixture is a
read-only test mount; nothing is installed in the production image.

Raw non-UTF8 executable, argument, and environment cases are exercised by the
upstream native Rust suite, not encoded through Docker's JSON API. Successful
and failed execution with broken stderr, SIGPIPE/SIGTERM ownership, PATH lookup,
and byte-preserving stdin/stdout are also upstream native-process regressions.

Existing Sonus #9 privilege/argv checks and #10 strict flags and kubelet probes
remain intact. The sidecar's separate Cargo runtime pin, application source,
SOPS boundaries, shell-runtime target and production Dockerfile are unchanged.
This is source and CI hardening, not a production deployment or a certification
of the entire Sonus observability rollout. Exact-head execution evidence belongs
in the linked pull request; do not infer a passing run from this test inventory.

Tracking: [Sonus issue #11](https://github.com/sonus-auris/sonus-auris-sidecar.rs/issues/11),
[DEN-670](https://linear.app/denman/issue/DEN-670), and upstream
[DEN-3432](https://linear.app/denman/issue/DEN-3432).
