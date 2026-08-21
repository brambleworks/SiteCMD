import { renderHook, act } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { useNavigationState } from "./useNavigationState";

describe("useNavigationState", () => {
  it("starts on dashboard", () => {
    const { result } = renderHook(() => useNavigationState());
    expect(result.current.state.page).toBe("dashboard");
    expect(result.current.state.issuesTarget).toBeNull();
    expect(result.current.state.searchFocus).toBeNull();
    expect(result.current.state.searchItemId).toBeNull();
    expect(result.current.state.searchLane).toBeNull();
    expect(result.current.state.updatesTarget).toBeNull();
    expect(result.current.state.alertsTarget).toBeNull();
    expect(result.current.state.focusIntegration).toBeNull();
    expect(result.current.state.arrivalPrompt).toBeNull();
    expect(result.current.state.settingsTab).toBeUndefined();
    expect(result.current.state.showCommandPalette).toBe(false);
    expect(result.current.state.issuesTabResetKey).toBe(0);
  });

  it("respects an initial page override", () => {
    const { result } = renderHook(() => useNavigationState({ initialPage: "issues" }));
    expect(result.current.state.page).toBe("issues");
  });

  it("navigates to issues with a target", () => {
    const { result } = renderHook(() => useNavigationState());
    act(() => {
      result.current.dispatch({ type: "NAVIGATE_ISSUES", target: { focus: "sec" } });
    });
    expect(result.current.state.page).toBe("issues");
    expect(result.current.state.issuesTarget).toEqual({ focus: "sec" });
  });

  it("clears all targets on URL change but preserves page and palette", () => {
    const { result } = renderHook(() => useNavigationState());
    act(() => {
      result.current.dispatch({
        type: "OPEN_SEARCH_CONSOLE",
        focus: "queries",
        itemId: "q-1",
        lane: "pending-verification",
      });
      result.current.dispatch({ type: "NAVIGATE_ISSUES", target: { focus: "sec" } });
      result.current.dispatch({ type: "TOGGLE_COMMAND_PALETTE" });
      result.current.dispatch({ type: "URL_CHANGED" });
    });
    expect(result.current.state.issuesTarget).toBeNull();
    expect(result.current.state.searchFocus).toBeNull();
    expect(result.current.state.searchItemId).toBeNull();
    expect(result.current.state.searchLane).toBeNull();
    expect(result.current.state.updatesTarget).toBeNull();
    expect(result.current.state.alertsTarget).toBeNull();
    expect(result.current.state.focusIntegration).toBeNull();
    expect(result.current.state.arrivalPrompt).toBeNull();
    // page is preserved (was last set by NAVIGATE_ISSUES)
    expect(result.current.state.page).toBe("issues");
    // palette is preserved
    expect(result.current.state.showCommandPalette).toBe(true);
  });

  it("opens settings to specific tab and clears page targets", () => {
    const { result } = renderHook(() => useNavigationState());
    act(() => {
      result.current.dispatch({ type: "NAVIGATE_ISSUES", target: { focus: "security" } });
    });
    expect(result.current.state.issuesTarget).not.toBeNull();
    act(() => {
      result.current.dispatch({ type: "OPEN_SETTINGS", tab: "integrations" });
    });
    expect(result.current.state.page).toBe("settings");
    expect(result.current.state.settingsTab).toBe("integrations");
    expect(result.current.state.issuesTarget).toBeNull();
  });

  it("toggles command palette", () => {
    const { result } = renderHook(() => useNavigationState());
    expect(result.current.state.showCommandPalette).toBe(false);
    act(() => result.current.dispatch({ type: "TOGGLE_COMMAND_PALETTE" }));
    expect(result.current.state.showCommandPalette).toBe(true);
    act(() => result.current.dispatch({ type: "TOGGLE_COMMAND_PALETTE" }));
    expect(result.current.state.showCommandPalette).toBe(false);
  });

  it("sets command palette open state explicitly", () => {
    const { result } = renderHook(() => useNavigationState());
    act(() => result.current.dispatch({ type: "SET_COMMAND_PALETTE_OPEN", open: true }));
    expect(result.current.state.showCommandPalette).toBe(true);
    act(() => result.current.dispatch({ type: "SET_COMMAND_PALETTE_OPEN", open: false }));
    expect(result.current.state.showCommandPalette).toBe(false);
  });

  it("RESET_ISSUES_TAB increments tabResetKey", () => {
    const { result } = renderHook(() => useNavigationState());
    const before = result.current.state.issuesTabResetKey;
    act(() => result.current.dispatch({ type: "RESET_ISSUES_TAB" }));
    expect(result.current.state.issuesTabResetKey).toBe(before + 1);
    act(() => result.current.dispatch({ type: "RESET_ISSUES_TAB" }));
    expect(result.current.state.issuesTabResetKey).toBe(before + 2);
  });

  it("OPEN_INTEGRATIONS sets focusIntegration and switches page", () => {
    const { result } = renderHook(() => useNavigationState());
    act(() => {
      result.current.dispatch({ type: "OPEN_INTEGRATIONS", focus: "github" });
    });
    expect(result.current.state.page).toBe("integrations");
    expect(result.current.state.focusIntegration).toBe("github");
  });

  it("OPEN_SETTINGS clears a stale arrival prompt", () => {
    const { result } = renderHook(() => useNavigationState());
    act(() => {
      result.current.dispatch({
        type: "SET_ARRIVAL_PROMPT",
        prompt: { page: "updates", entry: { id: "p", page: "updates", message: "" } as never },
      });
    });
    expect(result.current.state.arrivalPrompt).not.toBeNull();
    act(() => {
      result.current.dispatch({ type: "OPEN_SETTINGS", tab: "data" });
    });
    expect(result.current.state.page).toBe("settings");
    expect(result.current.state.arrivalPrompt).toBeNull();
  });

  it("OPEN_INTEGRATIONS clears a stale arrival prompt", () => {
    const { result } = renderHook(() => useNavigationState());
    act(() => {
      result.current.dispatch({
        type: "SET_ARRIVAL_PROMPT",
        prompt: {
          page: "search-console",
          entry: { id: "p", page: "search-console", message: "" } as never,
        },
      });
    });
    expect(result.current.state.arrivalPrompt).not.toBeNull();
    act(() => {
      result.current.dispatch({ type: "OPEN_INTEGRATIONS", focus: "github" });
    });
    expect(result.current.state.page).toBe("integrations");
    expect(result.current.state.arrivalPrompt).toBeNull();
  });

  it("OPEN_SEARCH_CONSOLE sets focus, itemId, and lane", () => {
    const { result } = renderHook(() => useNavigationState());
    act(() => {
      result.current.dispatch({
        type: "OPEN_SEARCH_CONSOLE",
        focus: "queries",
        itemId: "abc",
        lane: "pending-verification",
      });
    });
    expect(result.current.state.page).toBe("search-console");
    expect(result.current.state.searchFocus).toBe("queries");
    expect(result.current.state.searchItemId).toBe("abc");
    expect(result.current.state.searchLane).toBe("pending-verification");
  });

  it("OPEN_UPDATES sets the updates target", () => {
    const { result } = renderHook(() => useNavigationState());
    act(() => {
      result.current.dispatch({
        type: "OPEN_UPDATES",
        lane: null,
        itemId: "item-7",
      });
    });
    expect(result.current.state.page).toBe("updates");
    expect(result.current.state.updatesTarget).toEqual({ lane: null, itemId: "item-7" });
  });

  it("SET_ARRIVAL_PROMPT updates the prompt without touching page", () => {
    const { result } = renderHook(() => useNavigationState());
    act(() => {
      result.current.dispatch({
        type: "SET_ARRIVAL_PROMPT",
        prompt: {
          page: "updates",
          entry: { id: "p", page: "updates", message: "" } as never,
        },
      });
    });
    expect(result.current.state.arrivalPrompt?.page).toBe("updates");
    act(() => {
      result.current.dispatch({ type: "SET_ARRIVAL_PROMPT", prompt: null });
    });
    expect(result.current.state.arrivalPrompt).toBeNull();
  });

  it("CLEAR_PAGE_TARGETS resets every target but keeps page", () => {
    const { result } = renderHook(() => useNavigationState());
    act(() => {
      result.current.dispatch({ type: "NAVIGATE_ISSUES", target: { focus: "security" } });
    });
    expect(result.current.state.page).toBe("issues");
    act(() => {
      result.current.dispatch({ type: "CLEAR_PAGE_TARGETS" });
    });
    expect(result.current.state.page).toBe("issues");
    expect(result.current.state.issuesTarget).toBeNull();
    expect(result.current.state.settingsTab).toBeUndefined();
  });

  it("NAVIGATE_GENERIC clears targets and switches page", () => {
    const { result } = renderHook(() => useNavigationState());
    act(() => {
      result.current.dispatch({ type: "NAVIGATE_ISSUES", target: { focus: "security" } });
    });
    act(() => {
      result.current.dispatch({ type: "NAVIGATE_GENERIC", page: "alerts" });
    });
    expect(result.current.state.page).toBe("alerts");
    expect(result.current.state.issuesTarget).toBeNull();
  });

  it("NAVIGATE_ISSUES with no target leaves issuesTarget null", () => {
    const { result } = renderHook(() => useNavigationState());
    act(() => {
      result.current.dispatch({ type: "NAVIGATE_ISSUES" });
    });
    expect(result.current.state.page).toBe("issues");
    expect(result.current.state.issuesTarget).toBeNull();
  });

  it("SET_FOCUS_INTEGRATION updates value without changing page", () => {
    const { result } = renderHook(() => useNavigationState());
    act(() => {
      result.current.dispatch({ type: "OPEN_SETTINGS", tab: "data" });
    });
    expect(result.current.state.page).toBe("settings");
    act(() => {
      result.current.dispatch({ type: "SET_FOCUS_INTEGRATION", value: "jira" });
    });
    expect(result.current.state.page).toBe("settings");
    expect(result.current.state.focusIntegration).toBe("jira");
    act(() => {
      result.current.dispatch({ type: "SET_FOCUS_INTEGRATION", value: null });
    });
    expect(result.current.state.focusIntegration).toBeNull();
  });

  it("SET_PAGE switches page without clearing targets", () => {
    const { result } = renderHook(() => useNavigationState());
    act(() => {
      result.current.dispatch({ type: "NAVIGATE_ISSUES", target: { focus: "security" } });
    });
    act(() => {
      result.current.dispatch({ type: "SET_PAGE", page: "settings" });
    });
    expect(result.current.state.page).toBe("settings");
    // SET_PAGE is the low-level escape hatch - targets persist
    expect(result.current.state.issuesTarget).toEqual({ focus: "security" });
  });
  it("OPEN_ALERTS carries the deep link's target and a later page clears it", () => {
    const { result } = renderHook(() => useNavigationState());
    act(() => {
      result.current.dispatch({
        type: "OPEN_ALERTS",
        target: { alertId: "alr_0123456789ab", reason: null },
      });
    });
    expect(result.current.state.page).toBe("alerts");
    expect(result.current.state.alertsTarget).toEqual({
      alertId: "alr_0123456789ab",
      reason: null,
      arrival: 1,
    });

    act(() => {
      result.current.dispatch({
        type: "OPEN_ALERTS",
        target: { alertId: "alr_0123456789ab", reason: null },
      });
    });
    expect(result.current.state.alertsTarget).toEqual({
      alertId: "alr_0123456789ab",
      reason: null,
      arrival: 2,
    });

    // Otherwise a link's notice would follow the user around the app.
    act(() => {
      result.current.dispatch({ type: "NAVIGATE_GENERIC", page: "dashboard" });
    });
    expect(result.current.state.alertsTarget).toBeNull();
  });
});
