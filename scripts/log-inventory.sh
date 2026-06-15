#!/usr/bin/env bash
# Emit tracing/println inventory for logging audit reviews.
# Informational in check.sh; also runnable standalone.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

FORMAT="${LOG_INVENTORY_FORMAT:-text}"

emit_text() {
  echo "==> log inventory (tracing macros by file)"
  rg -c '(info|warn|error|debug|trace)!' --glob '*.rs' 2>/dev/null \
    | sort -t: -k2 -nr \
    || true

  echo ""
  echo "==> println/eprintln in bins"
  rg -c '(println|eprintln)!' --glob 'bins/**/*.rs' 2>/dev/null \
    | sort -t: -k2 -nr \
    || true

  echo ""
  echo "==> println/eprintln in crates (should be zero)"
  CRATE_PRINTS="$(rg -c '(println|eprintln)!' --glob 'crates/**/*.rs' 2>/dev/null || true)"
  if [[ -n "${CRATE_PRINTS}" ]]; then
    echo "${CRATE_PRINTS}"
  else
    echo "(none)"
  fi

  echo ""
  echo "==> tracing init paths in bins"
  rg -n 'init_tracing|init_subscriber' bins/ --glob '*.rs' 2>/dev/null || true

  echo ""
  echo "==> crates with tracing dep but no macros"
  for cargo in crates/*/Cargo.toml; do
    crate_dir="$(dirname "${cargo}")"
    if rg -q 'tracing' "${cargo}" 2>/dev/null \
      && ! rg -q '(info|warn|error|debug|trace)!' "${crate_dir}" --glob '*.rs' 2>/dev/null; then
      echo "${crate_dir}"
    fi
  done
}

emit_json() {
  python3 - <<'PY'
import json, re, subprocess, pathlib

root = pathlib.Path(".")
levels = ("info", "warn", "error", "debug", "trace")

def rg_count(pattern, glob):
    try:
        out = subprocess.check_output(
            ["rg", "-c", pattern, "--glob", glob],
            text=True,
            cwd=root,
        )
    except subprocess.CalledProcessError:
        return {}
    counts = {}
    for line in out.strip().splitlines():
        path, count = line.rsplit(":", 1)
        counts[path] = int(count)
    return counts

tracing_by_file = rg_count(r"(info|warn|error|debug|trace)!", "*.rs")
println_bins = rg_count(r"(println|eprintln)!", "bins/**/*.rs")
println_crates = rg_count(r"(println|eprintln)!", "crates/**/*.rs")

init_paths = []
try:
    out = subprocess.check_output(
        ["rg", "-n", r"init_tracing|init_subscriber", "bins/", "--glob", "*.rs"],
        text=True,
        cwd=root,
    )
    init_paths = [line.strip() for line in out.strip().splitlines()]
except subprocess.CalledProcessError:
    pass

silent_tracing_crates = []
for cargo in sorted((root / "crates").glob("*/Cargo.toml")):
    text = cargo.read_text()
    crate_dir = cargo.parent
    if "tracing" not in text:
        continue
    has_macros = any(
        re.search(rf"{lvl}!", p.read_text())
        for p in crate_dir.rglob("*.rs")
    )
    if not has_macros:
        silent_tracing_crates.append(str(crate_dir.relative_to(root)))

report = {
    "tracing_macro_files": len(tracing_by_file),
    "tracing_macro_total": sum(tracing_by_file.values()),
    "tracing_by_file": dict(sorted(tracing_by_file.items(), key=lambda kv: -kv[1])),
    "println_bins": println_bins,
    "println_crates": println_crates,
    "init_paths": init_paths,
    "silent_tracing_crates": silent_tracing_crates,
}
print(json.dumps(report, indent=2))
PY
}

case "${FORMAT}" in
  json) emit_json ;;
  *) emit_text ;;
esac
