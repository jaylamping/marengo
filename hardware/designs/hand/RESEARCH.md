# Humanoid hands: market, technology, and how we build our own

2026-11-06. Research only, no CAD. Owner: thread "Research humanoid hands to build our own".
Every number below is vendor-stated or paper-stated unless marked **measured** or **our calculation**. Prices move monthly; treat them as order-of-magnitude.

## 1. Bottom line

1. **The market has split into three tiers.** Frontier integrated hands (Tesla Optimus V3, 1X NEO, Figure 03, Sanctuary, Shadow DEX-EE) are 20 to 25 DOF, tendon-driven from forearm motors, tactile-skinned, and not for sale (DEX-EE is, at about $60k). Mid-tier research hands sell for $5k to $20k (Inspire, Unitree, LinkerBot, Psyonic, BrainCo). Open-source, 3D-printed hands you can build in a day now cost $1.3k to $2k in parts and have been cycle-tested to 10,000+ grasps (ORCA, RUKA v2, LEAP v2, BiDexHand).
2. **The technology converged in 2025 to 2026 on two architectures.** Tendon-driven with motors behind the wrist (Optimus, NEO, Shadow, ORCA, RUKA) when you want 16+ DOF and human-like fingers, and linkage-driven with pen-sized lead-screw linear actuators in the palm (Inspire, Zhaowei, LinkerBot, Tesla Gen 2) when you want cheap, stiff, self-locking, 6 to 12 DOF. In-finger motors (Figure, Unitree Dex5) exist but need custom micro-gearmotors we cannot buy.
3. **Recommendation: build our own tendon-driven hand with the actuator pack in a printed wrist module on the RS00 output, using ORCA's joint and tendon design (MIT licence) as the starting point and a Feetech/Dynamixel serial bus for the finger servos.** Target: 16 active finger DOF + 1 wrist flexion, under 800 g from the RS00 output face, Bambu P2S printable, replaceable fingers, fingertip FSRs for the first version. Reasons in section 6. Fallback if tendons prove too fiddly: a 6-actuator Inspire-style linkage hand with Actuonix PQ12 micro linear actuators (section 6.3).
4. **Two decisions for Joey** before any CAD: (a) the weight and length budget of the hand, which the arm threads need (section 5.2), and (b) whether the hand gets its own wrist flexion joint, which the arm currently lacks (section 5.3).

## 2. What our arm gives the hand

From the left-arm layout, forearm draft, and the RS00 manual (`hardware/docs/robstride/RS00_User_Manual_260713.pdf`).

| Item | Value | Source |
|---|---|---|
| Last actuator | RobStride RS00, lower-arm yaw (pronation/supination). No wrist pitch or deviation. | layout/README.md |
| RS00 torque | 5 N·m rated at 100 rpm, 14 N·m peak, 315 rpm no load, 48 V nominal, 24 to 60 V | RS00 manual §1.2, 1.3 |
| RS00 output interface | 6 × M3 on a 27 mm circle, Ø34.6 output face (vendor STEP). The 6 × M3 on Ø50 is the RS00 housing, not the output. The hand bolts to the forearm's wrist flange: 6 × M3 heat-set inserts on Ø50 with a Ø35 pilot, on the turning tube that rides the RS00 output | `../actuator-test-stand/README.md`, forearm B |
| Forearm | Ø64 round tube, 3 mm wall, 195 mm elbow to wrist; 51.4 mm from RS00 output to forearm end | layout, forearm README |
| Hand placeholder | 90 mm stub, Ø60 proxy; "hand" excluded from every torque figure so far | layout, envelope README |
| Elbow actuator | RS02: 6 N·m rated, 17 N·m peak | RS02 manual |
| Shoulder actuators | RS03: 20 N·m rated, 60 N·m peak | RS03 manual |
| Power and comms on the arm | 48 V trunk, one fused XT30 port per motor, CAN 2.0 at 1 Mbit/s, small CAN plug at each joint | arm trunk plan |
| Design load case | ground-level tip-over; printed parts fail before motors or frame | project memory |
| Printer | Bambu P2S, 0.4 mm high-flow nozzle, 256 mm cube | project memory |

Consequences:

