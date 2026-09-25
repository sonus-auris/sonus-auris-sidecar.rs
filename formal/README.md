# Formal refinement

This repository refines `sidecar-lifecycle-v1`.

The abstract machine covers boot, readiness, telemetry degradation, drain, stop, and boot
failure. It requires:

- requests are served only in the Ready phase;
- drain/stop/failure never accept new work;
- telemetry export failure alone does not make a healthy ready sidecar stop serving;
- shutdown removes readiness before terminal stop.

Binary probe, preflight, container-entrypoint, and trace-contract tests remain the concrete
refinement evidence.
