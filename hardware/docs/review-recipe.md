# Adversarial review recipe

How the gate 2 round on the left upper arm ran on Joey-PC, Windows, from Git Bash. Copy it for any hardware gate. All four tools were already logged in, so nothing needed installing or signing in.

| Reviewer | Tool | Model flag | Read-only by | Rev B run time |
| --- | --- | --- | --- | --- |
| Sol | Codex CLI | `-m gpt-6.1-sol` | `-s read-only` | 8.5 min |
| Astra | Codex CLI | `-m gpt-6-astra` | `-s read-only` | 9 min |
| Fable high (replaces Grok from 2026-11-06, Joey) | Claude Code CLI, fresh session, **API credits** | `--model claude-fable-5-1 --effort high` | `-p` print mode, `--allowedTools Read,Glob,Grep,Bash(sha256sum*)` | 25 min on the subscription, rev C |
| Opus high (added 2026-11-06, Joey) | Claude Code CLI, fresh session, **API credits** | `--model claude-opus-5-5 --effort high` | same as Fable | not yet timed |
| ~~Grok~~ (dropped 2026-11-06) | cursor-agent | `--model grok-4.7-high-fast` | `--mode ask` | 12.5 min |
| Wonder | Muse | `--model muse-wonder-1.0` | Prompt only, so check hashes after | 9 min |
| Argon | Antigravity `agy` (MacBook only) | `--model gemini-4-argon-high` (always High effort, Joey 2026-11-06) | Prompt plus a read-only `--settings` allow list, check hashes after | 26.5 min (left-arm rev C) |

Run times are from one run. Allow 20 minutes.

Power board gate 3 (2026-11-06): with `Bash(sha256sum*)` only, Fable and Opus cannot rerun scripts; add `Bash(python3 *)` when the packet has scripts to rerun, and check hashes after. Without sed and awk in its allow list, Argon exited 0 with an empty review after 25 min.

Fable and Opus: run each as a fresh session with only the packet folder, never from the thread that wrote the packet, so it has no memory of the author's reasoning. Both run on the **API credits**, not the Claude subscription, at **high** effort (Joey 2026-11-06 06:33Z: "high effort for both", API route confirmed). Claude Code bills the API account whenever `ANTHROPIC_API_KEY` is set in the shell, and the subscription when it is not, so set the key for these two commands only:

```bash
# key from https://platform.claude.com/settings/keys, kept in the shell profile on the machine, never in the repo or a packet
export ANTHROPIC_API_KEY=sk-ant-...

(claude -p --model claude-fable-5-1 --effort high --allowedTools "Read,Glob,Grep,Bash(sha256sum*)" \
  --output-format json "$P" > reviews/claude-fable.json 2> reviews/claude-fable.log </dev/null
 echo "fable exit $?" >> reviews/STATUS.txt
 python -c "import json;d=json.load(open('reviews/claude-fable.json',encoding='utf-8'));open('reviews/claude-fable.md','w',encoding='utf-8').write(d['result'])") &

(claude -p --model claude-opus-5-5 --effort high --allowedTools "Read,Glob,Grep,Bash(sha256sum*)" \
  --output-format json "$P" > reviews/claude-opus.json 2> reviews/claude-opus.log </dev/null
 echo "opus exit $?" >> reviews/STATUS.txt
 python -c "import json;d=json.load(open('reviews/claude-opus.json',encoding='utf-8'));open('reviews/claude-opus.md','w',encoding='utf-8').write(d['result'])") &
```

Cost per review, from the published API prices (2026-11-06; Fable 5.1 $10 in / $50 out per million tokens, Opus 5.5 $4 / $20) and a torso-frame-sized packet (about 200k tokens read, 20 to 40k tokens of thinking and review written at high): **Fable about $3 to $4, Opus about $1.50 to $2**. Check the balance at https://platform.claude.com/settings/billing before a round; if the credits are gone, drop the key and the Fable run falls back to the subscription (Opus too). The JSON output saved beside each review carries the token usage and `total_cost_usd`: read those, not this estimate. First real data point (power board rev B, 2026-11-07, about 540k cache write, 1.16M cache read, 79k output): Fable about $12, estimated from the session transcript, so allow three times the figure above until the JSON gives a billed number.

