#!/usr/bin/env bash
# Contracts for the Pi install step shared by deploy-pi.sh and install-pi.sh:
# - a failed remote install-pi.sh fails the deploy (TOOL-13);
# - install-pi.sh never signals a marengo-pi that systemd does not own (a
#   manual/MCP bench session started as the relative `bin/marengo-pi`): it
#   waits for one already exiting and refuses while one survives.
# pgrep/pkill only see this test's children; nothing else is signalled.
# Run: ./scripts/deploy-install.test.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=deploy-lib.sh
source "${ROOT}/scripts/deploy-lib.sh"

pass=0
fail=0
ok() {
  echo "ok: $1"
  pass=$((pass + 1))
}
bad() {
  echo "FAIL: $1" >&2
  fail=$((fail + 1))
}

TMP="$(mktemp -d)"
FAKE_PID=""
cleanup() {
  if [[ -n "${FAKE_PID}" ]]; then kill "${FAKE_PID}" 2>/dev/null || true; fi
  rm -rf "${TMP}"
}
trap cleanup EXIT

# --- remote_install_pi: the remote exit status decides the deploy ---
compose_ssh() {
  echo "$*" >>"${TMP}/ssh-calls"
  return "${FAKE_SSH_STATUS}"
}
status=0
FAKE_SSH_STATUS=1 remote_install_pi joey@pi '~/marengo' 2>"${TMP}/err" || status=$?
if [[ "${status}" -ne 0 ]] && grep -q 'install-pi.sh failed' "${TMP}/err"; then
  ok "failed remote install returns nonzero with an error"
else
  bad "failed remote install returned ${status}"
fi
status=0
FAKE_SSH_STATUS=0 remote_install_pi joey@pi '~/marengo' 2>/dev/null || status=$?
if [[ "${status}" -eq 0 ]] && grep -q 'sudo -n ~/marengo/scripts/install-pi.sh' "${TMP}/ssh-calls"; then
  ok "successful remote install returns 0"
else
  bad "successful remote install returned ${status}"
fi
if grep -q 'remote_install_pi "$PI_HOST" "$REMOTE_ROOT" || exit 1' "${ROOT}/scripts/deploy-pi.sh"; then
  ok "deploy-pi.sh exits when the install fails"
else
  bad "deploy-pi.sh does not exit on a failed install"
fi
if grep -q '^if ! require_no_marengo_pi; then' "${ROOT}/scripts/install-pi.sh" &&
  ! grep -Eq 'pkill|killall|kill -' "${ROOT}/scripts/install-pi.sh"; then
  ok "install-pi.sh refuses beside a surviving marengo-pi and signals none"
else
  bad "install-pi.sh signals marengo-pi or ignores survivors"
fi

# --- require_no_marengo_pi against a real process named marengo-pi ---
if [[ "$(uname -s)" != Linux ]] || ! command -v pgrep >/dev/null || ! command -v pkill >/dev/null; then
  echo "skip: process cases need Linux procps (install-pi.sh runs on the Pi)"
else
  REAL_PGREP="$(command -v pgrep)"
  REAL_PKILL="$(command -v pkill)"
  mkdir -p "${TMP}/bin" "${TMP}/session/bin"
  cp "$(command -v sleep)" "${TMP}/session/bin/marengo-pi"
  # Scoped to this test's children. Any pkill/killall call is recorded; the
  # check must never make one.
  cat >"${TMP}/bin/pgrep" <<CLI
#!/usr/bin/env bash
exec "${REAL_PGREP}" -P "\${STOP_TEST_PARENT}" "\$@"
CLI
  cat >"${TMP}/bin/pkill" <<CLI
#!/usr/bin/env bash
echo "pkill \$*" >>"${TMP}/signals"
exec "${REAL_PKILL}" -P "\${STOP_TEST_PARENT}" "\$@"
CLI
  cat >"${TMP}/bin/killall" <<CLI
#!/usr/bin/env bash
echo "killall \$*" >>"${TMP}/signals"
exit 1
CLI
  chmod +x "${TMP}"/bin/*
  alive() {
    kill -0 "${FAKE_PID}" 2>/dev/null && [[ "$(ps -o stat= -p "${FAKE_PID}" 2>/dev/null)" != Z* ]]
  }
  # $1: how long the fake bench session runs (s).
  run_check() {
    : >"${TMP}/signals"
    (cd "${TMP}/session" && exec bin/marengo-pi "$1") &
    FAKE_PID=$!
    status=0
    (
      export STOP_TEST_PARENT=$$ PATH="${TMP}/bin:${PATH}"
      require_no_marengo_pi
    ) 2>"${TMP}/check-err" || status=$?
  }

  run_check 300
  if [[ "${status}" -ne 0 ]] && alive && [[ ! -s "${TMP}/signals" ]] &&
    grep -q 'bench session is running; stop it first' "${TMP}/check-err"; then
    ok "a live bench session refuses the install and is not signalled"
  else
    bad "live bench session: status ${status}, alive=$(alive && echo yes || echo no), signals: $(cat "${TMP}/signals")"
  fi
  kill "${FAKE_PID}" 2>/dev/null || true
  wait "${FAKE_PID}" 2>/dev/null || true
  FAKE_PID=""

  run_check 1
  if [[ "${status}" -eq 0 ]] && ! alive && [[ ! -s "${TMP}/signals" ]]; then
    ok "a marengo-pi that is already exiting is waited for"
  else
    bad "exiting marengo-pi: status ${status}, signals: $(cat "${TMP}/signals")"
  fi
  kill "${FAKE_PID}" 2>/dev/null || true
  wait "${FAKE_PID}" 2>/dev/null || true
  FAKE_PID=""
fi

echo ""
echo "deploy-install.test: ${pass} passed, ${fail} failed"
[[ "${fail}" -eq 0 ]]
