import { urdfPreviewPanelClassName } from '@/components/dashboard/layout/constants';

export function UrdfPreviewPanel() {
  return (
    <div
      data-testid="urdf-preview-panel"
      className={urdfPreviewPanelClassName}
      aria-label="Robot preview viewport"
    />
  );
}