A thread may also run it from the cloud as a fresh (not forked) agent pointed at the packet path with the same prompt; record which way it ran.

Always pass the model flag. Codex without `-m` did not give Sol. Grok 4.8 Fast was not available through cursor-agent, so use `grok-4.7-high-fast`. Check with `--list-models` if that changes.

cursor-agent launched from Git Bash can be blocked by the repo's `.cursor` pre-tool hook (`impeccable-before-edit.js` dies with a bash syntax error, every file read rejected, exit still 0, 30 s). Launch it from PowerShell with the same flags (torso frame gate, 2026-11-06). Check that the review actually read files before trusting an exit 0.

Antigravity (Argon) is on the MacBook, not Joey-PC: `agy` 1.3.4 at `~/.local/bin/agy` (checked 2026-11-06 05:40Z, signed in, `agy models` lists `gemini-4-argon-high`, a one-word smoke test answered in 7 s). Run it from the Mac against the packet. Prompt must be attached to the flag (`-p='...'`), or `-p` swallows the next flag as the prompt:

```bash
AGY_ALLOW='{"permissions":{"allow":["command(sed)","command(awk)","command(cat)","command(ls)","command(head)","command(tail)","command(wc)","command(find)","command(grep)","command(shasum)","command(sha256sum)","command(python3)","command(python)"]}}'
(agy --model gemini-4-argon-high --disable-slash-commands --print-timeout 0 \
  --add-dir "$PWD" --settings "$AGY_ALLOW" -p="$P" > reviews/antigravity-argon.md 2> reviews/antigravity-argon.log </dev/null
 echo "argon exit $?" >> reviews/STATUS.txt) &
```

agy 1.5.1 (2026-11-06) auto-denies its `command` tool in headless mode and exits 0 after ~20 s with an empty review and the log line "a tool required the command permission that headless mode cannot prompt for". The `--settings` allow list above (session only, never saved) lets it read and rerun scripts; do not use `--dangerously-skip-permissions`. Run it from a scratch `git worktree` of the packet branch, not the main clone, and check the hashes after. Left-arm rev C pass 2 run: 1587 s (26.5 min), 57 KB review.

Read-only is by prompt only (like Muse); `--mode plan` is ignored with slash commands disabled, and there is no sandbox flag that forbids writes, so check the hashes after.

On Joey-PC, agy 1.5.2 (power board rev B, 2026-11-07, six tries):

- `command(...)` allow entries match exact command lines. `command(Get-ChildItem *)` did not match `Get-ChildItem -Recurse -Name`. There is no `read` entry.
- It uses PowerShell, so any shell call can soft-deny and end the run empty with exit 0. Add this to its prompt: run no shell commands, use only file tools.
- Reading outside the workspace also ends the run. Pass every folder it needs with `--add-dir`, including `C:/Program Files/KiCad/10.0/share/kicad/footprints`.
- Run it with `--output-format stream-json --log-file <path>`. The final `result` event carries the review in `response`, plus token usage and `denied_actions`. Without it, an empty review tells you nothing.
- **Check that the review is about the packet.** That run returned 100 KB of confident findings on a design that doesn't exist, with exit 0 and matching hashes. Grep its part values and refs against the netlist before reading further, and compare its questions to `PACKET.md`.

## Reviewer audit

Joey's rules, 2026-11-07. Results go in [reviewer-scorecard.md](reviewer-scorecard.md), one row per run, keyed on model and effort.

