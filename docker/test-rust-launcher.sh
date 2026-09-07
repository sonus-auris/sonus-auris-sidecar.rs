#!/usr/bin/env bash
# Linux Docker host test. nsenter uses the host curl; neither is in the image.
set -euo pipefail
image=${1:?usage: test-rust-launcher.sh IMAGE}
work=$(mktemp -d)
container=
cleanup() {
  if [[ -n "$container" ]]; then docker rm -f "$container" >/dev/null 2>&1 || true; fi
  rm -rf -- "$work"
}
trap cleanup EXIT

docker image inspect "$image" | jq -e '
  .[0].Config |
  .Entrypoint == ["/ores-launcher", "/sonus-auris-sidecar"] and
  (.Cmd // []) == [] and .User == "65532:65532"
'
flags=(--read-only --network none --cap-drop ALL --security-opt no-new-privileges)
container=$(docker run -d "${flags[@]}" "$image" 'two words' '' '*')
ready=false
for ((attempt=0; attempt<100; attempt++)); do
  test "$(docker inspect --format '{{.State.Running}}' "$container")" = true
  pid=$(docker inspect --format '{{.State.Pid}}' "$container")
  if sudo nsenter --target "$pid" --net -- curl --noproxy '*' --fail --silent \
      --max-time 1 http://127.0.0.1:9090/healthz >"$work/health"; then
    ready=true
    break
  fi
  sleep 0.1
done
test "$ready" = true

docker logs "$container" >"$work/stdout" 2>"$work/stderr"
# Existing app logs retain their own format. Require a canonical launcher record,
# not that every old app log has already migrated to the same schema.
jq -R -s -e '
  [split("\n")[] | fromjson? |
    select(.schema == "next-loggers/v1" and .fields["event.name"] == "process.exec.attempt")] |
  length == 1 and .[0].appName == "sonus-auris-sidecar" and
  .[0].fields["process.pid"] == 1 and
  .[0].fields["process.command_args"] == ["/sonus-auris-sidecar", "two words", "", "*"]
' "$work/stderr"
! grep -q 'next-loggers/v1' "$work/stdout"
# Check the actual application now occupying PID 1, without a shell in the image.
sudo cat "/proc/$pid/comm" | grep -qx 'sonus-auris-sid'
sudo awk '/^Uid:/{exit !($2 == 65532 && $3 == 65532)}' "/proc/$pid/status"
sudo awk '/^Gid:/{exit !($2 == 65532 && $3 == 65532)}' "/proc/$pid/status"
# A logging-only assertion cannot prove that exec received the original argv.
# Compare the actual application's NUL-separated bytes, including the empty arg.
printf '%s\0' /sonus-auris-sidecar 'two words' '' '*' >"$work/expected-argv"
sudo cat "/proc/$pid/cmdline" | cmp - "$work/expected-argv"
sudo cat "/proc/$pid/status" | grep -Eq '^NoNewPrivs:[[:space:]]+1$'
sudo cat "/proc/$pid/status" | grep -Eq '^CapEff:[[:space:]]+0+$'

for shell in /bin/sh /bin/bash; do
  if docker run --rm "${flags[@]}" --entrypoint "$shell" "$image" -c ':' >"$work/shell-out" 2>"$work/shell-err"; then
    printf 'unexpected shell in distroless image: %s\n' "$shell" >&2
    exit 1
  fi
  # Do not mistake a daemon/network/permission failure for proof of no shell.
  grep -Eq 'no such file|executable file not found' "$work/shell-err"
done
set +e
docker run --rm "${flags[@]}" --entrypoint /ores-launcher "$image" >"$work/missing-out" 2>"$work/missing-err"
status=$?
set -e
test "$status" -eq 64
jq -e '.schema == "next-loggers/v1" and .fields["event.name"] == "process.exec.invalid_command"' "$work/missing-err"
test ! -s "$work/missing-out"
set +e
docker run --rm "${flags[@]}" --entrypoint /ores-launcher "$image" /__ores_missing_command__ >"$work/exec-out" 2>"$work/exec-err"
status=$?
set -e
test "$status" -eq 127
jq -s -e '[.[] | select(.schema == "next-loggers/v1" and .fields["event.name"] == "process.exec.attempt")] | length == 1' "$work/exec-err"
test ! -s "$work/exec-out"
printf '%s\n' 'PASS: ores-otel record, actual native argv bytes, app PID 1, nonroot, no-new-privileges, no capabilities, HTTP startup, no sh/bash, exits 64/127'
printf '%s\n' 'Kubelet probe command behavior is not certified here; see issue #7.'
