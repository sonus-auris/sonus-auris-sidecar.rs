# Native launcher regression certification

Extends [the existing launcher contract](rust-launcher.md) without changing the
launcher source pin, Dockerfile, application code, bindings or probe paths.

The ordinary Docker build and actual Sonus HTTP startup checks now execute on
native Linux amd64 **and** arm64 runners. Existing Buildx multiarchitecture OCI
exports for both runtime and shell-runtime remain in place.

`docker/test-rust-launcher.sh` compares `/proc/<host-pid>/cmdline` against exact
NUL-separated expected argv bytes. This proves the executed application receives
spaces, the empty argument and a literal `*`, rather than merely trusting the
launcher's display log. It also checks the application's effective capabilities
are zero and no-new-privileges survives exec, in addition to the existing actual
PID 1, UID/GID 65532 and loopback HTTP checks.

Shell absence requires an explicit missing-executable error for both sh and bash;
an unrelated daemon error no longer passes the negative check. Empty launcher
invocation must exit 64; a nonexistent executable must exit 127. Both stay off
stdout and use canonical ores-otel stderr records.

Host-only tools read procfs and enter the network namespace; they are not copied
into the image. These are container regression tests, not a claim that kubelet
probe issue #7 is resolved or that the application has certified graceful drain.
No production deployment or secret configuration is changed.
