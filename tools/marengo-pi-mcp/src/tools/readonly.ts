import { z } from "zod";
import type { MarengoPiConfig } from "../config.js";
import { shellQuote, wrapRemote } from "../env.js";

export function registerReadonlyTools(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
) {
  return {
    pi_health: {
      description:
        "Pi health: CAN links, marengo-pi binary, deploy rev, commissioned motors.yaml map",
      inputSchema: z.object({}),
      handler: async () => {
        const body = wrapRemote(
          cfg,
          [
            "echo '=== CAN interfaces ==='",
            "ip -br link show type can || true",
            "echo",
            "echo '=== marengo-pi binary ==='",
            "test -x bin/marengo-pi && ls -la bin/marengo-pi || echo 'missing bin/marengo-pi'",
            "echo",
            "echo '=== deploy rev ==='",
            "cat .deploy-rev 2>/dev/null || echo '(no .deploy-rev)'",
            "echo",
            "echo '=== git (may lag rsync) ==='",
            "git log -1 --oneline 2>/dev/null || echo '(not a git checkout)'",
            "echo",
            "echo '=== commissioned motors (can0/id2, can1/id12) ==='",
            `grep -E 'can_interface|device_id|joint:' ${shellQuote(`${cfg.configDir}/motors.yaml`)} 2>/dev/null || echo '(motors.yaml not found)'`,
            "echo",
            "echo '=== marengo-pi/motor-repl running? ==='",
            "pgrep -af 'marengo-pi|motor-repl' || echo '(none)'",
            "echo",
            "echo '=== systemd ==='",
            "systemctl is-active marengo-can marengo-pi 2>/dev/null || true",
            "echo",
            "echo '=== IMU (i2c-1 / imu-probe) ==='",
            "test -x bin/imu-probe && ls -la bin/imu-probe || echo '(missing bin/imu-probe)'",
            "i2cdetect -y 1 2>&1 | grep -E '4b|--' || i2cdetect -y 1 2>&1 || echo '(i2cdetect unavailable)'",
          ].join("\n"),
        );
        return runRemote(body, 30_000);
      },
    },

    pi_can_status: {
      description: "Detailed CAN interface statistics for can0 and can1",
      inputSchema: z.object({}),
      handler: async () => {
        const body = wrapRemote(
          cfg,
          [
            "for iface in can0 can1; do",
            "  echo \"=== ${iface} ===\"",
            "  ip -details -statistics link show dev \"${iface}\" 2>&1 || true",
            "  echo",
            "done",
          ].join("\n"),
        );
        return runRemote(body, 15_000);
      },
    },

    pi_motor_repl_status: {
      description: "motor-repl status (read-only, no sustained enable)",
      inputSchema: z.object({}),
      handler: async () => {
        const body = wrapRemote(cfg, "bin/motor-repl status");
        return runRemote(body, 30_000);
      },
    },

    pi_gravity_preview: {
      description: "motor-repl gravity-preview for joint angles (read-only tau_g)",
      inputSchema: z.object({
        angles: z
          .array(z.number())
          .optional()
          .describe("Joint angles rad; default [0, 0] for dual pitch"),
      }),
      handler: async (args: { angles?: number[] }) => {
        const angles = args.angles ?? [0, 0];
        const angleArgs = angles.map((a) => String(a)).join(" ");
        const body = wrapRemote(cfg, `bin/motor-repl gravity-preview ${angleArgs}`);
        return runRemote(body, 30_000);
      },
    },

    pi_read_file: {
      description: "Read allowlisted file from Pi (config, env, bench logs/json)",
      inputSchema: z.object({
        path: z.string().describe("Absolute path on Pi"),
        tail: z.number().int().min(1).max(5000).optional(),
      }),
      handler: async (args: { path: string; tail?: number }) => {
        const { isAllowedReadPath } = await import("../paths.js");
        if (!isAllowedReadPath(cfg, args.path)) {
          return `Error: path not allowlisted: ${args.path}`;
        }
        const cmd = args.tail
          ? `tail -n ${args.tail} ${JSON.stringify(args.path)}`
          : `cat ${JSON.stringify(args.path)}`;
        const body = wrapRemote(cfg, cmd);
        return runRemote(body, 30_000);
      },
    },

    pi_journal: {
      description: "journalctl for marengo-can and marengo-pi units",
      inputSchema: z.object({
        lines: z.number().int().min(1).max(500).default(50),
      }),
      handler: async (args: { lines: number }) => {
        const body = wrapRemote(
          cfg,
          `journalctl -u marengo-can -u marengo-pi -u marengo-gateway -n ${args.lines} --no-pager 2>&1 || echo '(systemd units not installed)'`,
        );
        return runRemote(body, 30_000);
      },
    },

    pi_gateway_health: {
      description:
        "marengo-gateway health (HTTP /health on localhost:8080; requires gateway unit or manual start)",
      inputSchema: z.object({}),
      handler: async () => {
        const body = wrapRemote(
          cfg,
          [
            "echo '=== marengo-gateway unit ==='",
            "systemctl is-active marengo-gateway 2>/dev/null || echo inactive",
            "echo",
            "echo '=== HTTP /health ==='",
            "curl -sf http://127.0.0.1:8080/health 2>&1 || echo '(gateway not reachable on :8080)'",
            "echo",
            "echo '=== socket ==='",
            "test -S /run/marengo/chappe.sock && ls -la /run/marengo/chappe.sock || echo '(no chappe.sock)'",
          ].join("\n"),
        );
        return runRemote(body, 15_000);
      },
    },

    pi_candump_once: {
      description: "Snapshot candump for 2s on can0,can1",
      inputSchema: z.object({}),
      handler: async () => {
        const body = wrapRemote(
          cfg,
          "timeout 2 candump -ta can0 can1 2>&1 || true",
        );
        return runRemote(body, 10_000);
      },
    },

    pi_imu_probe: {
      description:
        "Run imu-probe on Pi (BNO085 rotation quaternions; read-only hardware check)",
      inputSchema: z.object({
        bus: z.string().default("/dev/i2c-1"),
        address: z.string().default("4b"),
        samples: z.number().int().min(1).max(100).default(5),
      }),
      handler: async (args: {
        bus: string;
        address: string;
        samples: number;
      }) => {
        const body = wrapRemote(
          cfg,
          `test -x bin/imu-probe && bin/imu-probe --bus ${JSON.stringify(args.bus)} --address ${JSON.stringify(args.address)} --samples ${args.samples} || echo 'imu-probe missing or failed'`,
        );
        return runRemote(body, 30_000);
      },
    },
  };
}

export type ReadonlyTools = ReturnType<typeof registerReadonlyTools>;
