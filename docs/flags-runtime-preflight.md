# Flags/runtime preflight boundary

This repository consumes the reviewed `ores-otel-sidecar` runtime at commit
`38c30455f492b028a990b562287a067db591126f`. That upstream revision is the
same runtime consumed by the exact-head-green `flags-2-env-sidecar.rs` rollout.

`sonus-auris-sidecar preflight` validates the repository-root
`.cli-flags.toml`, argv/env resolution, and bind-address configuration without
opening the HTTP listener. It is intended for CI/CD admission and image smoke
checks before the long-running sidecar starts.

The boundary is fail closed:

- unknown command-line options are rejected;
- malformed environment-backed bind values are rejected;
- an explicit valid `--bind` option may override an invalid environment value;
- rejected option values must not be reflected in diagnostics;
- configuration failures are reported through the runtime's invalid-config path
  rather than being mislabeled as parser failures.

The production image remains distroless and continues to use the compiled
`ores-launcher`. This change does not add a shell, dotenv loader, credential,
secret default, or second option parser.