- **The hand is a bolt-on module on the Ø50 six-hole wrist flange** (on the turning tube; the tube bolts to the RS00 output, 6 × M3 on Ø27). Anything we build or buy needs an adapter plate to that pattern. Nothing can route through the RS00 (it is not hollow for our purposes), so forearm-mounted tendon motors in the Optimus sense are out. "Forearm" motors for us means a motor block between the RS00 output and the palm, which is exactly how ORCA, RUKA and LEAP package theirs.
- **The hand needs its own low-voltage rail.** Every commercial hand runs at 12 or 24 V (Inspire 12 to 48 V, LinkerBot 24 V, O6 12 V, Dex3 12 to 58 V). Hobby bus servos run at 5 to 12 V. The arm trunk is 48 V, so the hand module or the forearm needs a small DC-DC (for example 48 V to 12 V, 60 W class) with its own fuse. This is a power-board-thread item.
- **The hand needs its own control link.** Commercial hands speak RS-485, USB, Ethernet, or CAN. Hobby servo hands speak half-duplex TTL serial. The cleanest plan is a small MCU in the hand module that talks to the finger servos and presents one CAN node on the arm bus, so the hand looks like a sixth RobStride to the controller. That is a code-project item but it changes the harness (one extra CAN drop, one extra power drop).
- **Mass at the wrist is the biggest number nobody has budgeted.** See 5.2.

## 3. Market survey

### 3.1 Frontier, integrated, not for sale

