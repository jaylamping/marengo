#!/usr/bin/env bash
# Run the restart helper against a fake systemctl and a real process named
# marengo-pi launched by a relative path, as MCP sessions launch it (TOOL-07).
# pgrep/pkill only see this test's children; nothing else is signalled.
set -euo pipefail
SCRIPT="${1:-$(cd "$(dirname "$0")" && pwd)/pi-restart-marengo-pi.sh}"

if [[ "$(uname -s)" != Linux ]] || ! command -v pgrep >/dev/null || ! command -v pkill >/dev/null ||
  ! command -v ps >/dev/null; then
  echo "skip: pi-restart-marengo-pi.test.sh needs Linux procps (the helper runs on the Pi)"
  exit 0
fi

REAL_PGREP="$(command -v pgrep)"
REAL_PKILL="$(command -v pkill)"
TMP="$(mktemp -d)"
FAKE_PID=""
cleanup() {
  if [[ -n "${FAKE_PID}" ]]; then kill "${FAKE_PID}" 2>/dev/null || true; fi
  rm -rf "${TMP}"
}
trap cleanup EXIT
mkdir -p "${TMP}/bin" "${TMP}/session/bin"
cp "$(command -v sleep)" "${TMP}/session/bin/marengo-pi"

cat > "${TMP}/bin/sudo" <<'CLI'
#!/usr/bin/env bash
[[ "$1" == -n ]] && shift
exec "$@"
CLI
cat > "${TMP}/bin/systemctl" <<'CLI'
#!/usr/bin/env bash
echo "systemctl $*" >> "${RESTART_TEST_STATE}/calls"
case "$1" in
  is-active)
    if [[ -f "${RESTART_TEST_STATE}/started" ]]; then
      [[ "${2:-}" == --quiet ]] || echo active
      exit 0
    fi
    [[ "${2:-}" == --quiet ]] || echo inactive
    exit 3 ;;
  start) touch "${RESTART_TEST_STATE}/started" ;;
  stop | cat) ;;
esac
CLI
cat > "${TMP}/bin/pgrep" <<CLI
#!/usr/bin/env bash
exec "${REAL_PGREP}" -P "${RESTART_TEST_PARENT:-$$}" "\$@"
CLI
cat > "${TMP}/bin/pkill" <<CLI
#!/usr/bin/env bash
if [[ "\${RESTART_TEST_CASE}" == unkillable ]]; then exit 0; fi
exec "${REAL_PKILL}" -P "${RESTART_TEST_PARENT:-$$}" "\$@"
CLI
chmod +x "${TMP}"/bin/*

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

run_case() {
  local case_name="$1" mode="$2"
  rm -rf "${TMP}/state"
  mkdir -p "${TMP}/state"
  (cd "${TMP}/session" && exec bin/marengo-pi 300) &
  FAKE_PID=$!
  status=0
  RESTART_TEST_STATE="${TMP}/state" RESTART_TEST_CASE="${case_name}" RESTART_TEST_PARENT=$$ \
    PATH="${TMP}/bin:${PATH}" bash "${SCRIPT}" "${mode}" >"${TMP}/out" 2>&1 || status=$?
}

alive() {
  kill -0 "${FAKE_PID}" 2>/dev/null && [[ "$(ps -o stat= -p "${FAKE_PID}" 2>/dev/null)" != Z* ]]
}

# 1. stop kills a marengo-pi started as the relative bin/marengo-pi.
run_case normal stop
[[ "${status}" == 0 ]] || { cat "${TMP}/out" >&2; fail "stop exited ${status}"; }
alive && fail "stop left the relative-path marengo-pi running"
grep -q "systemctl start" "${TMP}/state/calls" && fail "stop started the unit"
wait "${FAKE_PID}" 2>/dev/null || true
FAKE_PID=""

# 2. restart kills it, then starts the unit.
run_case normal restart
[[ "${status}" == 0 ]] || { cat "${TMP}/out" >&2; fail "restart exited ${status}"; }
alive && fail "restart left the relative-path marengo-pi running"
grep -q "systemctl start marengo-pi.service" "${TMP}/state/calls" || fail "restart did not start the unit"
wait "${FAKE_PID}" 2>/dev/null || true
FAKE_PID=""

# 3. a marengo-pi that survives the stop blocks the start and fails the helper.
run_case unkillable restart
[[ "${status}" != 0 ]] || fail "restart succeeded with a surviving marengo-pi"
grep -q "systemctl start" "${TMP}/state/calls" && fail "restart started a second owner beside a survivor"
grep -q "still running" "${TMP}/out" || fail "missing survivor error"
kill "${FAKE_PID}" 2>/dev/null || true
wait "${FAKE_PID}" 2>/dev/null || true
FAKE_PID=""

run_case unkillable stop
[[ "${status}" != 0 ]] || fail "stop reported success with a surviving marengo-pi"

echo "pi-restart-marengo-pi contracts ok"
