import type { SidebarIconKey } from '@/data/sidebar-nav';
import { HugeiconsIcon } from '@hugeicons/react';
import {
  Analytics01Icon,
  ChartHistogramIcon,
  CircuitBoardIcon,
  DashboardSquare01Icon,
  Database01Icon,
  File01Icon,
  GameController01Icon,
  HelpCircleIcon,
  LeftToRightListBulletIcon,
  SearchIcon,
  Settings05Icon,
  Shield01Icon,
  ThreeDViewIcon,
} from '@hugeicons/core-free-icons';

const sidebarIcons: Record<SidebarIconKey, typeof ThreeDViewIcon> = {
  overview: DashboardSquare01Icon,
  simulation: GameController01Icon,
  visualizer: ThreeDViewIcon,
  subsystems: CircuitBoardIcon,
  safety: Shield01Icon,
  telemetry: ChartHistogramIcon,
  logs: LeftToRightListBulletIcon,
  settings: Settings05Icon,
  docs: HelpCircleIcon,
  search: SearchIcon,
  'preset-golden': Database01Icon,
  'preset-bench': Analytics01Icon,
  'preset-tuning': File01Icon,
};

type SidebarIconProps = {
  icon: SidebarIconKey;
  className?: string;
};

export function SidebarIcon({ icon, className }: SidebarIconProps) {
  return (
    <HugeiconsIcon icon={sidebarIcons[icon]} strokeWidth={2} className={className} />
  );
}
