#!/usr/bin/env bash
# pi-remote.sh restart/stop-marengo-pi against a fake ssh; never contacts a Pi.
set -euo pipefail
SCRIPT="${1:-$(cd "$(dirname "$0")" && pwd)/pi-remote.sh}"
TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT
mkdir -p "${TMP}/bin" "${TMP}/ssh"
touch "${TMP}/ssh/id_ed25519_marengo"

cat > "${TMP}/bin/ssh" <<'CLI'
#!/usr/bin/env bash
printf '%s\n' "$@" > "${PI_REMOTE_TEST_STATE}/args"
cat > "${PI_REMOTE_TEST_STATE}/stdin"
CLI
chmod +x "${TMP}/bin/ssh"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

for cmd in stop-marengo-pi restart-marengo-pi; do
  mode="${cmd%-marengo-pi}"
  rm -f "${TMP}/args" "${TMP}/stdin"
  PI_REMOTE_TEST_STATE="${TMP}" MARENGO_CLOUD_SSH_DIR="${TMP}/ssh" PATH="${TMP}/bin:${PATH}" \
    bash "${SCRIPT}" "${cmd}"
  [[ "$(tail -n 4 "${TMP}/args" | tr '\n' ' ')" == "bash -s -- ${mode} " ]] ||
    fail "${cmd}: remote argv was $(tr '\n' ' ' < "${TMP}/args")"
  # TOOL-18: the ssh user cannot stop the marengo user's process; the root-owned
  # installed helper runs first through sudo -n, as in MCP pi_restart_marengo_pi.
  head -n 3 "${TMP}/stdin" | grep -qxF 'if [[ -x "$INSTALLED" ]] && sudo -n "$INSTALLED" "$1"; then' ||
    fail "${cmd}: installed helper is not tried first via sudo -n"
  grep -qxF 'INSTALLED=/usr/local/libexec/marengo/pi-restart-marengo-pi.sh' "${TMP}/stdin" ||
    fail "${cmd}: wrong installed helper path"
  grep -q '^PROC=marengo-pi$' "${TMP}/stdin" || fail "${cmd}: checkout fallback body missing"
  if grep -q '^#!' "${TMP}/stdin"; then fail "${cmd}: fallback body keeps its shebang"; fi
done

echo "pi-remote contracts ok"
