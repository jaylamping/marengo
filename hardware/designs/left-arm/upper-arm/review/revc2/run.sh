#!/bin/bash
sha256sum -c SHA256SUMS.txt --quiet && echo hashes ok
P="$(cat reviews/PROMPT.txt)"
M=/c/Users/joeyl/AppData/Local/Programs/muse/muse.cmd
I="-i views/elbow_compaction_study.png -i views/04_elbow_90_135.png"
t() { n=$1; s=$(date +%s); shift; "$@"; rc=$?; echo "$n exit $rc $(( $(date +%s)-s )) s" >> reviews/STATUS.txt; }
(t sol codex exec -m gpt-6.1-sol -c model_reasoning_effort=high -s read-only --skip-git-repo-check --ephemeral $I -o reviews/codex-sol.md "$P" > reviews/codex-sol.log 2>&1 </dev/null) &
(t astra codex exec -m gpt-6-astra -c model_reasoning_effort=high -s read-only --skip-git-repo-check --ephemeral $I -o reviews/codex-astra.md "$P" > reviews/codex-astra.log 2>&1 </dev/null) &
(t muse "$M" exec --model muse-wonder-1.0 --workspace . --prompt-file reviews/PROMPT.txt --image views/elbow_compaction_study.png --image views/04_elbow_90_135.png > reviews/muse-wonder.md 2> reviews/muse-wonder.log </dev/null) &
(t fable claude -p --model claude-fable-5-1 --effort high --allowedTools "Read,Glob,Grep,Bash(sha256sum*)" --output-format text "$P" > reviews/claude-fable.md 2> reviews/claude-fable.log </dev/null) &
wait
sha256sum -c SHA256SUMS.txt --quiet && echo hashes still ok
