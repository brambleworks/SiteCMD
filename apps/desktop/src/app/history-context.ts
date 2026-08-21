import { createContext, useContext } from "react";

import type { useHistory } from "@/hooks/useHistory";

export type HistoryContextValue = ReturnType<typeof useHistory>;

export const HistoryContext = createContext<HistoryContextValue | null>(null);

/** Read the shell's scan-history hook. Throws if used outside the provider. */
export function useHistoryContext(): HistoryContextValue {
  const value = useContext(HistoryContext);
  if (!value) {
    throw new Error(
      "useHistoryContext() must be used inside <HistoryProvider>; AppContent provides it.",
    );
  }
  return value;
}
