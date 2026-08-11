import {
  SidebarIdentityRow,
  SidebarUpdateConfirmDialog,
  SidebarUpdateStatusView,
} from '@/components/dashboard/sidebar/sidebar-update-status-view';
import { useSidebarSelfUpdate } from '@/components/dashboard/sidebar/use-sidebar-self-update';
import type { SidebarUser } from '@/data/sidebar-nav';

type SidebarUpdateStatusProps = {
  user: SidebarUser;
};

/** Identity row (Update when stale) + rev/Check status + confirm dialog. */
export function SidebarUpdateStatus({ user }: SidebarUpdateStatusProps) {
  const update = useSidebarSelfUpdate();

  return (
    <>
      <div className="flex w-full flex-col">
        <SidebarIdentityRow
          user={user}
          showUpdate={update.showUpdate}
          onUpdate={update.openConfirm}
        />
        <SidebarUpdateStatusView
          mode={update.mode}
          caption={update.caption}
          shaTitle={update.shaTitle}
          checking={update.checking}
          error={update.error}
          onCheck={update.onCheck}
        />
      </div>
      <SidebarUpdateConfirmDialog
        open={update.confirmOpen}
        deployBusy={update.deployBusy}
        upstreamSha={update.status?.upstream_sha || undefined}
        deploySha={update.status?.deploy_sha || undefined}
        onOpenChange={update.setConfirmOpen}
        onCancel={() => update.setConfirmOpen(false)}
        onConfirm={update.onConfirmUpdate}
      />
    </>
  );
}
