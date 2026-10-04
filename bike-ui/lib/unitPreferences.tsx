"use client";

import { createContext, useContext, type ReactNode } from "react";
import {
  DEFAULT_UNIT_SYSTEM,
  normalizeUnitSystem,
  type UnitSystem,
} from "./activityFormatting";
import { useUserPreferences } from "./queries";
import { useAuth } from "./auth";

type UnitPreferencesContextValue = {
  unitSystem: UnitSystem;
  isLoading: boolean;
};

const UnitPreferencesContext = createContext<UnitPreferencesContextValue>({
  unitSystem: DEFAULT_UNIT_SYSTEM,
  isLoading: false,
});

export function UnitPreferencesProvider({ children }: { children: ReactNode }) {
  const { user } = useAuth();
  const preferencesQuery = useUserPreferences({ enabled: !!user });

  return (
    <UnitPreferencesContext.Provider
      value={{
        unitSystem: normalizeUnitSystem(preferencesQuery.data?.unit_system),
        isLoading: !!user && preferencesQuery.isLoading,
      }}
    >
      {children}
    </UnitPreferencesContext.Provider>
  );
}

export function useUnitPreferences() {
  return useContext(UnitPreferencesContext);
}
