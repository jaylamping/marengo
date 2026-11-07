# Reviewer scorecard

One row per run, keyed on model and effort. Rules are in [review-recipe.md](review-recipe.md), "Reviewer audit". Add rows when a round's summary is written.

**Standing:** Argon REMOVED 2026-11-07 by Joey, after two off-target reviews. GLM 5.3 and Kimi K3.1 Pro REMOVED 2026-11-07 by Joey, for now, after delivering no review in two rounds. DeepSeek V4.1 Flash stays as a trial reviewer on CheaperInference. Trial reviewers don't gate until they pass the seeded test packet.

How to read the columns:

- **Real:** accepted findings the summary credits to this reviewer. The backfill counts blockers and majors, or the summary's main accepted list where it doesn't rank them. Minors are left out.
- **False:** claims rejected in the summary and credited to this reviewer. A claim shared by two reviewers counts for both.
- **Unique:** accepted findings credited to this reviewer alone.
- **False conf:** a real problem it checked and passed.
- **Min / unique:** minutes divided by unique findings, blank at zero.

Backfill limits: older summaries weren't written for this, so the counts are as good as their attribution. False confirmations weren't recorded before rev B, so 0 there means not recorded. Most older rows have no cost figure. Packet fidelity was checked with `hardware/tools/review/check_on_target.py` where the packet is in the repo (torso gate 2), and otherwise by the fact that findings were accepted against the files.

## Runs