- **Effort.** High, passed explicitly on every run. If a model has no High or close equivalent, use the provider's recommended default. The scorecard records which one ran. Codex takes `-c model_reasoning_effort=high` (tested 2026-11-07, the log prints `reasoning effort: high`). Muse takes `--reasoning-effort high`. Fable and Opus take `--effort high`. Argon's level is in its model name.
- **Time limit.** Reviewers run in parallel with 45 minutes per round. A late review counts as a miss: its findings get read later and become follow-up items. A big packet can get a longer limit if it's set up front.
- **Retries.** One retry for a real tool failure, then the reviewer sits out the round.
- **On target.** Before anyone reads a review, run `hardware/tools/review/check_on_target.py`. An off-target review scores zero. It checks the names a review cites and, with a netlist, the values it cites. Rev B's made-up Argon review passed on names alone, so a review that only passes names on a mechanical packet still needs a look against the questions.
- **Removal.** Two off-target or made-up reviews in five rounds, or more false than real findings over the last 3 rounds with no unique catches.
- **On notice.** Three time-limit misses in five rounds, or three rounds with no unique find.
- **No removal on one effort level.** First give it one trial at another level on the seeded test packet (rev B's close-order bug). Effort trials only ever run on that packet, never on real rounds.
- **Current standing.** Argon has one strike.

## Usage per reviewer

| Reviewer | Per-run figure | Where to look |
| --- | --- | --- |
| Fable, Opus | `--output-format json`: tokens and `total_cost_usd` | https://platform.claude.com/settings/billing for the billed figure |
| Argon | Tokens in the stream-json `result` event, no cost | Antigravity's Models & Usage screen. It has separate Gemini and Claude and GPT limits, weekly and 5-hour, as percentages |
| Wonder | None in stdout or the log. `muse trace inspect` not yet tried | `muse usage`, or Muse's web Usage page: percent of window and week for the whole account, which can lag by up to an hour |
| Sol, Astra | Not checked yet. Sessions are logged in `-o` and the `.log` | Not checked yet |

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

(codex exec -m gpt-6.1-sol -c model_reasoning_effort=high -s read-only --skip-git-repo-check --ephemeral $I \
  -o reviews/codex-sol.md "$P" > reviews/codex-sol.log 2>&1 </dev/null
 echo "sol exit $?" >> reviews/STATUS.txt) &

(codex exec -m gpt-6-astra -c model_reasoning_effort=high -s read-only --skip-git-repo-check --ephemeral $I \
  -o reviews/codex-astra.md "$P" > reviews/codex-astra.log 2>&1 </dev/null
 echo "astra exit $?" >> reviews/STATUS.txt) &

(powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "C:/Users/joeyl/AppData/Local/cursor-agent/cursor-agent.ps1" \
  -p --mode ask --trust --workspace . --model grok-4.7-high-fast \
  --output-format text "$P" > reviews/cursor-grok.md 2> reviews/cursor-grok.log </dev/null
 echo "cursor-grok exit $?" >> reviews/STATUS.txt) &

("$M" exec --model muse-wonder-1.0 --reasoning-effort high --workspace . --prompt-file reviews/PROMPT.txt \
  --image views/SHEET_1.png --image views/SHEET_2.png \
  > reviews/muse-wonder.md 2> reviews/muse-wonder.log </dev/null
 echo "muse exit $?" >> reviews/STATUS.txt) &

wait
```

| Tool | Prompt | Images | Review lands in |
| --- | --- | --- | --- |
| Codex | Argument | `-i`, repeated | `-o` file. The log holds the working trace, and can reach 500 KB |
| Claude Code (Fable, Opus) | Argument | None passed. It reads `views/` itself | stdout |
| cursor-agent | Argument | None passed. It reads `views/` itself | stdout |
| Muse | `--prompt-file` | `--image`, repeated | stdout. The log holds startup notes |

Harmless Muse notes: a malformed `poteto-mode` skill warning, and "Agent delegation: auto unavailable: workspace is untrusted".

## 4. After the run

1. `cat reviews/STATUS.txt`: every line should say exit 0. An empty `cursor-grok.log` is normal. Exit 0 proves nothing about the content: run `python -X utf8 -I hardware/tools/review/check_on_target.py <packet> reviews/*.md` before reading any review.
2. `sha256sum -c SHA256SUMS.txt --quiet` again, to prove nothing changed. This is the only check on Muse.
3. Read all four yourself. Check each finding against the files before accepting it. At gate 2, three were rejected, including one claimed blocker, and one suggested fix was impossible.
4. Write `reviews/SUMMARY.md`: accepted, with who raised it and how you checked it; rejected, with the reason; superseded; can wait.
5. Report your own errors plainly. Publish the summary, not the raw reviews. Example: `hardware/designs/left-arm/upper-arm/GATE2_REVIEW.md`.
