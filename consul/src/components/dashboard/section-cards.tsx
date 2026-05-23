import { CanBusCard } from '@/components/dashboard/cards/can-bus-card';
import { ControlLoopCard } from '@/components/dashboard/cards/control-loop-card';
import { JetsonHostCard } from '@/components/dashboard/cards/jetson-host-card';
import { PiHostCard } from '@/components/dashboard/cards/pi-host-card';

const sectionGridClassName =
  'grid grid-cols-1 gap-4 px-4 *:data-[slot=card]:bg-linear-to-t *:data-[slot=card]:from-primary/5 *:data-[slot=card]:to-card *:data-[slot=card]:shadow-xs lg:px-6 @xl/main:grid-cols-2 @5xl/main:grid-cols-4 dark:*:data-[slot=card]:bg-card';

export function SectionCards() {
  return (
    <div className={sectionGridClassName}>
      <PiHostCard />
      <JetsonHostCard />
      <CanBusCard />
      <ControlLoopCard />
    </div>
  );
}
