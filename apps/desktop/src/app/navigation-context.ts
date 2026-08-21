import { createContext, useContext } from "react";

import type { NavigationState } from "@/app/useNavigationState";

export type NavigationContextValue = Pick<
  NavigationState,
  | "page"
  | "settingsTab"
  | "issuesTarget"
  | "issuesTabResetKey"
  | "searchFocus"
  | "searchItemId"
  | "searchLane"
  | "updatesTarget"
  | "alertsTarget"
  | "focusIntegration"
  | "arrivalPrompt"
>;

export const NavigationContext = createContext<NavigationContextValue | null>(null);

/** Read the current navigation snapshot. Throws if used outside the provider. */
export function useNavigation(): NavigationContextValue {
  const value = useContext(NavigationContext);
  if (!value) {
    throw new Error(
      "useNavigation() must be used inside <NavigationProvider>; AppContent provides it.",
    );
  }
  return value;
}
