# Adversarial review recipe

How the gate 2 round on the left upper arm ran on Joey-PC, Windows, from Git Bash. Copy it for any hardware gate. All four tools were already logged in, so nothing needed installing or signing in.

| Reviewer | Tool | Model flag | Read-only by | Rev B run time |
| --- | --- | --- | --- | --- |
| Sol | Codex CLI | `-m gpt-6.1-sol` | `-s read-only` | 8.5 min |
| Astra | Codex CLI | `-m gpt-6-astra` | `-s read-only` | 9 min |
| Grok | cursor-agent | `--model grok-4.7-high-fast` | `--mode ask` | 12.5 min |
| Wonder | Muse | `--model muse-wonder-1.0` | Prompt only, so check hashes after | 9 min |

Run times are from one run. Allow 20 minutes.

Always pass the model flag. Codex without `-m` did not give Sol. Grok 4.8 Fast was not available through cursor-agent, so use `grok-4.7-high-fast`. Check with `--list-models` if that changes.

Antigravity (Argon) is not on Joey-PC. It may be on the Mac, but that is untested.

## 1. Build the packet

Make a folder with everything a reviewer needs, then work from inside it.

```text
REVIEW_PACKET_<NAME>/
  PACKET.md          what it is, checks done, not done, known open items, numbered questions
  SHA256SUMS.txt     hash of every file below except reviews/
  data/              STEP, JSON, the docs the claims come from
  scripts/           anything that produced the numbers
  views/             PNG sheets
  reviews/PROMPT.txt prompt, and where the outputs land
```

Do not include earlier reviews, so each one stays independent. Do not put your own conclusion in `PACKET.md`. Put in the claims and let them check. If scripts need input files, include those too: gate 2 left out `stl/` and `cad_export.json` beside the scripts, and a reviewer could not rerun them.

```bash
find data scripts views PACKET.md -type f -print0 | xargs -0 sha256sum > SHA256SUMS.txt
```

## 2. Prompt template

Save as `reviews/PROMPT.txt`. Change the bracketed parts.

```text
You are an adversarial [mechanical] reviewer for [a humanoid robot arm]. This is a read-only review: do not create, edit, or delete any file, do not run anything that changes the system, and do not touch hardware.
Read PACKET.md in this folder, then the files in data/, scripts/ and views/. Answer its [six] questions and try to find what would make this part fail or make the test print a waste.
Check the arithmetic yourself rather than trusting it. Separate what you verified from what you are assuming. Rank findings as blocker, major, or minor, each with evidence and the smallest fix. Do not praise the design.
Return the review as your final message.
```

## 3. Run

From inside the packet folder, in Git Bash. Each runs in the background, and all four can run at once. In Claude Code, run the whole block as a background shell and wait for the notification.

The `</dev/null` matters, so that no tool waits on stdin.

```bash
sha256sum -c SHA256SUMS.txt --quiet && echo hashes ok
P="$(cat reviews/PROMPT.txt)"
M=/c/Users/joeyl/AppData/Local/Programs/muse/muse.cmd
I="-i views/SHEET_1.png -i views/SHEET_2.png"

(codex exec -m gpt-6.1-sol -s read-only --skip-git-repo-check --ephemeral $I \
  -o reviews/codex-sol.md "$P" > reviews/codex-sol.log 2>&1 </dev/null
 echo "sol exit $?" >> reviews/STATUS.txt) &

(codex exec -m gpt-6-astra -s read-only --skip-git-repo-check --ephemeral $I \
  -o reviews/codex-astra.md "$P" > reviews/codex-astra.log 2>&1 </dev/null
 echo "astra exit $?" >> reviews/STATUS.txt) &

(powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "C:/Users/joeyl/AppData/Local/cursor-agent/cursor-agent.ps1" \
  -p --mode ask --trust --workspace . --model grok-4.7-high-fast \
  --output-format text "$P" > reviews/cursor-grok.md 2> reviews/cursor-grok.log </dev/null
 echo "cursor-grok exit $?" >> reviews/STATUS.txt) &

("$M" exec --model muse-wonder-1.0 --workspace . --prompt-file reviews/PROMPT.txt \
  --image views/SHEET_1.png --image views/SHEET_2.png \
  > reviews/muse-wonder.md 2> reviews/muse-wonder.log </dev/null
 echo "muse exit $?" >> reviews/STATUS.txt) &

wait
```

| Tool | Prompt | Images | Review lands in |
| --- | --- | --- | --- |
| Codex | Argument | `-i`, repeated | `-o` file. The log holds the working trace, and can reach 500 KB |
| cursor-agent | Argument | None passed. It reads `views/` itself | stdout |
| Muse | `--prompt-file` | `--image`, repeated | stdout. The log holds startup notes |

Harmless Muse notes: a malformed `poteto-mode` skill warning, and "Agent delegation: auto unavailable: workspace is untrusted".

## 4. After the run

1. `cat reviews/STATUS.txt`: every line should say exit 0. An empty `cursor-grok.log` is normal.
2. `sha256sum -c SHA256SUMS.txt --quiet` again, to prove nothing changed. This is the only check on Muse.
3. Read all four yourself. Check each finding against the files before accepting it. At gate 2, three were rejected, including one claimed blocker, and one suggested fix was impossible.
4. Write `reviews/SUMMARY.md`: accepted, with who raised it and how you checked it; rejected, with the reason; superseded; can wait.
5. Report your own errors plainly. Publish the summary, not the raw reviews. Example: `hardware/designs/left-arm/upper-arm/GATE2_REVIEW.md`.
