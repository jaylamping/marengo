# Compute plan: what marengo needs, what we have, what's worth buying

**Status:** planning discussion, not a decision. Started 2026-11-07 from Joey's ask to go over compute goals, what's feasible, current hardware options, the model landscape, and how the Pi 5 and Jetson fit.
**Owner:** hardware project, thread "Plan the robot's compute stack". Anything that changes software (process roles, buses, ADR 0014) goes to the software project through Joey.

## Short version

1. **Keep the Pi 5 + Jetson Orin Nano 8 GB split. Don't buy compute now.** The split in [ADR 0014](../../docs/decisions/0014-jetson-perception-semantic-motion.md) is still right: the Pi does real-time control, safety and CAN; the Jetson does cameras, speech and small models; anything big runs on the desktop PC or in the cloud.
2. **Prices are terrible right now.** The DRAM shortage pushed NVIDIA's Jetson prices up by as much as 101% in July 2026 (Orin Nano Super kit $249 to $399, AGX Thor kit $3,499 to $5,499), and the Pi 5 is now $175 for 8 GB and $385 for 16 GB. What you already own got more valuable. Buy only when a measured bottleneck shows up.
3. **The real compute bottleneck for the full body is CAN, not CPU.** Five limb buses need five channels; the 2-channel MCP2515 HAT can't do it. Power board Rev B already moved this to isolated USB-CAN adapters on the Pi, one per bus. That's the next compute-side purchase, and it's cheap.
4. **If a good deal turns up, the one upgrade worth having is a Jetson Orin NX 16 GB *module*.** It drops into your existing Orin Nano dev kit carrier, doubles memory (the Jetson's real limit), and roughly doubles AI throughput. Nothing else on the market changes what the robot can do for a sensible price.
5. **Skip:** Jetson Thor (only needed for on-robot foundation models, $5.5k), the Hailo AI HAT+ 2 (puts AI load on the control computer, weak LLM quality), a second Pi 5 for the robot, and Strix Halo mini PCs (great LLM boxes, but $2–3k, 50–120 W and not robot-shaped).

## 1. What compute has to do, by phase

| Phase | Compute job | Where it runs | Load |
|---|---|---|---|
| Now: arm on the bench | 200 Hz control loop, Davout safety, CAN, IMU, gateway to Consul | Pi 5 | light |
| Calibration loop ([CALIBRATION_LOOP.md](../designs/left-arm/testing/CALIBRATION_LOOP.md)) | BRIO camera, AprilTag detection, fitting | Off-robot host is fine; Jetson later | light, not real-time |
| Full body, stationary | Same loop over 23 joints on 5 CAN buses | Pi 5 + 5 USB-CAN adapters | CPU light; CAN is the limit (section 3) |
| Standing and walking | State estimation + a learned or model-based balance/gait policy at 50–200 Hz | Pi 5 (a locomotion policy is a small MLP, well under 1 ms per step on a Pi CPU) | moderate |
| Training locomotion policies | Thousands of parallel sim envs (MuJoCo Playground / Isaac Lab) | Desktop GPU (the 5080 PC), never the robot | heavy, offline |
| Seeing and talking | Cameras, person/object detection, speech-to-text, text-to-speech, wake word | Jetson | moderate, continuous |
| Language commands ("wave your right hand") | LLM turns words into a motion-primitive plan (ADR 0014) | Cloud or PC by default; small local model on the Jetson as an offline fallback | bursty |
| End-to-end robot foundation models (VLA) | Camera + words in, joint targets out | Not on this robot for now (section 5) | very heavy |

The key point: nothing in the roadmap up to walking needs more on-robot compute than we have. Walking needs a decent *training* GPU off the robot, and the policy itself is tiny.

## 2. How the current hardware fits

### Raspberry Pi 5 (control computer)

- **Right job, keep it.** 4× Cortex-A76 at 2.4 GHz handles a 200 Hz loop for 23 joints, Davout, the IMU and the gateway with room left. Running a locomotion policy here is fine too.
- **Watch:** keep it free of camera and AI work so the control loop stays boring. That's why the Hailo HAT is a no (section 4).
- **I/O it needs for the full body:** USB for the CAN adapters (it has 2× USB 3 + 2× USB 2; a small powered hub if we pass four), I²C for the BNO085, GPIO for the E-stop sense and the TPS3431 heartbeat on the power board, one Gigabit Ethernet port (goes to the Jetson, section 6).
- **Power:** 5 V at up to 5 A, ~5–12 W in use.

### Jetson Orin Nano 8 GB (perception and interaction computer)

- **Right job, keep it.** On JetPack 6.2 it runs in "Super" mode: up to 67 INT8 TOPS at 25 W, 102 GB/s memory bandwidth (was 40 TOPS / 68 GB/s). If it hasn't been reflashed since early 2025, do that first; it's a free ~1.5–1.7× speedup.
- **Its real limit is 8 GB of shared memory**, not raw speed. Vision plus a 7–8 B LLM at the same time doesn't fit well (ADR 0014's own math).
- **What it can actually run** (measured numbers from a 2026 Orin Nano Super benchmark, llama.cpp, 25 W mode):

  | Model | Quant | Tokens/s |
  |---|---|---|
  | Gemma 3 4B | Q4_K_M | ~23 |
  | Qwen 3.6 7B / Mistral 7B / Llama 3.1 8B | Q4_K_M | ~13–14 |
  | Llama 3.1 8B with TensorRT-LLM | 4-bit | ~18 (claimed) |
  | 13B and up | | out of memory or too slow |

  A 3–4 B model at 20+ tokens/s is plenty to turn "wave your right hand" into a short primitive plan in a second or two, with vision paused. That makes an **offline fallback on the Jetson realistic**, which ADR 0014 rejected; worth updating there.
