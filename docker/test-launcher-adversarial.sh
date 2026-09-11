#!/usr/bin/env bash
# Host-side Docker tests; no shell or diagnostic utility is installed in the image.
set -euo pipefail
image=${1:?usage: test-launcher-adversarial.sh IMAGE}
work=$(mktemp -d)
prefix="launcher-adversarial-$(basename "$work")"
active=
case_number=0
cleanup() {
  if [[ -n "$active" ]]; then docker rm -f "$active" >/dev/null 2>&1 || true; fi
  rm -rf -- "$work"
}
trap cleanup EXIT
flags=(--read-only --network none --cap-drop ALL --security-opt no-new-privileges)

missing_command() {
  case_number=$((case_number + 1))
  active="$prefix-$case_number"
  set +e
  timeout --kill-after=5s 15s docker run --rm --name "$active" "${flags[@]}" \
    --entrypoint /ores-launcher "$image" /__ores_missing_command__ "$@" \
    >"$work/stdout" 2>"$work/stderr"
  status=$?
  set -e
  test "$status" -eq 127
  active=
  test ! -s "$work/stdout"
  jq -s -e '
    length == 2 and
    all(.[]; .schema == "next-loggers/v1" and
      .fields["launcher.log_policy"] == "redacted-bounded-v2" and
      .fields["process.pid"] == 1) and
    .[0].fields["event.name"] == "process.exec.attempt" and
    .[1].fields["event.name"] == "process.exec.failed" and
    .[1].fields["error.kind"] == "NotFound"
  ' "$work/stderr" >/dev/null
  ! grep -q 'synthetic-.*needle' "$work/stderr"
}

# Synthetic values only. Test each form independently so an earlier option's
# redact-next state cannot accidentally hide a later redaction defect.
for alias in -p -P -k -u -H; do
  missing_command "${alias}synthetic-token-needle"
done
missing_command '--token:synthetic-needle'
missing_command '--passwordsynthetic-needle'
missing_command '--token-synthetic-needle=value'
missing_command --token synthetic-opaque-needle
missing_command --token --password synthetic-chain-needle
missing_command 'https://example.invalid/synthetic-url-needle?signature=fixture'
missing_command $'line one\nline two\r\t\033[31m"quoted"'

# Display limits must remain explicit; JSON events remain parseable and bounded.
arguments=()
printf -v padding '%0256d' 0
for ((index=0; index<64; index++)); do arguments+=("$index:$padding"); done
missing_command "${arguments[@]}"
jq -s -e 'all(.[];
  .fields["process.command_args_count"] == 65 and
  (.fields["process.command_args"] | length) == 33 and
  .fields["process.command_args"][32] == "[33 arguments omitted]")' \
  "$work/stderr" >/dev/null
test "$(wc -c < "$work/stderr")" -lt 32768

# Mount only this test-owned executable text. The production image is unchanged.
# Direct execve must report ENOEXEC (Linux 8), not try an absent /bin/sh (ENOENT).
printf 'exit 23\n' >"$work/no-shebang"
chmod 0555 "$work/no-shebang"
active="$prefix-text"
set +e
timeout --kill-after=5s 15s docker run --rm --name "$active" "${flags[@]}" \
  --mount "type=bind,src=$work/no-shebang,dst=/launcher-text,readonly" \
  --entrypoint /ores-launcher "$image" /launcher-text \
  >"$work/text-out" 2>"$work/text-err"
status=$?
set -e
test "$status" -eq 126
active=
test ! -s "$work/text-out"
jq -s -e 'length == 2 and
  .[0].fields["event.name"] == "process.exec.attempt" and
  .[1].fields["event.name"] == "process.exec.failed" and
  .[1].fields["error.os_code"] == 8' "$work/text-err" >/dev/null
printf '%s\n' 'PASS: redaction v2, ordered failure telemetry, bounded JSON, no implicit shell, exits 126/127'
