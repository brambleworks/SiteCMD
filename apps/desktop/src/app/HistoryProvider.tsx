import { type ReactNode } from "react";

import { HistoryContext, type HistoryContextValue } from "@/app/history-context";

export function HistoryProvider({
  value,
  children,
}: {
  value: HistoryContextValue;
  children: ReactNode;
}) {
  return <HistoryContext.Provider value={value}>{children}</HistoryContext.Provider>;
}