- **Vision:** YOLO-class detection at 30–60 fps INT8, human pose, AprilTags, depth from a stereo camera: all comfortable.
- **Power:** 7–25 W from the power board's spare 12 V output (barrel jack). Verify the dev kit carrier's DC input range accepts our 12 V rail before first power-up; the kit ships with a 19 V brick.

### The desktop PC (RTX 5080, 16 GB VRAM)

- Part of the compute stack even though it's not on the robot: policy training in sim, the bigger models over the house network, and offline calibration fitting. (ADR 0014 still says 3070 Ti; the PC now has a 5080.)
- 16 GB of VRAM fits: Whisper large-v3-turbo, 7–14 B LLMs at Q4 with room for context, small VLMs, and a 3 B VLA like GR00T in bf16 (about 6.5 GB of weights). 27–32 B dense models at Q4 are a squeeze or don't fit; those go to a cloud provider.
- It's also Joey's gaming PC, so the robot has to cope with the GPU being busy or the PC being off (section 8).

## 3. The CAN bottleneck (the one that bites first)

RobStride drives use 1 Mbit/s classic CAN with 29-bit IDs. An 8-byte extended frame is about 130 bits on the wire with stuffing, so one bus carries at most ~7,600 frames/s. Every joint needs a command and a reply each control tick, so 2 frames per joint per tick. Keeping each bus under ~50% load:

| Bus | Joints | Max loop rate at 50% load |
|---|---|---|
| One leg | 6 | ~315 Hz |
| One arm | 5 | ~380 Hz |
| Waist | 1 | plenty |

So 200 Hz across the whole body is comfortable on five buses, and legs can go to ~300 Hz if a gait controller wants it. 500 Hz+ on a leg would need the leg split over two buses.

**Interface hardware:** the MCP2515 HAT has 2 channels and a 2-frame receive buffer, which is already the bench's tightest limit ([docs/status.md](../../docs/status.md), open decision 1). Power board Rev B dropped the on-board CAN bridge chips and picked **isolated USB-CAN adapters with gs_usb firmware** (CANable 2.0 Pro isolated class, about $50–60 each), one per bus ([REV_B_CLEAN_SHEET.md](../electrical/pdb/REV_B_CLEAN_SHEET.md), item 11). Linux sees them as native SocketCAN devices with deep buffers. Five adapters is about $250–300; buy them as the limbs come online, not all at once. The bus plan ADR still owed by the software project should name them.

