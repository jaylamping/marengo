#!/usr/bin/env bash
# Exercise the real entrypoint against a fake kernel CLI; never touches CAN.
set -euo pipefail
SCRIPT="${1:-$(cd "$(dirname "$0")" && pwd)/vcan-up.sh}"
TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT
mkdir -p "${TMP}/bin"

cat > "${TMP}/bin/uname" <<'CLI'
#!/usr/bin/env bash
echo Linux
CLI
cat > "${TMP}/bin/modprobe" <<'CLI'
#!/usr/bin/env bash
echo "modprobe $*" >> "${VCAN_TEST_STATE}/calls"
if [[ "${VCAN_TEST_CASE}" == loaded ]]; then exit 0; fi
echo 'modprobe: module database unavailable' >&2
exit 1
CLI
cat > "${TMP}/bin/ip" <<'CLI'
#!/usr/bin/env bash
set -euo pipefail
echo "ip $*" >> "${VCAN_TEST_STATE}/calls"
if [[ "$1" == -details ]]; then
  echo "$4: virtual interface is up"
  exit 0
fi
case "$2" in
  show) [[ -f "${VCAN_TEST_STATE}/$3" ]] ;;
  add)
    iface="$4"
    if [[ "${#iface}" -gt 15 ]]; then
      echo 'Error: argument is wrong: name too long' >&2
      exit 1
    fi
    if [[ "${iface}" != vcan0 && "${iface}" != vcan1 ]]; then
      case "${VCAN_TEST_CASE}" in
        unsupported) echo 'RTNETLINK answers: Operation not supported' >&2; exit 1 ;;
        collision)
          touch "${VCAN_TEST_STATE}/${iface}" "${VCAN_TEST_STATE}/foreign-probe"
          echo "${iface}" > "${VCAN_TEST_STATE}/foreign-name"
          echo 'RTNETLINK answers: File exists' >&2
          exit 1 ;;
      esac
      echo "${iface}" > "${VCAN_TEST_STATE}/created-probe"
      echo "${iface}" >> "${VCAN_TEST_NAMES}"
    fi
    touch "${VCAN_TEST_STATE}/${iface}"
    if [[ "${VCAN_TEST_CASE}" == creation_interrupted ]]; then kill -TERM "${PPID}"; fi
    ;;
  del)
    iface="$4"
    [[ -f "${VCAN_TEST_STATE}/${iface}" ]] || exit 90
    if [[ "${VCAN_TEST_CASE}" == cleanup_failure || "${VCAN_TEST_CASE}" == interrupted ]]; then
      if [[ ! -f "${VCAN_TEST_STATE}/first-delete" ]]; then
        touch "${VCAN_TEST_STATE}/first-delete"
        echo 'RTNETLINK answers: temporary deletion failure' >&2
        if [[ "${VCAN_TEST_CASE}" == interrupted ]]; then kill -TERM "${PPID}"; fi
        exit 1
      fi
    fi
    rm "${VCAN_TEST_STATE}/${iface}"
    ;;
  set)
    [[ -f "${VCAN_TEST_STATE}/$4" ]] || exit 91
    touch "${VCAN_TEST_STATE}/$4-up"
    ;;
  *) echo "unexpected ip command: $*" >&2; exit 92 ;;
esac
CLI
chmod +x "${TMP}/bin/"*

require() {
  if ! "$@"; then
    cat "${STATE}/output" >&2
    cat "${STATE}/calls" >&2
    echo "vcan-up contract failed: ${CASE}: $*" >&2
    exit 1
  fi
}

run_case() {
  CASE="$1"
  STATE="${TMP}/${CASE}"
  mkdir -p "${STATE}"
  : > "${STATE}/calls"
  if [[ "${CASE}" == existing ]]; then touch "${STATE}/vcan0" "${STATE}/vcan1"; fi
  local status=0
  VCAN_TEST_CASE="${CASE}" VCAN_TEST_STATE="${STATE}" VCAN_TEST_NAMES="${TMP}/names" \
    PATH="${TMP}/bin:${PATH}" \
    bash "${SCRIPT}" > "${STATE}/output" 2>&1 || status=$?
  case "${CASE}" in
    fallback|loaded|existing)
      require test "${status}" -eq 0
      require test -f "${STATE}/vcan0-up"
      require test -f "${STATE}/vcan1-up"
      ;;
    unsupported)
      require test "${status}" -eq 1
      require grep -Fq 'modprobe: module database unavailable' "${STATE}/output"
      require grep -Fq 'RTNETLINK answers: Operation not supported' "${STATE}/output"
      require test ! -f "${STATE}/vcan0"
      require test ! -f "${STATE}/vcan1"
      require test "$(grep -c 'ip link del ' "${STATE}/calls" || true)" -eq 0
      ;;
    collision)
      require test "${status}" -eq 1
      require test -f "${STATE}/foreign-probe"
      require test -f "${STATE}/$(cat "${STATE}/foreign-name")"
      require grep -Fq 'RTNETLINK answers: File exists' "${STATE}/output"
      require test "$(grep -c 'ip link del ' "${STATE}/calls" || true)" -eq 0
      ;;
    cleanup_failure|interrupted|creation_interrupted)
      local expected=1
      if [[ "${CASE}" != cleanup_failure ]]; then expected=143; fi
      require test "${status}" -eq "${expected}"
      local deletes=2
      if [[ "${CASE}" == creation_interrupted ]]; then
        deletes=1
      else
        require grep -Fq 'temporary deletion failure' "${STATE}/output"
      fi
      require test "$(grep -c 'ip link del ' "${STATE}/calls" || true)" -eq "${deletes}"
      ;;
  esac
  if [[ -f "${STATE}/created-probe" ]]; then
    local probe
    probe="$(cat "${STATE}/created-probe")"
    require test "${#probe}" -le 15
    require test ! -f "${STATE}/${probe}"
  fi
  if [[ "${CASE}" == loaded || "${CASE}" == existing ]]; then
    require test ! -f "${STATE}/created-probe"
  fi
  if [[ "${CASE}" == existing ]]; then
    require test "$(grep -c '^modprobe ' "${STATE}/calls" || true)" -eq 0
    require test "$(grep -Ec 'ip link (add|del) ' "${STATE}/calls" || true)" -eq 0
  fi
  echo "vcan-up ${CASE}: passed"
}

cases=("${@:2}")
if [[ "${#cases[@]}" -eq 0 ]]; then
  cases=(fallback unsupported collision cleanup_failure interrupted creation_interrupted loaded existing)
fi
for scenario in "${cases[@]}"; do run_case "${scenario}"; done
if [[ -f "${TMP}/names" && -n "$(sort "${TMP}/names" | uniq -d)" ]]; then
  echo 'vcan-up contract failed: probe names must differ between invocations' >&2
  exit 1
fi
echo "vcan-up entrypoint contracts: ${#cases[@]} passed"
