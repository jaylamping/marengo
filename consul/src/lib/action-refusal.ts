import type { ActionEvent } from '@/gen/marengo/v1/marengo_pb';

/**
 * Operator text for a Pi `ActionEvent` the runtime refused (`motion_refused`,
 * a refused limit patch, …), or null for an accepted one. A gateway 200 only
 * means "published"; this is where a refusal becomes visible.
 */
export function formatActionRefusal(event: ActionEvent): string | null {
  if (event.accepted) {
    return null;
  }
  const action = event.action.trim() || 'command';
  const joint = event.joint.trim() ? ` (${event.joint.trim()})` : '';
  const reason = event.rejectReason.trim() || 'no reason given';
  return `Pi refused ${action}${joint}: ${reason}`;
}
