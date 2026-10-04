"use client";

import { createContext, useContext, useState, type ReactNode } from "react";
import {
  getClientConfig,
  initializeClientConfig,
  type AppConfig,
} from "./config";

const ConfigContext = createContext<AppConfig | null>(null);

export function ConfigProvider({
  initialConfig,
  children,
}: {
  initialConfig: AppConfig;
  children: ReactNode;
}) {
  const [value] = useState(() => initializeClientConfig(initialConfig));

  return (
    <ConfigContext.Provider value={value}>{children}</ConfigContext.Provider>
  );
}

export function useConfig(): AppConfig {
  return useContext(ConfigContext) ?? getClientConfig();
}
