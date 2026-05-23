/** Dummy power board metrics — replace with shunt telemetry from custom board later. */

export type PowerBoardMetrics = {
  boardName: string;
  busVoltageV: number;
  shuntCurrentA: number;
  powerW: number;
  energyWhSession: number;
  boardTempC: number;
  shuntRatingA: number;
  peakPowerW: number;
  healthy: boolean;
  detail: string;
};

export const dummyPowerBoardMetrics: PowerBoardMetrics = {
  boardName: 'marengo-power',
  busVoltageV: 47.8,
  shuntCurrentA: 12.4,
  powerW: 592.7,
  energyWhSession: 2.38,
  boardTempC: 38.2,
  shuntRatingA: 150,
  peakPowerW: 841,
  healthy: true,
  detail: 'INA226 shunt · 48 V main bus · no faults',
};