## 4. Hardware options on the market (November 2026)

All prices are list prices after the 2026 DRAM increases; street prices vary.

| Option | What it is | Price | Fits marengo? |
|---|---|---|---|
| **Jetson Orin NX 16 GB module** | Same carrier as your Orin Nano kit; 16 GB, up to 157 TOPS in Super mode | Up ~80% in July; check street price | **Best upgrade if a deal appears.** Doubles memory: vision + a 7–8 B model at once becomes realistic. Module only, reuse the carrier. |
| Second Jetson Orin Nano 8 GB | Another of what you have | $399 kit | Not useful. Two 8 GB boxes don't add up to one 16 GB box for models. |
| Jetson AGX Orin 32/64 GB | Big Orin | $3,499 kit, $1,799 32 GB module | Too expensive for what it adds over an Orin NX 16 GB. |
| Jetson Thor (T5000 / T4000) | Blackwell, 128 GB (T5000), built for humanoid foundation models | $5,499 kit; T4000 module pricing unclear | Only needed to run GR00T-class models on the robot. Not now. |
| Raspberry Pi AI HAT+ 2 | Hailo-10H, 40 TOPS, 8 GB on the HAT | $130 | **No.** Puts AI work on the control computer, the LLM support is 1.5–3 B models that Tom's Hardware found unreliable, and the Jetson already does this job better. |
| Raspberry Pi AI HAT+ (Hailo-8/8L) | Vision-only NPU | $70–110 | No, same reason; vision lives on the Jetson. |
| Second Raspberry Pi 5 | | $110 (4 GB) to $385 (16 GB) | Not for the robot. A spare 4 GB Pi for a bench or sim rig can be handy, nothing more. |
| RK3588 boards (Orange Pi 5, Rock 5) | 6 TOPS NPU, 8–16 GB | $130–310 now, often sold out | No. Weaker AI software than Jetson, and prices doubled too. |
| Strix Halo mini PC (Ryzen AI Max+ 395) | x86 with 64–128 GB unified memory | ~$2,000–3,000 | Great off-robot LLM server, wrong for the robot (size, 50–120 W). Only if you ever want 30–70 B models at home. |

**What counts as a "good deal":** an Orin NX 16 GB module (not a whole kit, you have the carrier) at or below roughly its pre-July price, or a used one from a trusted seller. My rough line is under ~$450 for the module; that's a judgment, not a researched number. Anything Orin with 8 GB or less isn't worth buying for this robot.

## 5. Model landscape (what's worth running)