| Round | Model | Effort | Real | False | Unique | False conf | Fidelity | Retries | Min | Cost | Min / unique | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Torso gate 1 | gpt-6.1-sol | config high | 17 | 3 | 0 | 0 | on | 0 | 9.2 | sub | | |
| Torso gate 1 | gpt-6-astra | config high | 19 | 3 | 2 | 0 | on | 0 | 6.7 | sub | 3.4 | |
| Torso gate 1 | grok-4.7-high-fast | in model name | 14 | 2 | 0 | 0 | on | 1 | 12.5 | sub | | First try blocked by a repo hook |
| Torso gate 1 | muse-wonder-1.0 | default (high) | 14 | 2 | 0 | 0 | on | 0 | 5.1 | sub | | |
| Upper arm gate 2 | gpt-6.1-sol | config high | 13 | 0 | 0 | 0 | on | 0 | 8.5 | sub | | |
| Upper arm gate 2 | gpt-6-astra | config high | 14 | 0 | 0 | 0 | on | 0 | 9 | sub | | |
| Upper arm gate 2 | grok-4.7-high-fast | in model name | 9 | 1 | 0 | 0 | on | 0 | 12.5 | sub | | Dropped from the panel 2026-11-06 |
| Upper arm gate 2 | muse-wonder-1.0 | default (high) | 12 | 2 | 1 | 0 | on | 0 | 9 | sub | 9 | |
| Torso gate 2 | gpt-6.1-sol | config high | 12 | 0 | 0 | 0 | on (checked) | 1 | 11.1 | sub | | `-i` swallowed the prompt on try 1 |
| Torso gate 2 | gpt-6-astra | config high | 11 | 0 | 1 | 0 | on (checked) | 1 | 11.6 | sub | 11.6 | Same |
| Torso gate 2 | claude-opus-5-5 | high | 15 | 0 | 3 | 0 | on (checked) | 0 | 16 | API, not recorded | 5.3 | |
| Torso gate 2 | gemini-4-argon-high | high | 8 | 1 | 0 | 0 | on (checked) | 0 | 16.1 | sub | | |
| Torso gate 2 | claude-fable-5-1 | high | 11 | 0 | 1 | 0 | on (checked) | 0 | 18.3 | API, not recorded | 18.3 | |
| Torso gate 2 | muse-wonder-1.0 | default (high) | 5 | 0 | 0 | 0 | on (checked) | 0 | 4.2 | sub | | |
| PDB rev A gate 3, pass 1 | gpt-6.1-sol | unknown (Mac) | 16 | 1 | 1 | 0 | on | 0 | 10.7 | sub | 10.7 | |
| PDB rev A gate 3, pass 1 | gpt-6-astra | unknown (Mac) | 12 | 0 | 0 | 0 | on | 0 | 11.8 | sub | | |
| PDB rev A gate 3, pass 1 | claude-opus-5-5 | high | 15 | 3 | 0 | 0 | on | 0 | 17 | API, not recorded | | 2 false blockers |
| PDB rev A gate 3, pass 1 | claude-fable-5-1 | high | 12 | 4 | 0 | 0 | on | 0 | 18.5 | API, not recorded | | |
| PDB rev A gate 3, pass 1 | gemini-4-argon-high | high | 11 | 8 | 2 | 0 | on | 1 | 36.4 | sub | 18.2 | Unique catches include M8. 5 false blockers. Cited footprint files that don't exist: an early warning, not a strike |
| PDB rev A gate 3, pass 1 | muse-wonder-1.0 | default (high) | 13 | 4 | 0 | 0 | on | 0 | 4.2 | sub | | |
| PDB rev A gate 3, pass 2 | gpt-6.1-sol | unknown (Mac) | 9 | 1 | 2 | 0 | on | 0 | 11.7 | sub | 5.8 | |
| PDB rev A gate 3, pass 2 | claude-opus-5-5 | high | 12 | 2 | 5 | 0 | on | 0 | 17.6 | API, not recorded | 3.5 | |
| PDB rev A gate 3, pass 2 | claude-fable-5-1 | high | 7 | 4 | 1 | 0 | on | 0 | 22.9 | API, not recorded | 22.9 | |
| PDB rev A gate 3, pass 3 | gpt-6.1-sol | unknown (Mac) | 3 | 0 | 0 | 0 | on | 0 | 8.5 | sub | | |
| PDB rev A gate 3, pass 3 | claude-opus-5-5 | high | 7 | 0 | 2 | 0 | on | 0 | 13.6 | API, not recorded | 6.8 | |
| PDB rev A gate 3, pass 3 | claude-fable-5-1 | high | 4 | 0 | 0 | 0 | on | 0 | 15.8 | API, not recorded | | |
| PDB rev A gate 3, pass 4 | gpt-6.1-sol | unknown (Mac) | 5 | 0 | 1 | 0 | on | 0 | 7.5 | sub | 7.5 | |
| PDB rev A gate 3, pass 4 | claude-opus-5-5 | high | 7 | 0 | 3 | 0 | on | 0 | 15.9 | API, not recorded | 5.3 | |
| Upper arm rev C | gpt-6.1-sol | high | 10 | 0 | 0 | 0 | on | 0 | 18.7 | sub | | |
| Upper arm rev C | gpt-6-astra | high | 10 | 0 | 0 | 0 | on | 0 | 18.1 | sub | | |
| Upper arm rev C | claude-fable-5-1 | high | 10 | 1 | 0 | 0 | on | 0 | 25.2 | sub | | |
| Upper arm rev C | gemini-4-argon-high | high | 6 | 2 | 4 | 0 | on | 1 | 26.5 | sub | 6.6 | Found the blocked cable exit and the rear hub M4 alone |
| Upper arm rev C | muse-wonder-1.0 | default (high) | 8 | 5 | 0 | 0 | on | 0 | 7.9 | sub | | |
| Upper arm print package | gpt-6.1-sol | config high | 3 | 0 | 0 | 0 | on | 0 | 14.2 | sub | | Counts blockers only |
| Upper arm print package | gpt-6-astra | config high | 3 | 0 | 0 | 0 | on | 0 | 9.4 | sub | | Counts blockers only |
| Upper arm print package | claude-opus-5-5 | high | 6 | 0 | 1 | 0 | on | 0 | 16 | not recorded | 16 | Cloud agent. Counts blockers only |
| Upper arm print package | claude-fable-5-1 | high | 5 | 0 | 1 | 0 | on | 0 | 25 | not recorded | 25 | Cloud agent. Counts blockers only |
| Test pass 1 | claude-fable-5-1 | high | 9 | 0 | 2 | 0 | on | 0 | 28 | not recorded | 14 | Cloud agent |
| Test pass 1 | claude-opus-5-5 | high | 11 | 0 | 4 | 0 | on | 0 | 25 | not recorded | 6.3 | Cloud agent |
| PDB rev B | claude-fable-5-1 | high | 11 | 1 | 1 | 0 | on (checked) | 0 | 19.1 | about $12, estimated | 19.1 | Unique catch: U1 power limit |
| PDB rev B | muse-wonder-1.0 | default (high) | 12 | 5 | 2 | 1 | on (checked) | 0 | 8.6 | sub | 4.3 | Confirmed the wrong 544 W power limit |
| PDB rev B | gemini-4-argon-high | high | 0 | 0 | 0 | 0 | **off** | 5 | 31.4 | sub | | Scores zero. Invented a different board. 24 of 80 cited values match the netlist, against 4 of 4 and 29 of 30 for the others. **Strike 1** |
| PDB rev B2 | claude-fable-5-1 | high | 6 | 0 | 1 | 0 | on (checked) | 0 | 23.9 | $10.44 (JSON) | 23.9 | Unique: latch can arm with reset held at power-up |
| PDB rev B2 | claude-opus-5-5 (agy) | high | 12 | 2 | 4 | 0 | on (checked) | 0 | 24.6 | Antigravity quota | 6.2 | First agy run. Unique: a UVLO solder bridge back-drives RUN and keeps bank A on; J20 pins 3-4 restart without reset |
| PDB rev B2 | gpt-6.1-sol | high | 13 | 0 | 1 | 0 | on (checked) | 0 | 22 | sub | 22 | Unique: U2 reverse output-to-input |
| PDB rev B2 | gpt-6-astra | high | 12 | 0 | 0 | 0 | on (checked) | 0 | 19.7 | sub | | |
| PDB rev B2 | muse-wonder-1.0 | high | 9 | 2 | 0 | 1 | on (checked) | 0 | 15.8 | sub | | Passed the gate arithmetic that B1 overturned |
| PDB rev B2 | gemini-4-argon-high | high | 0 | 0 | 0 | 0 | **off** | 0 | 17.8 | sub | | Scores zero. 15 of 49 cited values match. **Strike 2: removed** |
| PDB rev B2 | deepseek-v4.1-flash (trial) | high | 0 | 0 | 0 | 0 | no review | 1 | 68 | Go, <$0.01 (console) | | Miss. Denied shell call; retry hit the Go quota and hung |
| PDB rev B2 | glm-5.3 (trial) | high | 0 | 0 | 0 | 0 | no review | 0 | 45 | Go, $1.03 (console) | | Miss. Go quota ran out, then hung |
| PDB rev B2 | kimi-k3.1-pro (trial) | high | 0 | 0 | 0 | 0 | no review | 0 | 45 | Go, $2.56 (console) | | Miss. Go quota ran out, then hung |
| PDB rev B2 late | deepseek-v4.1-flash (trial) | high | 14 | 3 | 4 | 0 | on (checked) | 0 | 12.6 | CheaperInference, ~$0.06 | 3.2 | First delivered open-model review. Unique: U2 short puts the pack on VCTL; SPARE_ON cap; INA228 strapping |
| PDB rev B2 late | glm-5.3 (trial) | high | 0 | 0 | 0 | 0 | no review | 0 | 24 | CheaperInference, $2.02 (cap) | | Miss. Stopped at the $2 cap before its final answer |
| PDB rev B2 late | kimi-k3.1-pro (trial) | high | 0 | 0 | 0 | 0 | no review | 1 | 21 | CheaperInference, $1.08 | | Miss. The provider cut the stream twice |
Not scored: `torso-frame/review/packet3` has reviews but no summary in the same form. Score it if its findings get a summary.

