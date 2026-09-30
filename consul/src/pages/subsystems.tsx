import { Navigate } from 'react-router-dom';

/** Legacy Inventory URL — permanent client redirect to Telemetry. */
export function SubsystemsPage() {
  return <Navigate to="/telemetry" replace />;
}