| Job | Good current choices | Runs on |
|---|---|---|
| Object and person detection | YOLO family (v8n to v11-class) INT8 in TensorRT | Jetson, 30–60 fps |
| Human pose / hand tracking | YOLO-pose, MediaPipe-class | Jetson |
| Fiducials (calibration) | AprilTag | Any CPU |
| Speech to text | Whisper small / distil-Whisper, NVIDIA Parakeet | Jetson (TensorRT); cloud as fallback |
| Text to speech | Piper (light), Kokoro-class | Jetson or Pi CPU |
| Language to motion plan (ADR 0014) | Cloud frontier model for quality; Gemma 3 4B / Qwen 3.x 4–8 B locally | Cloud/PC first, Jetson offline fallback |
| Small vision-language model ("what am I holding?") | Qwen3-VL 2B and similar | Jetson, slow (seconds per answer), vision paused |
| Locomotion | Small MLP policy trained with RL in sim (the approach behind most current humanoid walking, including Unitree's G1 demos) | Train on the PC GPU, run on the Pi |
| End-to-end VLA (GR00T N1.6/N1.7, π0/π0.5, SmolVLA, OpenVLA) | GR00T N1.7 is a 3 B model; NVIDIA's own numbers are 40–126 ms per step **on Thor**, with no Orin support claimed | Not on this robot. Marengo v1 has no hands, which is where VLAs earn their keep. Revisit with hands, and run one on the PC over Wi-Fi first if curious. |

Two takeaways:

- **Locomotion is cheap to run and expensive to train.** The robot-side compute is fine; the work is sim, a good URDF/MJCF with weighed masses, and GPU time on the PC.
- **Language is about latency and quality, not on-robot TOPS.** Cloud gives the best plans; a local 4 B model gives an offline fallback. Neither needs new hardware.

## 6. Wiring, network, power and heat

- **Pi ↔ Jetson:** one direct Ethernet cable on an isolated subnet (ADR 0014). That uses the Pi's only Ethernet port, so the operator link is Wi-Fi on both boards, or a USB Gigabit dongle on the Pi if wired bench access matters. No switch inside the robot.
- **Cameras:** MIPI CSI or USB to the Jetson, never the Pi.
- **Power:** Pi 5 V/5 A and Jetson 12 V barrel both come off the power board; the Jetson is on one of the two spare 12 V outputs. Together ~15–35 W.
- **Heat:** already in the chest budget (60–75 W total including both pitch RS03s) with two 40 mm exhaust fans behind the D1 door ([body-shell manufacturing README](../designs/body-shell/manufacturing/README.md)). An Orin NX would sit in the same envelope (10–25 W modes; the dev kit's carrier and cooler are the same size).

## 8. Joey's direction (2026-11-07)

The robot stays in the house for now. Pi = everything that isn't inference (CAN, connectivity, telemetry, logging). Jetson = small, latency-critical inference. Everything heavier goes to the 5080 when it fits, otherwise to the fastest cloud provider.

How to make that work:

- **One routing point.** A small service (on the Jetson) takes each inference request and picks Jetson, then PC, then cloud, by what the task needs and what's reachable. Callers don't care where it ran.
- **Graceful loss.** If the PC is off or busy gaming and the internet is down, the robot keeps running and only loses the features that needed them. Nothing that keeps it standing, safe or stopped ever crosses the network; that stays on the Pi.
- **Latency budget.** House Wi-Fi adds a few ms to tens of ms with spikes; wired Ethernet to the PC is steadier. Cloud adds tens to hundreds of ms plus the provider's time to first token. Fine for language, speech and "what am I looking at"; not for anything inside a control loop.
- **The MacBook (M5 Max, 128 GB unified memory) is a fourth tier during work sessions.** Joey has it open beside the robot whenever he's working on it. 128 GB holds models the 5080 can't: 70 B dense at Q4 (~40 GB), gpt-oss-120b (~63 GB), big MoE models, large VLMs. It's slower per token than the 5080 on small models and has no CUDA, so it can't run TensorRT, Isaac Lab, or CUDA-only robot models (GR00T's tooling). Use it for big LLMs/VLMs via MLX or llama.cpp; keep CUDA work on the 5080. It sleeps and runs on battery, so the router treats it as "there when it's there".
- **Routing order:** Jetson (small, latency-critical) → 5080 (CUDA, fits in 16 GB) → MacBook (big models, during sessions) → cloud (everything else, or when nothing local is up).
- **Fastest providers.** For open-weight models, Groq and Cerebras run hundreds to over a thousand tokens/s with low time to first token. For the best plans from words, a frontier model is slower but smarter. Measure from the house before picking.

## 9. Learning direction (Joey 2026-11-07)

Joey does not want a robot that only replays a fixed list of actions. His picture: train every core movement and interaction as a primitive skill first (move hand to a Cartesian point and hold, turn head toward a sound, stand in place). A reasoning model then works out what a command means ("what is a wave, which hand, I'm right dominant"), composes the skills into a sequence, and the results feed back so the robot keeps improving, including updating its own weights from each day's events.

This supersedes ADR 0014's "primitive selection from a fixed catalog" as the long-term goal (software project to update). Two learning speeds:

- **Fast, no weight change:** keep a memory of plans that worked and what Joey corrected, and feed it back as context next time.
- **Slow, nightly:** fine-tune (LoRA) the planner on the day's logged episodes, on the 5080 or the Mac overnight. Feasible today for 4–8 B models. Needs a score for each episode (camera check, Joey's feedback) and an eval suite: new weights only go live if they beat the old ones, and the old ones are kept.

Whatever the model learns, motion still goes through the Pi's limit checks.

Weight-space programming ([MIT CSAIL post, 2026-11-02](https://rikiyat.github.io/blog/weight-space-programming/)): an LLM reads and edits network weights directly from a goal, no gradient descent. Early and small (17 k to 1.5 M parameter policies, edits on a 0.8 B LLM); the authors say edits unlock existing abilities rather than add missing ones. Most relevant here at small scale: our motor policies are that size. Watch item, not the plan of record.

Hardware consequences:

- **Log everything, keep it.** Joint commands and feedback, IMU, audio and camera from the Jetson, operator commands and outcomes. Needs an NVMe SSD on the Jetson (the dev kit has M.2 slots) and on the Pi (M.2 HAT), sized for days of data, with a nightly offload to the PC.
- **Leave room for a bigger on-robot module.** If the model moves onto the robot later, the compute bay, the 12 V output and the chest fans should take an Orin NX 16 GB without rework, and ideally not rule out a Thor-class module (40–130 W) without a redesign.

## 10. Open questions for Joey

1. Which Pi 5 do you have, 4, 8 or 16 GB? (8 GB or more is plenty; 4 GB is still fine for control.)
2. Has the Jetson been flashed to JetPack 6.2 or later (Super mode)?
3. Do you want the robot to work fully offline (no cloud, no PC), or is "cloud/PC for language, Jetson for vision" fine? Offline pushes toward the Orin NX 16 GB.
4. Do you want me to watch for Orin NX 16 GB module deals, and what's your ceiling?

## Sources

- Jetson price increases: [Hardware Busters, July 2026](https://hwbusters.com/news/nvidia-jetson-prices-jump-up-to-101-the-249-orin-nano-super-is-now-399/), [CNX Software, July 2026](https://www.cnx-software.com/2026/07/22/nvidia-increases-the-price-of-jetson-modules-and-devkits-by-up-to-101/)
- Pi 5 prices: [Hardware Busters, Nov 2026](https://hwbusters.com/news/raspberry-pi-5-16gb-price-jumps-to-385-and-the-extra-8gb-now-costs-more-than-a-whole-8gb-board/); SBC price comparison: [CNX Software, Apr 2026](https://www.cnx-software.com/2026/04/28/what-a-difference-two-years-make-comparing-sbc-prices-in-2024-and-2026/)
- Orin Nano Super LLM throughput: [SpecPicks benchmark, 2026](https://specpicks.com/reviews/jetson-orin-nano-super-local-llm-tokens-per-second-2026)
- AI HAT+ 2: [Tom's Hardware review](https://tomshardware.com/raspberry-pi/raspberry-pi-ai-hat-plus-2-review)
- GR00T N1.7 on Thor: [Jetson AI Lab tutorial](https://www.jetson-ai-lab.com/tutorials/groot_n17_on_thor/); [GR00T N1.6 model card](https://huggingface.co/nvidia/GR00T-N1.6-3B)
- Strix Halo mini PCs: [SpecPicks Ryzen AI Max+ 395 review](https://specpicks.com/reviews/amd-ryzen-ai-max-395-strix-halo-128gb-mini-pc-2026)
- In repo: [ADR 0014](../../docs/decisions/0014-jetson-perception-semantic-motion.md), [hardware ADR 0001](decisions/0001-can-and-motors.md), [power board PLAN §3.6](../electrical/pdb/PLAN.md), [REV_B_CLEAN_SHEET.md](../electrical/pdb/REV_B_CLEAN_SHEET.md)
