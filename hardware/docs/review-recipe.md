# Adversarial review recipe

How the gate 2 round on the left upper arm ran on Joey-PC, Windows, from Git Bash. Copy it for any hardware gate. All four tools were already logged in, so nothing needed installing or signing in.

| Reviewer | Tool | Model flag | Read-only by | Rev B run time |
| --- | --- | --- | --- | --- |
| Sol | Codex CLI | `-m gpt-6.1-sol` | `-s read-only` | 8.5 min |
| Astra | Codex CLI | `-m gpt-6-astra` | `-s read-only` | 9 min |
| Fable xhigh (replaces Grok from 2026-11-06, Joey) | Claude Code CLI, fresh session, **API credits** | `--model claude-fable-5-1 --effort xhigh` | `-p` print mode, `--allowedTools Read,Glob,Grep,Bash(sha256sum*)` | not yet timed |
| Opus xhigh (added 2026-11-06, Joey) | Claude Code CLI, fresh session, **API credits** | `--model claude-opus-5-5 --effort xhigh` | same as Fable | not yet timed |
| ~~Grok~~ (dropped 2026-11-06) | cursor-agent | `--model grok-4.7-high-fast` | `--mode ask` | 12.5 min |
| Wonder | Muse | `--model muse-wonder-1.0` | Prompt only, so check hashes after | 9 min |
| Argon | Antigravity `agy` (MacBook only) | `--model gemini-4-argon-high` (always High effort, Joey 2026-11-06) | Prompt only, so check hashes after | not yet timed |

Run times are from one run. Allow 20 minutes.

Fable and Opus: run each as a fresh session with only the packet folder, never from the thread that wrote the packet, so it has no memory of the author's reasoning. Both run on the **API credits**, not the Claude subscription (Joey 2026-11-06 06:17Z: "an Extra High Fable and Extra High Opus review from the free api credits, as long as we have them available"). Claude Code bills the API account whenever `ANTHROPIC_API_KEY` is set in the shell, and the subscription when it is not, so set the key for these two commands only:

```bash
# key from https://platform.claude.com/settings/keys, kept in the shell profile on the machine, never in the repo or a packet
export ANTHROPIC_API_KEY=sk-ant-...

(claude -p --model claude-fable-5-1 --effort xhigh --allowedTools "Read,Glob,Grep,Bash(sha256sum*)" \
  --output-format text "$P" > reviews/claude-fable.md 2> reviews/claude-fable.log </dev/null
 echo "fable exit $?" >> reviews/STATUS.txt) &

(claude -p --model claude-opus-5-5 --effort xhigh --allowedTools "Read,Glob,Grep,Bash(sha256sum*)" \
  --output-format text "$P" > reviews/claude-opus.md 2> reviews/claude-opus.log </dev/null
 echo "opus exit $?" >> reviews/STATUS.txt) &
```

Cost per review, from the published API prices (2026-11-06; Fable 5.1 $10 in / $50 out per million tokens, Opus 5.5 $4 / $20) and a torso-frame-sized packet (about 200k tokens read, 30 to 60k tokens of thinking and review written at xhigh): **Fable about $3 to $6, Opus about $2 to $3**. Check the balance at https://platform.claude.com/settings/billing before a round; if the credits are gone, drop the key and the Fable run falls back to the subscription (Opus too). `claude -p` has no run-time cost print, so read the token usage from the console's usage page after the first round and correct these numbers here.

A thread may also run it from the cloud as a fresh (not forked) agent pointed at the packet path with the same prompt; record which way it ran.

Always pass the model flag. Codex without `-m` did not give Sol. Grok 4.8 Fast was not available through cursor-agent, so use `grok-4.7-high-fast`. Check with `--list-models` if that changes.

cursor-agent launched from Git Bash can be blocked by the repo's `.cursor` pre-tool hook (`impeccable-before-edit.js` dies with a bash syntax error, every file read rejected, exit still 0, 30 s). Launch it from PowerShell with the same flags (torso frame gate, 2026-11-06). Check that the review actually read files before trusting an exit 0.

Antigravity (Argon) is on the MacBook, not Joey-PC: `agy` 1.3.4 at `~/.local/bin/agy` (checked 2026-11-06 05:40Z, signed in, `agy models` lists `gemini-4-argon-high`, a one-word smoke test answered in 7 s). Run it from the Mac against the packet. Prompt must be attached to the flag (`-p='...'`), or `-p` swallows the next flag as the prompt:

```bash
(agy --model gemini-4-argon-high --disable-slash-commands --print-timeout 0 \
  --add-dir "$PWD" -p="$P" > reviews/antigravity-argon.md 2> reviews/antigravity-argon.log </dev/null
 echo "argon exit $?" >> reviews/STATUS.txt) &
```

Read-only is by prompt only (like Muse); `--mode plan` is ignored with slash commands disabled, and there is no sandbox flag that forbids writes, so check the hashes after.

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
| Claude Code (Fable, Opus) | Argument | None passed. It reads `views/` itself | stdout |
| cursor-agent | Argument | None passed. It reads `views/` itself | stdout |
| Muse | `--prompt-file` | `--image`, repeated | stdout. The log holds startup notes |

Harmless Muse notes: a malformed `poteto-mode` skill warning, and "Agent delegation: auto unavailable: workspace is untrusted".

## 4. After the run

1. `cat reviews/STATUS.txt`: every line should say exit 0. An empty `cursor-grok.log` is normal.
2. `sha256sum -c SHA256SUMS.txt --quiet` again, to prove nothing changed. This is the only check on Muse.
3. Read all four yourself. Check each finding against the files before accepting it. At gate 2, three were rejected, including one claimed blocker, and one suggested fix was impossible.
4. Write `reviews/SUMMARY.md`: accepted, with who raised it and how you checked it; rejected, with the reason; superseded; can wait.
5. Report your own errors plainly. Publish the summary, not the raw reviews. Example: `hardware/designs/left-arm/upper-arm/GATE2_REVIEW.md`.