| Hand | DOF | Actuation | Sensing | Notable | Source |
|---|---|---|---|---|---|
| Tesla Optimus V3 (patents, 2026) | ~22 incl. 2 wrist; 4 per finger | Tendon, 3 cables per finger, all actuators in the forearm, "wrist router" reorders the bundle from a lateral stack to a vertical stack to cut crosstalk | Not stated in patents | Gen 3 reporting: 17 motors, 22 Hall sensors, 94 tactile points; durability goal of millions of cycles | [Teslarati](https://www.teslarati.com/tesla-optimus-v3-hand-arm-details-revealed-new-patents/), [China tracker](https://robotics.techbuzzchina.com/reports/robot-hands-china.html) |
| 1X NEO (July 2026) | 25 (22 hand + 3 wrist) | Tendon from in-house forearm motors at 5:1 to 15:1, all joints backdrivable and force-controlled | Tactile skin: normal, shear, slip | 3.5 N·m thumb CMC, 2.6 N·m finger MCP, 45 N distal flexion, 17.75 N·m wrist, ±0.2 mm, IP68, washable, "10,000 hands this year" | [1x.tech](https://1x.tech/discover/neos-hands) |
| Figure 03 | ~16 (02 figure) | Motor and sensor unit in each finger | 3 g tactile threshold, palm cameras | 25 kg per-hand payload (02) | [Notebookcheck](https://www.notebookcheck.net/Figure-03-Humanoid-debuts-with-wireless-charging-and-ultra-sensitive-tactile-palms.1134940.0.html), [Wikipedia](https://en.wikipedia.org/wiki/Humanoid_hand) |
| Sanctuary Phoenix | 20 to 21 | Hydraulic | Micro-barometer arrays, ~5 mN | High force density, fluid maintenance | [Wikipedia](https://en.wikipedia.org/wiki/Humanoid_hand) |
| Shadow Dexterous Hand / DEX-EE | 24 joints, 20 actuated | 20 DC motors in the forearm, tendon | Optional | Shadow Hand Plus about €110k; DEX-EE about $60k; Shadow's rule of thumb is ~€5k per DOF | [Shadow](https://shadowrobot.com/how-much-does-a-robot-hand-cost/), [humanoid.guide](https://humanoid.guide/product/dex5-1/) |

Takeaway: every frontier hand moved the motors out of the hand and runs tendons, accepts that tendons are a wear item, and pairs the hand with tactile skin. None publishes mass.

### 3.2 Commercial hands you can order

| Hand | DOF (active/total) | Actuation | Mass | Fingertip or grip | Interface, power | Tactile | Price | Source |
|---|---|---|---|---|---|---|---|---|
| LinkerBot L6 | 6 / 11 | Linkage, own joint module | 624 g | 10 N thumb, 8 N finger, 28 kg load | CAN or RS-485, 24 V | no | €4,934 | [OpenELAB](https://openelab.io/products/linker-hand-linkerbot-l6-11) |
| LinkerBot L20 Lite | 10 / 20 | Linkage | 800 g | 12 N tip, 5 kg load, ±0.22 mm | CAN or RS-485, 24 V | pressure-resistive std. | €4,934 | [OpenELAB](https://openelab.io/products/linker-hand-linkerbot-l20-lite) |
| LinkerBot O6 | 12 | not stated (tendon implied by "tendon replacement") | 580 g | ~500 g pinch, ~2 kg power grasp | USB serial, 12 V 3 A | 16 capacitive regions | quote (reported ¥6,666 ≈ $920 in China) | [Robotics Center](https://www.roboticscenter.ai/hardware/linker-bot-o6) |
| Inspire RH56DFX | 6 / 12 | 6 micro linear screw servos in the palm, self-locking | 540 g (650 g with wrist) | 15 N thumb, 10 N finger, ±0.2 mm | RS-485 or CAN, 12 to 48 V, 2 A peak | optional | ~$8.4k to $10.5k | [Knoxlabs](https://www.knoxlabs.com/products/inspire-robots-rh56dfx-dexterous-hand), [Robotics Center](https://www.roboticscenter.ai/learn/dexterous-robot-hand-comparison-2026) |
| Unitree Dex3-1 | 7 (3 fingers) | 7 in-joint micro force-control joints, 45 g each, 0.49 to 3.1 N·m | 710 g | 500 g grasp load | USB 2.0, 12 to 58 V | 33 pads | from $7.8k | [Unitree](https://www.unitree.com/Dex3-1/) |
| Unitree Dex5-1 | 16 / 20 | Hollow-cup motors + micro composite transmission in the joints | 1,100 g | 10 N tip, 4.5 kg | USB 2.0 | 94 pads (1P) | ~$15k | [humanoid.guide](https://humanoid.guide/product/dex5-1/), [SourceBotics](https://sourcebotics.com/guides/chinese-dexterous-hands-2026-buyers-guide/) |
| BrainCo Revo 2 | 12 | prosthetic-derived | n/s | n/s | n/s | optional skin | from $5.9k | [Robotics Center](https://www.roboticscenter.ai/learn/dexterous-robot-hand-comparison-2026) |
| Psyonic Ability Hand | 6 | 6 BLDC, prosthetic | 470 g | 45 kg "strength", 200 ms close | BLE, I2C, UART, RS-485 | 30 touch sensors | ~$15k | [humanoid.guide](https://humanoid.guide/product/ability-hand/) |
| Wuji 20-DOF direct drive | 20 | micro-actuator per phalanx | 580 g | 15 N tip, 300k cycles | n/s | adapter-ready | from $13k | [Robotics Center](https://www.roboticscenter.ai/learn/dexterous-robot-hand-comparison-2026) |
| Allegro | 16 | in-finger servos | ~1 kg class | n/s | CAN, 24 V | none std. | $13k to $15k | [Robotics Center](https://www.roboticscenter.ai/learn/dexterous-robot-hand-comparison-2026) |
| Sharpa Wave | 22 active | n/s | 1,300 g | 20 N tip, ±1 mm, 500 Hz | Gigabit Ethernet, 18 to 28 V, 180 W peak | 1,000+ taxels per fingertip | ~$50k | [CNX](https://www.cnx-software.com/2026/06/02/sharpa-wave-high-end-dexterous-robotic-hand-with-22-dof-high-sensitivity-dynamic-tactile-array/) |
| Robotera XHAND1 | 12 active | independent drives | 1,100 g | n/s | n/s | optional | quote | [SourceBotics](https://sourcebotics.com/guides/chinese-dexterous-hands-2026-buyers-guide/) |

Market facts worth knowing ([China Humanoid Robotics Tracker](https://robotics.techbuzzchina.com/reports/robot-hands-china.html)): hands are 15 to 20 % of a humanoid's cost; average hand price in 2024 was about $8k; Chinese domestic hands are ¥5k to ¥100k against ¥1M+ for Shadow-class imports; LinkerBot claims 80 % share of high-DOF hands and is the only maker above 1,000 units per month; domestic tactile sensors fell to ¥199; the bottlenecks everyone names are tactile sensing, sub-20 mm micro-reducer supply, manual assembly, and no common interface.

### 3.3 Open-source hands we could build as-is

| Hand | DOF | Actuators | Tendon / drive | Parts cost | Build time | Mass | Proven | Licence | Source |
|---|---|---|---|---|---|---|---|---|---|
| ORCA v1 (ETH Zürich, 2025) | 17: 4 thumb, 3 × 4 fingers, 1 wrist flexion | 17 Dynamixel XC330 (ROBOTIS edition also XC430 for wrist); a Feetech STS3215 variant is documented by Robotics Center | Nylon tendons through joint centres over metal pins, motors in the base | < 2,000 CHF | < 8 h | ~1.3 kg assembled (ROBOTIS edition) | 10,000 cycles / 20 h without hardware failure; FSR fingertips degraded after thousands of cycles | MIT | [paper](https://www.alphaxiv.org/abs/2504.04259), [specs](https://roboticscenter.ai/hardware/orca-hand/specs), [ROBOTIS](https://robotis.us/orca-v1-hand-fully-assembled) |
| RUKA v2 (NYU, 2026) | 16 actuated of 20, incl. per-finger abduction and a decoupled 2-DOF parallel wrist | Forearm motors, one tendon per abduction with spring return | Tendons through a hole in the spherical wrist centre, bearing routing plate | < $1,500 | n/s | n/s | Payload limits: 1.2 kg DIP/PIP, 0.78 kg MCP, 0.84 kg thumb | open | [arXiv](https://arxiv.org/html/2603.26660v1) |
| RUKA v1 (NYU, 2025) | 15 underactuated | Dynamixel, in the base | Tendon | < $1,300 | < 7 h | n/s | learned joint-to-motor models from a MANUS glove | open | [site](https://ruka-hand.github.io) |
| LEAP Hand v1 (CMU) | 16 (4 fingers) | 16 Dynamixel XC330 in the fingers | Direct, in-joint | ~$2,000 | ~ | ~0.6 kg class | the default lab hand, Isaac/MuJoCo models everywhere | MIT | [Robotics Center](https://www.roboticscenter.ai/learn/dexterous-robot-hand-comparison-2026) |
| LEAP v2 / MOTIF (USC) | 12 per page, 5 fingers | 12 Dynamixel RX-10 class | Tendon with silicone friction valleys | ~$2,100 BOM | ~ | n/s | aluminium palm stiffener, brass bearings | open | [EmergentMind](https://www.emergentmind.com/topics/leap-hand-v2) |
| BiDexHand (2025) | 16 actuated, 21 joints | 16 servos in a proximal sleeve | Endless pull-pull loops in PTFE guides, spring tensioner | "low cost" | n/s | n/s | 2.1 N tip, 4.5 kg hook lift, 9 of 11 Kapandji | open | [arXiv](https://arxiv.org/html/2504.14712v2) |
| ISyHand (MPI, 2025) | n/s | on-joint Dynamixel | direct | ~$1,300 | < 4 h | n/s | | open | [site](https://isyhand.is.mpg.de/) |
| Amazing Hand (Pollen/Seeed, 2025) | 8 (2 per finger, 4 fingers) | 8 Feetech SCS0009 in the fingers, parallel linkage | linkage | $89 kit | hours | ~400 g | toy-class torque, ±20° lateral | open | [CNX](https://www.cnx-software.com/2025/12/02/amazing-hand-a-8-dof-3d-printable-open-source-robotic-hand-for-prototyping-and-research/) |
| DexHand V2 (TheRobotStudio / Rob Knight) | n/s | n/s | tendon | n/s | | | CERN-OHL-S, but the repo is private early access | closed for now | [repo](https://github.amrom.workers.dev/TheRobotStudio/V2_DexHand) |
| InMoov hand | 5 + wrist | hobby servos in the forearm | fishing-line tendons | ~$150 | | | the 2012 maker baseline, weak and sloppy by today's standard | CC | [InMoov](https://inmoov.fr/inmoov-hand) |

Takeaway: ORCA is the best-documented, most-tested open hand with a real wrist flexion joint and an MIT licence. RUKA v2 has the best wrist (2 DOF, decoupled) and real abduction. LEAP v1 is the simplest but has no thumb opposition worth the name and no fifth finger. Everything open-source runs on hobby bus servos (Dynamixel X-series or Feetech STS) and costs $1.3k to $2k.

## 4. Technology state

### 4.1 Actuation architectures

| Architecture | Who | Pros | Cons | Buildable by us? |
|---|---|---|---|---|
| **Tendon, motors behind the wrist** | Optimus, NEO, Shadow, ORCA, RUKA, LEAP v2, BiDexHand | Light, fast fingers; motors sized for grip not fit; heat out of the hand; human-like finger shape; inherent compliance | Tendons creep, lose pretension, abrade at pulleys; wrist motion changes tendon length (crosstalk) unless routed through the wrist centre; re-tensioning is maintenance | Yes. The whole open-source field does it with printed phalanges, nylon or UHMWPE line, and $25 to $60 bus servos. |
| **Linkage, micro linear screw actuators in the palm** | Inspire, Zhaowei, LinkerBot, Leili, Tesla Gen 2 | Stiff, precise (±0.2 mm), self-locking (grip holds with power off), nothing crosses the wrist, cheap at volume, easy to bolt on | 6 to 12 DOF with coupled fingers; shocks go straight into the screw; needs force sensing for compliance; coupled linkages mean less independent fingertip control | Yes, with Actuonix PQ12 (50 N, 20 mm stroke, 22 g, ~$190) or Firgelli/MightyZap pen actuators. Inspire's own LAF actuators are not sold separately. |
| **In-finger motors** | Figure, Unitree Dex5, Wuji, LEAP v1, Allegro | Fully independent joints, no tendons, torque sensing in the joint | Needs sub-20 mm gearmotors with encoders; fingers fat and heavy; 1 kg hands | Only with Dynamixel XL330 (18 g, 0.52 N·m) in the fingers, LEAP style. Fingers come out ~25 mm thick. Not human-looking. |
| **Hydraulic** | Sanctuary | Force density, compliance | Fluid, seals, pump, weight | No. |

Within tendon hands the industry is converging on "tendon plus rigid hybrids": tendons for flexion, springs or a second tendon for extension, rigid printed phalanges with bearings or pins at the joints, and tendons routed through the joint centre of rotation so wrist motion does not pull the fingers.

### 4.2 Tendon materials (from [humanoid.guide](https://humanoid.guide/tendon-driven-humanoid-hands/) and the China tracker)

- UHMWPE (Dyneema, Spectra fishing braid): dominant. ~15× steel strength-to-weight, low friction, but creeps under cyclic load and softens above 90 °C.
- Aramid (Kevlar, Technora): less creep, abrades faster at pulleys.
- Nylon monofilament: what ORCA used; cheap, stretchy, easy to knot and re-tension.
- Steel cable: heat-tolerant but fatigues at finger-scale bend radii.
- Failure modes to design for: creep (lose calibration), lost pretension, abrasion at pins, flex fatigue, crosstalk at the wrist. Mitigations: spring tensioners (BiDexHand), routing through joint centres (ORCA, RUKA v2), replaceable finger modules, re-tension procedure.

### 4.3 Servos available to us

| Servo | Mass | Stall torque | Volts | Bus | Price | Notes | Source |
|---|---|---|---|---|---|---|---|
| Dynamixel XL330-M288-T | 18 g | 0.52 N·m at 5 V | 3.7 to 6 V | TTL half-duplex, to 4 Mbps | $27 | plastic gears, 20 × 34 × 26 mm, used by LEAP v1 and many hands | [ROBOTIS](https://emanual.robotis.com/docs/en/dxl/x/xl330-m288/), [robotis.us](https://robotis.us/xl/) |
| Dynamixel XC330-M288-T | 23 g | ~0.93 N·m at 5 V | 3.7 to 6 V | TTL | ~$90 | metal gears, ORCA's motor | [Generation Robots](https://generationrobots.com/en/403889-dynamixel-xc330-m288-t-servo.html) |
| Feetech STS3215 | ~55 g | ~1.9 N·m at 7.4 V (12 V variant ~3 N·m) | 6 to 12 V | TTL, Dynamixel-like protocol | ~$15 to $20 | ORCA Feetech variant, SO-ARM100 motor; bulky for 17 units | [Robotics Center](https://roboticscenter.ai/hardware/orca-hand/specs) |
| Feetech SCS0009 | 10 g | 0.19 N·m | 5 V | TTL | ~$8 | Amazing Hand; too weak for a working hand | [CNX](https://www.cnx-software.com/2025/12/02/amazing-hand-a-8-dof-3d-printable-open-source-robotic-hand-for-prototyping-and-research/) |
| Actuonix PQ12-100 (linear) | 22 g | 50 N max, 20 N at 8 mm/s, 20 mm stroke | 6 or 12 V | PWM (R) or pot feedback (P) | ~$190 (A$295 inc. tax) | Inspire-style linkage fingers; back-drive force 35 N | [Core Electronics](https://core-electronics.com.au/actuonix-pq12-100-6-r-micro-actuator.html.plain.md) |

**Our calculation, tendon force to fingertip force:** a finger with a 6 mm tendon moment arm at the MCP and a 40 mm proximal phalanx converts 0.5 N·m of servo torque on a 10 mm spool (50 N tendon) into roughly 50 × 6 / 40 ≈ 7.5 N at the PIP, before friction. That is the Inspire/LinkerBot class (8 to 12 N tip). An XL330 is marginal; an XC330 or STS3215 is comfortable. Fingertip forces above 20 N (NEO 45 N) need gear ratios or motors we cannot buy at this size.

### 4.4 Tactile sensing ([Robotics Center guide](https://www.roboticscenter.ai/guides/best-tactile-sensors-robot-learning/))

| Option | Principle | Cost | Fit for a printed hand |
|---|---|---|---|
| FSR under silicone (ORCA) | resistive | $2 to $5 per tip | Day one. Wears out after thousands of cycles; make tips replaceable. |
| AnySkin / ReSkin (Meta, NYU) | magnetometer under magnetic elastomer | DIY parts, ~$10s per patch | Best open option for pads and palm; needs EMI care near motors and magnetic encoders (RUKA v2 flags this). |
| DIGIT / GelSight Mini | camera behind gel | < $1k DIY, low $k commercial | Fingertip-sized but one USB camera per tip; two tips at most on a Pi. |
| Xela uSkin, Contactile | 3-axis taxels, capacitive papillae | $k to $10k | Commercial; skip for now. |
| Chinese pressure arrays (Unitree 94 pads, LinkerBot 16 regions) | resistive film | ¥199 class in China | Hard to buy separately today. |

Frontier hands treat tactile as mandatory (NEO slip detection, Figure 3 g threshold). For us, v1 is FSR tips plus servo current as a torque proxy; v2 is AnySkin pads.

### 4.5 DOF reality check

- A human hand has about 27 DOF. Shadow has 20 actuated. The frontier is 22 to 25. Commercial mid-tier is 6 to 16 active. Research shows most grasps live in 2 to 3 postural synergies, which is why 6-actuator linkage hands (Inspire, Psyonic) do real work.
- The thumb is the whole game: 4 DOF (CMC flexion, CMC abduction, MCP, IP) with true opposition is what separates a hand that can pinch, key-grip and manipulate from a gripper with four extra fingers. Every serious open design (ORCA, RUKA, BiDexHand) spends 4 of its DOF there.
- Per-finger abduction is the next thing people add (RUKA v2) and the first thing cheap hands drop.

## 5. What this means for Marengo

### 5.1 Buy versus build

| | Buy mid-tier (LinkerBot L6 €4.9k, Inspire ~$9k, Dex3 ~$7.8k) | Build open-source as-is (ORCA, ~$2k) | Design our own (ORCA-derived, ~$1.5k to $2.5k) |
|---|---|---|---|
| Time to a working hand | weeks (shipping, adapter, 24 V rail, protocol bridge) | 2 to 3 weekends after parts arrive | months, with gates |
| DOF | 6 to 12 | 17 incl. wrist flexion | 16 to 18 incl. wrist flexion, our choice |
| Mass | 540 to 800 g | ~1.3 kg (ORCA assembled) | target < 800 g by trimming the motor block and using XC330s |
| Looks | industrial, grey, visibly a product | lab hand, exposed servos | ours, shell-matched, male proportions |
| Fits the "badass left arm" brief and the human-like-form rule | no | partly | yes |
| Repairability | vendor | print and replace | print and replace |
| Risk | protocol and power integration, two hands = two purchases | known design, known failure modes | tendon tuning, our own bugs |

Buying a LinkerBot L6 pair (€9.9k) buys the least learning and the least look. Building ORCA as-is buys the most learning per dollar but is 1.3 kg and looks like a lab hand. Designing our own on ORCA's bones is the path that matches how the arm is being done.

### 5.2 Mass budget (our calculation, needs Joey's number)

Nothing in the arm torque work includes a hand. With the 215 / 195 skeleton and a hand centre of mass about 100 mm past the RS00 output:

| Hand mass | Extra elbow torque, arm horizontal | Extra shoulder pitch torque | Against RS02 rated 6 / peak 17 N·m and RS03 rated 20 / peak 60 N·m |
|---|---|---|---|
| 0.5 kg (Inspire class) | 1.4 N·m | 2.5 N·m | comfortable |
| 0.8 kg (our target) | 2.3 N·m | 4.0 N·m | fine, but it is a third of the RS02 rating on its own |
| 1.3 kg (ORCA as built) | 3.8 N·m | 6.5 N·m | eats most of the RS02 continuous rating before payload |

A human hand is about 0.6 % of body mass, roughly 400 to 500 g. A hard cap of 800 g including the wrist module, servos, DC-DC and MCU keeps the elbow honest and leaves payload. The upper-arm and forearm threads should carry this number in their URDF and FEA load cases now.

### 5.3 The missing wrist

The arm has pronation/supination (RS00) but no wrist flexion or deviation. Humans use wrist flexion constantly for orientation. Three ways to get it:

1. **Hand module carries one wrist flexion servo** (ORCA style, 1 DOF, in the tendon motor block). Cheapest, keeps the arm at five RobStrides. Recommended for v1.
2. **Hand module carries a 2-DOF parallel wrist** (RUKA v2 style). Better, heavier, more tendons through the wrist centre.
3. **Add a real wrist actuator to the arm.** Out of scope: no spare RobStrides, and the forearm is frozen around the RS00.

Option 1 also gives the hand a natural place to put the servo pack: a printed "wrist block" bolted to the Ø50 wrist flange, housing 17 servos in two rows, the DC-DC, and the MCU, with the flexion joint at its distal end and the palm beyond. That block is also where the tendons get re-tensioned and where a service door goes.

### 5.4 Electrical integration (hand-off to the power-board and trunk threads)

- One fused 48 V XT30 drop at the forearm feeds a 48 V to 12 V DC-DC (60 W class, 5 A) inside the wrist block; a second 12 V to 5 V stage if we use XL330/XC330 (5 V, 1.5 A stall each, so 17 servos need a current-limited supply and software torque caps).
- One CAN drop at the forearm. A small MCU (RP2040 or STM32 with CAN) bridges CAN to the servo TTL bus and reads the FSRs, and the hand enumerates as one CAN node. The fuse that protects the hand is the hand's own.
- Keep the RS00 plug clearance and the forearm fuse pocket already listed in the forearm README.

### 5.5 Print and structure

- Phalanges and palm at 100 % infill PETG or PA-CF for the pin bosses; tendon paths need PTFE tube liners or polished pins, not raw printed channels.
- Joints on 2 or 3 mm steel pins in printed bosses (ORCA) or brass sleeves (LEAP v2). No bearings in the fingers; too heavy.
- Every finger a plug-in module with its own tendon terminations so a broken finger is a 20-minute swap. Tip-over case: a finger breaks, not the servo or the RS00.
- Fingertips: silicone over-moulded or TPU-printed caps on FSRs, replaceable.

## 6. Recommended path

### 6.1 Phase 0, this month: learn on ORCA's bones (~$600 to $900)

Print ORCA's palm, one full finger and the thumb from the MIT repo on the P2S, drive them with 7 XC330s (or STS3215s if we want 12 V and cheaper) on a U2D2/USB adapter, measure fingertip force, tendon creep over 1,000 cycles, and servo temperature. This answers the three questions no paper answers for us: what a printed tendon finger from our printer actually does, how fast nylon versus Dyneema creeps, and how hot 5 V servos get in a closed block. Record it like the elbow dummy test.

### 6.2 Phase 1: Marengo hand v1 (the build)

- 16 finger DOF (4 thumb, 3 × 4 fingers: MCP flex, MCP abduct, PIP flex with DIP coupled) + 1 wrist flexion = 17 servos, ORCA layout, our own palm and phalanx geometry at male proportions and a shell-matching outer skin.
- Wrist block on the Ø50 six-hole wrist flange (turning tube on the RS00 output), servos in two rows, DC-DC and CAN MCU inside, service door, spring tensioners.
- Mass target 800 g at the flange. Parts ~$1.5k to $2.5k per hand (17 × XC330 is the bulk).
- Gate 1 (layout freeze): finger geometry, tendon routing diagram, mass estimate, wrist-block envelope against the forearm fold. Gate 2 before first prints. Gate 3 before it goes on the arm.
- Same adversarial reviewer set as the arm.

### 6.3 Fallback: linkage hand with Actuonix PQ12s

If tendons fight us, an Inspire-style 6-actuator hand (one PQ12 per finger, two for the thumb) in a printed palm gives 8 to 12 N tips, self-locking grip, no tendons, and about 600 g. Lower dexterity, but it is the design that ships in volume today, and the arm interface, power and CAN bridge are identical, so nothing upstream changes.

### 6.4 What to decide now

1. Hand mass cap (proposed 800 g at the RS00 flange) and hand length (proposed 180 to 195 mm flange to middle fingertip, human male is about 190 mm) so the arm threads can load their models.
2. Wrist flexion inside the hand module: yes or no (proposed yes, 1 DOF).
3. Servo family: Dynamixel XC330 (23 g, metal gears, 5 V, ~$90) versus Feetech STS3215 (55 g, 12 V, ~$18). XC330 keeps the hand light; STS3215 saves ~$1.2k per hand and runs straight off 12 V. Proposed XC330 for the fingers, STS3215 for the wrist flexion.
4. Approve Phase 0 parts spend (~$600 to $900).

## Sources

Market and comparisons: [Robotics Center comparison 2026](https://www.roboticscenter.ai/learn/dexterous-robot-hand-comparison-2026), [SourceBotics Chinese hands 2026](https://sourcebotics.com/guides/chinese-dexterous-hands-2026-buyers-guide/), [China Humanoid Robotics Tracker, robot hands](https://robotics.techbuzzchina.com/reports/robot-hands-china.html), [Wikipedia humanoid hand](https://en.wikipedia.org/wiki/Humanoid_hand), [Shadow, cost of a robot hand](https://shadowrobot.com/how-much-does-a-robot-hand-cost/), [humanoid.guide tendon hands](https://humanoid.guide/tendon-driven-humanoid-hands/), [humanoid.guide linkage fingers](https://humanoid.guide/?p=19332).
Products: [1X NEO hands](https://1x.tech/discover/neos-hands), [Teslarati Optimus V3 patents](https://www.teslarati.com/tesla-optimus-v3-hand-arm-details-revealed-new-patents/), [Figure 03](https://www.notebookcheck.net/Figure-03-Humanoid-debuts-with-wireless-charging-and-ultra-sensitive-tactile-palms.1134940.0.html), [LinkerBot L6](https://openelab.io/products/linker-hand-linkerbot-l6-11), [LinkerBot L20 Lite](https://openelab.io/products/linker-hand-linkerbot-l20-lite), [LinkerBot O6](https://www.roboticscenter.ai/hardware/linker-bot-o6), [Inspire RH56DFX](https://www.knoxlabs.com/products/inspire-robots-rh56dfx-dexterous-hand), [Unitree Dex3-1](https://www.unitree.com/Dex3-1/), [Unitree Dex5-1](https://humanoid.guide/product/dex5-1/), [Psyonic Ability Hand](https://humanoid.guide/product/ability-hand/), [Sharpa Wave](https://www.cnx-software.com/2026/06/02/sharpa-wave-high-end-dexterous-robotic-hand-with-22-dof-high-sensitivity-dynamic-tactile-array/).
Open source: [ORCA paper](https://www.alphaxiv.org/abs/2504.04259), [ORCA specs](https://roboticscenter.ai/hardware/orca-hand/specs), [ORCA ROBOTIS edition](https://robotis.us/orca-v1-hand-fully-assembled), [RUKA v2](https://arxiv.org/html/2603.26660v1), [RUKA v1](https://ruka-hand.github.io), [LEAP v2 / MOTIF](https://www.emergentmind.com/topics/leap-hand-v2), [BiDexHand](https://arxiv.org/html/2504.14712v2), [ISyHand](https://isyhand.is.mpg.de/), [Amazing Hand](https://www.cnx-software.com/2025/12/02/amazing-hand-a-8-dof-3d-printable-open-source-robotic-hand-for-prototyping-and-research/), [DexHand V2](https://github.amrom.workers.dev/TheRobotStudio/V2_DexHand), [InMoov](https://inmoov.fr/inmoov-hand).
Components: [XL330 e-manual](https://emanual.robotis.com/docs/en/dxl/x/xl330-m288/), [XC330 listing](https://generationrobots.com/en/403889-dynamixel-xc330-m288-t-servo.html), [Actuonix PQ12](https://core-electronics.com.au/actuonix-pq12-100-6-r-micro-actuator.html.plain.md), [tactile sensor guide](https://www.roboticscenter.ai/guides/best-tactile-sensors-robot-learning/).