## By model

Totals of the rows above. Compare them with care: rounds differ, and not every model ran in every round.

| Model | Runs | Real | False | Unique | False conf | Off target | Retries | Mean min |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| claude-opus-5-5 | 7 | 73 | 5 | 18 | 0 | 0 | 0 | 17.3 |
| claude-opus-5-5 (agy) | 1 | 12 | 2 | 4 | 0 | 0 | 0 | 24.6 |
| claude-fable-5-1 | 9 | 75 | 10 | 7 | 0 | 0 | 0 | 21.9 |
| gpt-6.1-sol | 10 | 101 | 5 | 5 | 0 | 0 | 1 | 12.2 |
| gpt-6-astra | 7 | 81 | 3 | 3 | 0 | 0 | 1 | 12.3 |
| muse-wonder-1.0 | 7 | 73 | 20 | 3 | 2 | 0 | 0 | 7.8 |
| gemini-4-argon-high (removed) | 5 | 25 | 11 | 6 | 0 | 2 | 7 | 25.6 |
| grok-4.7-high-fast (dropped) | 2 | 23 | 3 | 0 | 0 | 0 | 1 | 12.5 |
| deepseek-v4.1-flash (trial) | 2 | 14 | 3 | 4 | 0 | 0 | 1 | 40.3 |
| glm-5.3 (trial) | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 34.5 |
| kimi-k3.1-pro (trial) | 2 | 0 | 0 | 0 | 0 | 0 | 1 | 33.0 |
