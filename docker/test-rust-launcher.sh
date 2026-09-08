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

# The sidecar now owns a strict flags-2-env CLI. Start the long-running server
# with one valid argument; arbitrary operands are verified separately below.
server_arg='--bind=127.0.0.1:9090'
container=$(docker run -d "${flags[@]}" "$image" "$server_arg")
ready=false
for ((attempt=0; attempt<100; attempt++)); do
  if [[ "$(docker inspect --format '{{.State.Running}}' "$container")" != true ]]; then
    docker logs "$container" >"$work/early-stdout" 2>"$work/early-stderr" || true
    cat "$work/early-stderr" >&2
    exit 1
  fi
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
jq -R -s -e --arg server_arg "$server_arg" '
  [split("\n")[] | fromjson? |
    select(.schema == "next-loggers/v1" and .fields["event.name"] == "process.exec.attempt")] |
  length == 1 and .[0].appName == "sonus-auris-sidecar" and
  .[0].fields["process.pid"] == 1 and
  .[0].fields["process.command_args"] == ["/sonus-auris-sidecar", $server_arg]
' "$work/stderr"
! grep -q 'next-loggers/v1' "$work/stdout"
# Check the actual application now occupying PID 1, without a shell in the image.
sudo cat "/proc/$pid/comm" | grep -qx 'sonus-auris-sid'
sudo awk '/^Uid:/{exit !($2 == 65532 && $3 == 65532)}' "/proc/$pid/status"
sudo awk '/^Gid:/{exit !($2 == 65532 && $3 == 65532)}' "/proc/$pid/status"
printf '%s\0' /sonus-auris-sidecar "$server_arg" >"$work/expected-server-argv"
sudo cat "/proc/$pid/cmdline" | cmp - "$work/expected-server-argv"
sudo cat "/proc/$pid/status" | grep -Eq '^NoNewPrivs:[[:space:]]+1$'
sudo cat "/proc/$pid/status" | grep -Eq '^CapEff:[[:space:]]+0+$'

# Prove byte-for-byte launcher argv preservation, including an empty operand and
# a literal wildcard, while also proving the strict application rejects those
# undeclared operands instead of silently starting another listener.
set +e
docker run --rm "${flags[@]}" "$image" 'two words' '' '*' \
  >"$work/rejected-stdout" 2>"$work/rejected-stderr"
status=$?
set -e
test "$status" -eq 2
jq -R -s -e '
  [split("\n")[] | fromjson? |
    select(.schema == "next-loggers/v1" and .fields["event.name"] == "process.exec.attempt")] |
  length == 1 and
  .[0].fields["process.command_args"] == ["/sonus-auris-sidecar", "two words", "", "*"]
' "$work/rejected-stderr"
test ! -s "$work/rejected-stdout"

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
printf '%s\n' 'PASS: ores-otel record, strict app argv, rejected arbitrary argv bytes, app PID 1, nonroot, no-new-privileges, no capabilities, HTTP startup, no sh/bash, exits 2/64/127'
printf '%s\n' 'Kubelet probe behavior is certified separately by the workflow docker-exec matrix.'
