#!/usr/bin/env bash
# Fast public CLI propagation contracts; real scanner fixtures are run separately.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT
mkdir -p "${TMP}/bin" "${TMP}/workspace"
cat > "${TMP}/bin/cargo" <<'CLI'
#!/usr/bin/env bash
set -euo pipefail
case "$*" in
  'deny --version') echo 'cargo-deny test adapter'; exit 0 ;;
  'audit --version') echo 'cargo-audit test adapter'; exit 0 ;;
  *' fetch '*) stage=fetch; status=41 ;;
  *' check '*) stage=check; status=42 ;;
  'audit') stage=audit; status=43 ;;
  *) echo "unexpected command: $*" >&2; exit 99 ;;
esac
echo "${stage}" >> "${SCAN_TEST_CALLS}"
if [[ "${SCAN_TEST_STAGE}" == "${stage}" ]]; then
  echo "scanner ${stage} failed" >&2
  exit "${status}"
fi
CLI
chmod +x "${TMP}/bin/cargo"

run_case() {
  local stage="$1" expected="$2" expected_calls="$3"
  local status=0
  local calls="${TMP}/${stage}.calls"
  SCAN_TEST_STAGE="${stage}" SCAN_TEST_CALLS="${calls}" PATH="${TMP}/bin:${PATH}" \
    bash "${ROOT}/scripts/check-dependencies.sh" "${TMP}/workspace" \
      > "${TMP}/${stage}.log" 2>&1 || status=$?
  [[ "${status}" -eq "${expected}" ]] || {
    cat "${TMP}/${stage}.log" >&2
    echo "expected exit ${expected}, got ${status} for ${stage}" >&2
    exit 1
  }
  [[ "$(paste -sd, "${calls}")" == "${expected_calls}" ]] || {
    echo "unexpected downstream scanner calls for ${stage}" >&2
    exit 1
  }
}

run_case fetch 41 fetch
run_case check 42 fetch,check
run_case audit 43 fetch,check,audit
run_case success 0 fetch,check,audit
echo "dependency gate propagation: 4 passed"
