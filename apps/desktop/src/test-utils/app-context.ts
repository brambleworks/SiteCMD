import { createElement, type ReactElement } from "react";
import { vi } from "vitest";

import { HistoryProvider } from "@/app/HistoryProvider";
import { NavigationProvider } from "@/app/NavigationProvider";
import type { HistoryContextValue } from "@/app/history-context";
import type { NavigationContextValue } from "@/app/navigation-context";

function buildNavigationValue(
  overrides: Partial<NavigationContextValue> = {},
): NavigationContextValue {
  return {
    page: "issues",
    settingsTab: undefined,
    issuesTarget: null,
    issuesTabResetKey: 0,
    searchFocus: null,
    searchItemId: null,
    searchLane: null,
    updatesTarget: null,
    alertsTarget: null,
    focusIntegration: null,
    arrivalPrompt: null,
    ...overrides,
  };
}

function buildHistoryValue(overrides: Partial<HistoryContextValue> = {}): HistoryContextValue {
  return {
    history: [],
    executions: [],
    codeHistory: [],
    sessions: [],
    loading: false,
    historyError: null,
    loadHistory: vi.fn(),
    ...overrides,
  };
}

export function withAppContext(
  ui: ReactElement,
  overrides: {
    navigation?: Partial<NavigationContextValue>;
    history?: Partial<HistoryContextValue>;
  } = {},
): ReactElement {
  return createElement(NavigationProvider, {
    value: buildNavigationValue(overrides.navigation),
    children: createElement(HistoryProvider, {
      value: buildHistoryValue(overrides.history),
      children: ui,
    }),
  });
}
