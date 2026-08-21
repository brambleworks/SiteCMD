export type IssuesTab = "issues" | "by-page" | "history";

const ISSUES_VIEW_KEY = "issues.view";

export function loadIssuesView(): IssuesTab {
  if (typeof window === "undefined") return "issues";
  const value = window.localStorage.getItem(ISSUES_VIEW_KEY);
  return value === "by-page" || value === "history" ? value : "issues";
}

export function saveIssuesView(tab: IssuesTab) {
  window.localStorage.setItem(ISSUES_VIEW_KEY, tab);
}
