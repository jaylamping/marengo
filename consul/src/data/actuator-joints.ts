/** Per-joint caps as published by Davout in the live ActuatorLimitSnapshot. */
export type JointCapLimits = {
  kpMax: number;
  kdMax: number;
  velocityMaxRadS: number;
  tauFfMaxNm: number;
};
