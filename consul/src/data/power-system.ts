import {
  dummyBatterySystemMetrics,
  type BatterySystemMetrics,
} from '@/data/battery-metrics';
import {
  dummyPowerBoardMetrics,
  type PowerBoardMetrics,
} from '@/data/power-metrics';

export type PowerSystemMetrics = {
  board: PowerBoardMetrics;
  batteries: BatterySystemMetrics;
};

export const dummyPowerSystemMetrics: PowerSystemMetrics = {
  board: dummyPowerBoardMetrics,
  batteries: dummyBatterySystemMetrics,
};
