import { simHeroShellVariant } from '@/components/dashboard/simulation/constants';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { HugeiconsIcon } from '@hugeicons/react';
import { ThreeDViewIcon } from '@hugeicons/core-free-icons';

export function SimViewportPlaceholder() {
  return (
    <Card variant={simHeroShellVariant} className="flex min-h-80 flex-col">
      <CardHeader>
        <CardDescription>Viewport</CardDescription>
        <CardTitle className="text-lg font-semibold">Isaac Sim stage</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-1 flex-col">
        <div className="flex flex-1 flex-col items-center justify-center gap-3 rounded-lg border border-dashed bg-card/90 px-6 py-12 text-center">
          <HugeiconsIcon
            icon={ThreeDViewIcon}
            strokeWidth={1.5}
            className="size-10 text-muted-foreground"
          />
          <div className="space-y-1">
            <p className="text-sm font-medium">Live viewport stream</p>
            <p className="max-w-md text-sm text-muted-foreground">
              WebRTC or Kit remote viewport lands here. Scrub time, orbit camera, and
              select links against the same cursor as Overview plots.
            </p>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
