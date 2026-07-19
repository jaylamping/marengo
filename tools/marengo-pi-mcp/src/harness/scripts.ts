/**
 * Data-driven marengo-pi pipe scripts for bench harness profiles.
 * Add/edit suites here — do not copy-paste profile blocks in harness/index.ts.
 */

import type { BenchProfile } from "../bench-profiles.js";
import { profileMeta } from "../bench-profiles.js";

const ROLL_JOINT = "right_shoulder_roll";
const PITCH_JOINT = "right_shoulder_pitch";
const YAW_JOINT = "right_upper_arm_yaw";

function holdAt(joint: string, rad: number): string {
  return `hold-at ${joint} ${rad}`;
}

function rollReturnHomeLines(): string[] {
  return [
    holdAt(ROLL_JOINT, 0.05),
    "sleep 15",
    holdAt(ROLL_JOINT, 0.02),
    "sleep 15",
    holdAt(ROLL_JOINT, 0),
    "sleep 25",
  ];
}

export type HarnessPassKind = "smoke" | "commissioning";

export interface HarnessScript {
  name: string;
  lines: string[];
  timeoutSec: number;
}

export interface HarnessScriptSuite {
  /** Smoke = fault/exit heuristics only; commissioning = metric gates (e.g. ±50 mrad). */
  passKind: HarnessPassKind;
  /** When true, green smoke must not be treated as Y3–Y4 / wave unlock. */
  operatorSignoffRequired: boolean;
  /** Informational step prepended before scripts (always ok). */
  note?: { name: string; output: string };
  scripts: HarnessScript[];
}

