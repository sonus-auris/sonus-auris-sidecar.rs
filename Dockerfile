# syntax=docker/dockerfile:1
#
# Distroless production image for sonus-auris-sidecar.
# Prefer linux/arm64:
#   docker buildx build --platform linux/arm64 -t sonus-auris-sidecar:dev .
#
# This is the ores-otel loopback probe helper (127.0.0.1:9090 — /healthz
# /readyz /metrics), NOT an OTLP collector. Pair it in the same pod as the
# app. The app exports OTLP in-process to
# dd-otel-collector.observability.svc.cluster.local:4318 (HTTP) or :4317 (gRPC).
#
# No ores-sops in the production target: secrets stay on the app container
# (env/enc + sops-entrypoint) or a k8s Secret. The shared Rust launcher emits
# an ores-otel command record and execs the app without requiring a shell.
# shell-runtime remains a separate conformance target. The launcher does not
# replace decryption or other required initialization in application images.
#
# k8s contract (see ores-otel/ores-otel-sidecar.rs/k8s/container.yaml):
#   - bind SONUS_AURIS_SIDECAR_BIND=127.0.0.1:9090 (loopback only)
#   - livenessProbe exec ["/sonus-auris-sidecar", "probe"]
#     NOTE: existing probe argv/runtime mismatch is tracked in issue #7.
#   - no readinessProbe
#   - do not publish :9090 on a Service
#   - do not EXPOSE 4317/4318

# This stage builds for the target architecture, not the host architecture.
# The reviewed upstream source revision has one authority: docker/ores-launcher.rev.
FROM rust:1.90-bookworm AS launcher-build
WORKDIR /launcher-source
COPY docker/ores-launcher.rev ./ores-launcher.rev
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=cargo-registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,id=cargo-git,sharing=locked \
    grep -Eq '^[0-9a-f]{40}$' ores-launcher.rev \
    && test "$(wc -l < ores-launcher.rev)" -eq 1 \
    && cargo install --locked \
        --git https://github.com/ores-otel/ores.otel.log.git \
        --rev "$(cat ores-launcher.rev)" \
        --features launcher --bin ores-launcher --root /launcher \
        oresoftware-next-loggers \
    && strip /launcher/bin/ores-launcher

FROM rust:1.90-bookworm AS build
ARG TARGETARCH
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=cargo-registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,id=cargo-git,sharing=locked \
    --mount=type=cache,target=/src/target,id=sonus-auris-sidecar-target-${TARGETARCH},sharing=locked \
    cargo test --all-targets --locked \
    && cargo build --release --locked --bin sonus-auris-sidecar \
    && strip "target/release/sonus-auris-sidecar" \
    && cp "target/release/sonus-auris-sidecar" "/usr/local/bin/sonus-auris-sidecar"

# Shell-capable conformance target. It is intentionally not the final/default
# target; production remains distroless below.
FROM debian:bookworm-slim AS shell-runtime
RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates \
    && apt-get clean \
    && find /var/lib/apt/lists -mindepth 1 -delete \
    && groupadd --system --gid 65532 nonroot \
    && useradd --system --uid 65532 --gid 65532 --no-create-home --shell /usr/sbin/nologin nonroot
COPY --from=build --chown=65532:65532 "/usr/local/bin/sonus-auris-sidecar" "/sonus-auris-sidecar"
COPY --chmod=0555 docker/entrypoint.sh /usr/local/bin/entrypoint.sh
ENV SONUS_AURIS_SIDECAR_BIND=127.0.0.1:9090 \
    ORES_OTEL_SIDECAR_BIND=127.0.0.1:9090 \
    OTEL_SERVICE_NAME=sonus-auris-sidecar
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["/sonus-auris-sidecar"]

FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
# Keep the app's absolute path stable for direct invocations and kubelet probes.
COPY --from=build --chown=65532:65532 "/usr/local/bin/sonus-auris-sidecar" "/sonus-auris-sidecar"
COPY --from=launcher-build --chmod=0555 /launcher/bin/ores-launcher /ores-launcher
ENV SONUS_AURIS_SIDECAR_BIND=127.0.0.1:9090 \
    ORES_OTEL_SIDECAR_BIND=127.0.0.1:9090 \
    OTEL_SERVICE_NAME=sonus-auris-sidecar
USER 65532:65532
ENTRYPOINT ["/ores-launcher", "/sonus-auris-sidecar"]
CMD []
