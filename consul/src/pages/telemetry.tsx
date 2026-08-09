/**
 * Telemetry route shell — Phase 1 IA stub.
 * Full read-only live table lands in Phase 2.
 */
export function TelemetryPage() {
  return (
    <div
      data-testid="telemetry-stub"
      className="flex flex-1 flex-col items-start gap-2 p-6"
    >
      <h2 className="text-sm font-medium text-foreground">Telemetry</h2>
      <p className="max-w-prose font-mono text-xs leading-relaxed text-faint">
        Live read-only master inventory. Commissioning actions live on Hardware.
      </p>
    </div>
  );
}
