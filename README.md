# sonus-auris-sidecar.rs

k8s sidecar for Sonus Auris.

Inherits [`ores-otel-sidecar`](https://github.com/ores-otel/ores-otel-sidecar.rs).
Bind with `SONUS_AURIS_SIDECAR_BIND` (default `127.0.0.1:9090`).

```sh
cargo run --bin sonus-auris-sidecar
```