/** Exhaustive: every right-arm scripted profile must appear; others return null. */
const SCRIPT_SUITES: Partial<Record<BenchProfile, () => HarnessScriptSuite>> = {
  roll_attached: () => {
    const pitchHold0 = holdAt(PITCH_JOINT, 0);
    return {
      passKind: "smoke",
      operatorSignoffRequired: false,
      scripts: [
        {
          name: "roll_sign_probe",
          timeoutSec: 40,
          lines: [
            "home",
            "enable bench",
            pitchHold0,
            "sleep 2",
            holdAt(ROLL_JOINT, 0.15),
            "sleep 8",
            ...rollReturnHomeLines(),
            "status",
            "disable",
            "quit",
          ],
        },
        {
          name: "roll_hold_sweep",
          timeoutSec: 90,
          lines: [
            "home",
            "enable bench",
            pitchHold0,
            "sleep 2",
            holdAt(ROLL_JOINT, 0.15),
            "sleep 12",
            holdAt(ROLL_JOINT, 0.785),
            "sleep 15",
            holdAt(ROLL_JOINT, 1.2),
            "sleep 15",
            holdAt(ROLL_JOINT, 1.57),
            "sleep 15",
            ...rollReturnHomeLines(),
            "status",
            "disable",
            "quit",
          ],
        },
        {
          name: "roll_round_trip",
          timeoutSec: 70,
          lines: [
            "home",
            "enable bench",
            pitchHold0,
            "sleep 2",
            holdAt(ROLL_JOINT, 1.57),
            "sleep 25",
            ...rollReturnHomeLines(),
            "status",
            "disable",
            "quit",
          ],
        },
      ],
    };
  },

  arm_2dof_smoke: () => ({
    passKind: "smoke",
    operatorSignoffRequired: false,
    scripts: [
      {
        name: "smoke_pitch_hold",
        timeoutSec: 40,
        lines: [
          "home",
          "enable bench",
          holdAt(ROLL_JOINT, 0),
          "sleep 2",
          holdAt(PITCH_JOINT, 0.3),
          "sleep 10",
          holdAt(PITCH_JOINT, 0),
          "sleep 10",
          "status",
          "disable",
          "quit",
        ],
      },
      {
        name: "smoke_roll_hold",
        timeoutSec: 45,
        lines: [
          "home",
          "enable bench",
          holdAt(PITCH_JOINT, 0),
          "sleep 2",
          holdAt(ROLL_JOINT, 0.785),
          "sleep 15",
          holdAt(ROLL_JOINT, 0),
          "sleep 15",
          "status",
          "disable",
          "quit",
        ],
      },
      {
        name: "smoke_cross_talk",
        timeoutSec: 50,
        lines: [
          "home",
          "enable bench",
          holdAt(ROLL_JOINT, 0),
          "sleep 2",
          holdAt(PITCH_JOINT, 0.3),
          "sleep 8",
          "status",
          holdAt(PITCH_JOINT, 0),
          "sleep 8",
          holdAt(ROLL_JOINT, 0.785),
          "sleep 8",
          "status",
          holdAt(ROLL_JOINT, 0),
          "sleep 8",
          "disable",
          "quit",
        ],
      },
    ],
  }),

  yaw_attached: () => {
    const pitchHold0 = holdAt(PITCH_JOINT, 0);
    const rollHold0 = holdAt(ROLL_JOINT, 0);
    return {
      passKind: "smoke",
      operatorSignoffRequired: true,
      note: {
        name: "yaw_suite_smoke_note",
        output:
          "yaw_attached harness is smoke (fault/exit heuristics only). " +
          "Y3–Y4 ±50 mrad / pitch-drift sign-off requires operator review of " +
          "position-trace-latest.csv + candump — see docs/bench-yaw-test-suite.md. " +
          "JSON pass=true with pass_kind=smoke is NOT commissioning complete.",
      },
      scripts: [
        {
          name: "yaw_sign_probe",
          timeoutSec: 55,
          lines: [
            "home",
            "enable bench",
            pitchHold0,
            rollHold0,
            "sleep 2",
            holdAt(YAW_JOINT, 0.15),
            "sleep 8",
            holdAt(YAW_JOINT, 0),
            "sleep 6",
            holdAt(YAW_JOINT, -0.15),
            "sleep 8",
            holdAt(YAW_JOINT, 0),
            "sleep 8",
            "status",
            "disable",
            "quit",
          ],
        },
        {
          name: "yaw_hold_ladder",
          timeoutSec: 120,
          lines: [
            "home",
            "enable bench",
            pitchHold0,
            rollHold0,
            "sleep 2",
            holdAt(YAW_JOINT, 0.3),
            "sleep 12",
            holdAt(YAW_JOINT, 0.6),
            "sleep 12",
            holdAt(YAW_JOINT, 0),
            "sleep 10",
            holdAt(YAW_JOINT, -0.3),
            "sleep 12",
            holdAt(YAW_JOINT, -0.6),
            "sleep 12",
            holdAt(YAW_JOINT, 0),
            "sleep 12",
            "status",
            "disable",
            "quit",
          ],
        },
        {
          name: "yaw_cross_talk",
          timeoutSec: 90,
          lines: [
            "home",
            "enable bench",
            rollHold0,
            "sleep 2",
            holdAt(PITCH_JOINT, 0.3),
            "sleep 8",
            holdAt(YAW_JOINT, 0.3),
            "sleep 12",
            holdAt(YAW_JOINT, 0),
            "sleep 8",
            holdAt(YAW_JOINT, -0.3),
            "sleep 12",
            holdAt(YAW_JOINT, 0),
            "sleep 8",
            holdAt(PITCH_JOINT, 0),
            "sleep 10",
            "status",
            "disable",
            "quit",
          ],
        },
      ],
    };
  },

  bare_motor: () => ({
    passKind: "smoke",
    operatorSignoffRequired: false,
    scripts: [
      {
        name: "right_only_gravity",
        timeoutSec: 35,
        lines: [
          "home",
          "enable bench",
          "status",
          "gravity-on",
          "status",
          "disable",
          "quit",
        ],
      },
      {
        name: "left_only_gravity",
        timeoutSec: 35,
        lines: [
          "home",
          "enable bench",
          "status",
          "gravity-on",
          "status",
          "disable",
          "quit",
        ],
      },
      {
        name: "both_gravity",
        timeoutSec: 35,
        lines: [
          "home",
          "enable bench",
          "status",
          "gravity-on",
          "status",
          "disable",
          "quit",
        ],
      },
    ],
  }),
};

export function harnessScriptSuite(profile: BenchProfile): HarnessScriptSuite | null {
  const factory = SCRIPT_SUITES[profile];
  return factory ? factory() : null;
}

/** Default pass kind when profile has no script suite (weighted angles path, etc.). */
export function defaultPassKind(profile: BenchProfile): HarnessPassKind {
  void profileMeta(profile);
  return "smoke";
}
