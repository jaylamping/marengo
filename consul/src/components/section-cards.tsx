import { Badge } from '@/components/ui/badge';
import {
  Card,
  CardAction,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { HugeiconsIcon } from '@hugeicons/react';
import { ChartUpIcon, ChartDownIcon } from '@hugeicons/core-free-icons';

export function SectionCards() {
  return (
    <div className="grid grid-cols-1 gap-4 px-4 *:data-[slot=card]:bg-linear-to-t *:data-[slot=card]:from-primary/5 *:data-[slot=card]:to-card *:data-[slot=card]:shadow-xs lg:px-6 @xl/main:grid-cols-2 @5xl/main:grid-cols-4 dark:*:data-[slot=card]:bg-card">
      <Card className="@container/card">
        <CardHeader>
          <CardDescription>Safety State</CardDescription>
          <CardTitle className="text-2xl font-semibold tabular-nums @[250px]/card:text-3xl">
            ENABLED
          </CardTitle>
          <CardAction>
            <Badge variant="outline" className="border-green-500/40 text-green-400">
              <HugeiconsIcon icon={ChartUpIcon} strokeWidth={2} />
              All gates
            </Badge>
          </CardAction>
        </CardHeader>
        <CardFooter className="flex-col items-start gap-1.5 text-sm">
          <div className="line-clamp-1 flex gap-2 font-medium">
            Davout path clear
            <HugeiconsIcon icon={ChartUpIcon} strokeWidth={2} className="size-4" />
          </div>
          <div className="text-muted-foreground">
            E-stop open · limits nominal · 0 violations
          </div>
        </CardFooter>
      </Card>
      <Card className="@container/card">
        <CardHeader>
          <CardDescription>Active Actuators</CardDescription>
          <CardTitle className="text-2xl font-semibold tabular-nums @[250px]/card:text-3xl">
            3 / 23
          </CardTitle>
          <CardAction>
            <Badge variant="outline">
              +1 tuning
            </Badge>
          </CardAction>
        </CardHeader>
        <CardFooter className="flex-col items-start gap-1.5 text-sm">
          <div className="line-clamp-1 flex gap-2 font-medium">
            Left arm bench slice
          </div>
          <div className="text-muted-foreground">
            29 devices inventoried · 6 limb groups
          </div>
        </CardFooter>
      </Card>
      <Card className="@container/card">
        <CardHeader>
          <CardDescription>CAN Bus</CardDescription>
          <CardTitle className="text-2xl font-semibold tabular-nums @[250px]/card:text-3xl">
            can0 OK
          </CardTitle>
          <CardAction>
            <Badge variant="outline">
              4 nodes
            </Badge>
          </CardAction>
        </CardHeader>
        <CardFooter className="flex-col items-start gap-1.5 text-sm">
          <div className="line-clamp-1 flex gap-2 font-medium">
            0.8 ms avg RTT
          </div>
          <div className="text-muted-foreground">
            RS03×2 · RS02×2 · no missed heartbeats
          </div>
        </CardFooter>
      </Card>
      <Card className="@container/card">
        <CardHeader>
          <CardDescription>Control Loop</CardDescription>
          <CardTitle className="text-2xl font-semibold tabular-nums @[250px]/card:text-3xl">
            500 Hz
          </CardTitle>
          <CardAction>
            <Badge variant="outline">
              <HugeiconsIcon icon={ChartDownIcon} strokeWidth={2} />
              1.2 ms p99
            </Badge>
          </CardAction>
        </CardHeader>
        <CardFooter className="flex-col items-start gap-1.5 text-sm">
          <div className="line-clamp-1 flex gap-2 font-medium">
            Berthier → Davout steady
          </div>
          <div className="text-muted-foreground">
            Last overruns: none this session
          </div>
        </CardFooter>
      </Card>
    </div>
  );
}
