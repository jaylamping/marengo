import { PiHostCard } from '@/components/dashboard/cards/pi-host-card';
import { PowerSystemCard } from '@/components/dashboard/cards/power-system-card';
import { sectionCardsGridClassName } from '@/components/dashboard/layout/constants';

export function SectionCards() {
  return (
    <div className={sectionCardsGridClassName} data-testid="section-cards-grid">
      <PiHostCard />
      <PowerSystemCard />
    </div>
  );
}
