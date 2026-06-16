import React, { createContext, useContext, useMemo } from 'react';

import type { RobotModel } from '@/urdf/parse-urdf';
import { getRobotModel } from '@/urdf/parse-urdf';

const RobotModelContext = createContext<RobotModel | null>(null);

type RobotModelProviderProps = {
  children: React.ReactNode;
  urdfXml: string;
};

export function RobotModelProvider({ children, urdfXml }: RobotModelProviderProps) {
  const model = useMemo(() => getRobotModel(urdfXml), [urdfXml]);

  return (
    <RobotModelContext.Provider value={model}>
      {children}
    </RobotModelContext.Provider>
  );
}

export function useRobotModel(): RobotModel {
  const context = useContext(RobotModelContext);
  if (!context) {
    throw new Error('useRobotModel must be used within a RobotModelProvider');
  }
  return context;
}

export function useJoint(jointName: string) {
  const model = useRobotModel();
  return model.getJoint(jointName);
}

export function useChainFromRoot(jointName: string) {
  const model = useRobotModel();
  return model.getChainFromRoot(jointName);
}
