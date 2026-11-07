# Argon reader

Joey, 2026-11-07: Argon (Gemini 4 Argon, through Antigravity `agy` on the Google AI Ultra Gemini quota) is the project's **reader**. It isn't a reviewer. Use it for heavy reading: long manuals, datasheet tables, whole-folder consistency checks, video, and first-pass graph reading. **Every value it returns is checked before anyone uses it.**

## Command (Joey-PC)

```bash
AGY=/c/Users/joeyl/AppData/Local/agy/bin/agy.exe
cd <task folder>
"$AGY" --model gemini-4-argon-high \
  --add-dir "C:/Temp/<task folder>" --add-dir "<every folder it must read>" \
  -p='Read C:/Temp/<task folder>/TASK.md and do exactly what it says. Run no shell commands; use only file read and search tools. Do not modify any file.' \
  --output-format json --log-file C:/Temp/<task folder>/agy.log > out.json
```

- **Read-only is enforced by the prompt only.** Never pass `--dangerously-skip-permissions`. Headless mode denies any tool it can't match and ends the run, so no shell. Check `denied_actions` in `out.json`.
- Pass every folder it needs with `--add-dir`. Reading outside them ends the run.
- Attach the prompt to the flag (`-p='...'`). In Git Bash, prefix slash commands with `MSYS_NO_PATHCONV=1`.
- The answer is in `out.json` → `response`, with token usage beside it.

## Inputs

- **Text and netlists:** pass the files.
- **PDF figures:** render the pages to PNG first (PyMuPDF, `page.get_pixmap(dpi=200)`) and pass the folder. It reads PNGs with its image tool. Keep the PNGs, because the verification step needs them.
- **Video:** put the file in the task folder and name it in TASK.md. This isn't tried yet; trial it on a short clip first.

## TASK.md shape

1. Name the exact files and the question for each one.
2. Say: "Read only what the files show. If it isn't there, answer NOT IN SOURCE. Never estimate from memory."
3. Fix the output table:

| Item | File, figure or line | Reading (value + units) | Confidence (high/med/low) | How read |
| --- | --- | --- | --- | --- |

## Mandatory verification

- **Text and table values, and netlist claims:** spot-check at least a third of the rows against the file, plus every row a decision rests on.
- **Graph readings: check every value used in a decision**, by pixel measurement on the PNG (axis calibration, then a column scan for the curve) or by eye against the gridlines.
- **Log-log plots:** never use Argon's number unchecked. In trial 2 it read the IPB017 SOA 1.5–2× high, in the unsafe direction, and still marked the readings "high" confidence.
- Score each task in `reviewer-scorecard.md` as "Argon (task: ...)".
- Argon is out for good if it invents content that isn't in the source.

## Trials, 2026-11-07

| Task | Result |
| --- | --- |
| Claim checker: 20 round-4 claims against the netlist and datasheet text, with a hidden key | **20 of 20 correct**, each cited by net, pin or datasheet line. 4.3 min. One wrong side note: it quoted 544 W, which Sol and Astra derive as 652 W |
| Figure reading, Arcol HS temperature-rise graph | Correct, within about 5 %. It correctly reported that Arcol publishes **no pulse or overload curve** (NOT IN SOURCE) |
| Figure reading, Murata DBE0125 derating curves (12 sets) | Usable but read low: up to 2.5 A (about 13 %) under on the high-airflow curves at 60–70 °C. The error is in the conservative direction |
| Figure reading, IPB017N10N5LF SOA (log-log) | **Failed**: 1 of 10 readings within 25 %, all of them high. Pixel-measured: 10 µs 800/320 A, 100 µs 200/81 A, 1 ms 63/24 A, 10 ms 30/12 A, DC 15/4.7 A at 20/50 V. ZthJC readings (0.08, 0.2, 0.4 K/W at 1, 10, 100 ms) were fine |
