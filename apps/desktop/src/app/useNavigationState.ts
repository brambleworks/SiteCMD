import { useReducer, type Dispatch } from "react";

import type { NavPage } from "@/components/layout/NavSidebar";
import type { DesktopPromptEntry } from "@/lib/desktop-prompts";

type ArrivalPromptPage = "search-console" | "updates";

interface ArrivalPromptState {
  page: ArrivalPromptPage;
  entry: DesktopPromptEntry;
}

type SearchLaneState = "pending-verification" | null;
type FocusTargetState = {
  focus?: string | null;
  itemId?: string | null;
} | null;
type UpdatesTargetState = {
  lane?: SearchLaneState;
  itemId?: string | null;
} | null;
/** What a `sitecmd://connected/...` link asked the alert timeline to show:
 *  one alert by its opaque id, or the not-found notice under its reason. */
type AlertsTargetInput = {
  alertId?: string | null;
  reason?: string | null;
} | null;
type AlertsTargetState = {
  alertId?: string | null;
  reason?: string | null;
  /** Distinguishes a new outside-app arrival from a refetch of the same
   *  alert while the Alerts page remains mounted. */
  arrival: number;
} | null;

export interface NavigationState {
  page: NavPage;
  issuesTabResetKey: number;
  settingsTab: string | undefined;
  showCommandPalette: boolean;
  issuesTarget: FocusTargetState;
  searchFocus: string | null;
  searchItemId: string | null;
  searchLane: SearchLaneState;
  updatesTarget: UpdatesTargetState;
  alertsTarget: AlertsTargetState;
  focusIntegration: string | null;
  arrivalPrompt: ArrivalPromptState | null;
}

type NavigationAction =
  | { type: "SET_PAGE"; page: NavPage }
  | { type: "NAVIGATE_GENERIC"; page: NavPage }
  | { type: "NAVIGATE_ISSUES"; target?: FocusTargetState }
  | { type: "RESET_ISSUES_TAB" }
  | { type: "OPEN_SETTINGS"; tab?: string }
  | { type: "OPEN_INTEGRATIONS"; focus: string | null }
  | {
      type: "OPEN_SEARCH_CONSOLE";
      focus: string | null;
      itemId?: string | null;
      lane?: SearchLaneState;
      prompt?: ArrivalPromptState | null;
    }
  | {
      type: "OPEN_UPDATES";
      lane: SearchLaneState;
      itemId: string | null;
      prompt?: ArrivalPromptState | null;
    }
  | { type: "OPEN_ALERTS"; target: AlertsTargetInput }
  | { type: "CLEAR_PAGE_TARGETS" }
  | { type: "SET_FOCUS_INTEGRATION"; value: string | null }
  | { type: "SET_ARRIVAL_PROMPT"; prompt: ArrivalPromptState | null }
  | { type: "TOGGLE_COMMAND_PALETTE" }
  | { type: "SET_COMMAND_PALETTE_OPEN"; open: boolean }
  | { type: "URL_CHANGED" };

export type NavigationDispatch = Dispatch<NavigationAction>;

const EMPTY_TARGETS: Pick<
  NavigationState,
  | "settingsTab"
  | "issuesTarget"
  | "searchFocus"
  | "searchItemId"
  | "searchLane"
  | "updatesTarget"
  | "alertsTarget"
  | "focusIntegration"
> = {
  settingsTab: undefined,
  issuesTarget: null,
  searchFocus: null,
  searchItemId: null,
  searchLane: null,
  updatesTarget: null,
  alertsTarget: null,
  focusIntegration: null,
};

function createInitialNavigationState(initialPage: NavPage = "dashboard"): NavigationState {
  return {
    page: initialPage,
    issuesTabResetKey: 0,
    showCommandPalette: false,
    arrivalPrompt: null,
    ...EMPTY_TARGETS,
  };
}

function navigationReducer(state: NavigationState, action: NavigationAction): NavigationState {
  switch (action.type) {
    case "SET_PAGE":
      return { ...state, page: action.page };

    case "NAVIGATE_GENERIC":
      return {
        ...state,
        ...EMPTY_TARGETS,
        arrivalPrompt: null,
        page: action.page,
      };

    case "NAVIGATE_ISSUES":
      return {
        ...state,
        ...EMPTY_TARGETS,
        arrivalPrompt: null,
        page: "issues",
        issuesTarget: action.target ?? null,
      };

    case "RESET_ISSUES_TAB":
      return { ...state, issuesTabResetKey: state.issuesTabResetKey + 1 };

    case "OPEN_SETTINGS":
      return {
        ...state,
        ...EMPTY_TARGETS,
        arrivalPrompt: null,
        page: "settings",
        settingsTab: action.tab,
      };

    case "OPEN_INTEGRATIONS":
      return {
        ...state,
        ...EMPTY_TARGETS,
        arrivalPrompt: null,
        page: "integrations",
        focusIntegration: action.focus,
      };

    case "OPEN_SEARCH_CONSOLE":
      return {
        ...state,
        ...EMPTY_TARGETS,
        arrivalPrompt: action.prompt ?? null,
        page: "search-console",
        searchFocus: action.focus,
        searchItemId: action.itemId ?? null,
        searchLane: action.lane ?? null,
      };

    case "OPEN_UPDATES":
      return {
        ...state,
        ...EMPTY_TARGETS,
        arrivalPrompt: action.prompt ?? null,
        page: "updates",
        updatesTarget: { lane: action.lane, itemId: action.itemId },
      };

    case "OPEN_ALERTS":
      return {
        ...state,
        ...EMPTY_TARGETS,
        arrivalPrompt: null,
        page: "alerts",
        alertsTarget: action.target
          ? {
              ...action.target,
              arrival: (state.alertsTarget?.arrival ?? 0) + 1,
            }
          : null,
      };

    case "CLEAR_PAGE_TARGETS":
      return { ...state, ...EMPTY_TARGETS };

    case "SET_FOCUS_INTEGRATION":
      return { ...state, focusIntegration: action.value };

    case "SET_ARRIVAL_PROMPT":
      return { ...state, arrivalPrompt: action.prompt };

    case "TOGGLE_COMMAND_PALETTE":
      return { ...state, showCommandPalette: !state.showCommandPalette };

    case "SET_COMMAND_PALETTE_OPEN":
      return { ...state, showCommandPalette: action.open };

    case "URL_CHANGED":
      return { ...state, ...EMPTY_TARGETS, arrivalPrompt: null };
  }
}

interface UseNavigationStateOptions {
  initialPage?: NavPage;
}

interface UseNavigationStateResult {
  state: NavigationState;
  dispatch: NavigationDispatch;
}

export function useNavigationState(
  options: UseNavigationStateOptions = {},
): UseNavigationStateResult {
  const [state, dispatch] = useReducer(
    navigationReducer,
    options.initialPage ?? "dashboard",
    createInitialNavigationState,
  );

  return { state, dispatch };
}
