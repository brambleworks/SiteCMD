import { clearPersistedShellPage } from "@/lib/app-shell-state";
import { reloadAppWindow } from "@/lib/app-reload";
import { clearStoredProjectSelection } from "@/lib/project-selection-state";

type StartupStage = "booting" | "mounted" | "failed";

const STARTUP_STAGE_ATTR = "data-sitecmd-startup";

export function markStartupStage(stage: StartupStage) {
  if (typeof document === "undefined") return;
  document.documentElement.setAttribute(STARTUP_STAGE_ATTR, stage);
}

export function readStartupStage(): StartupStage | null {
  if (typeof document === "undefined") return null;
  const stage = document.documentElement.getAttribute(STARTUP_STAGE_ATTR);
  if (stage === "booting" || stage === "mounted" || stage === "failed") {
    return stage;
  }
  return null;
}

function clearRoot(root: HTMLElement) {
  while (root.firstChild) {
    root.removeChild(root.firstChild);
  }
}

function buildButton(label: string, onClick: () => void) {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = label;
  button.className = "sitecmd-startup-fallback__button";
  button.addEventListener("click", onClick);
  return button;
}

export function resetPersistedWorkspaceState() {
  clearPersistedShellPage();
  clearStoredProjectSelection();
}

export function supportsRequiredWebviewFeatures(): boolean {
  return typeof CSS !== "undefined" && CSS.supports("color", "color-mix(in oklab, black, white)");
}

// Preloaded boot.css classes avoid inline styles and keep the CSP strict.
export function renderStartupFallback(options: {
  title: string;
  description: string;
  details?: string | null;
  showResetAction?: boolean;
}) {
  if (typeof document === "undefined") return;
  const root = document.getElementById("root");
  if (!root) return;

  markStartupStage("failed");
  root.setAttribute("data-sitecmd-startup-fallback", "true");
  clearRoot(root);

  const container = document.createElement("div");
  container.className = "sitecmd-startup-fallback";

  const panel = document.createElement("div");
  panel.className = "sitecmd-startup-fallback__panel";

  const title = document.createElement("h1");
  title.className = "sitecmd-startup-fallback__title";
  title.textContent = options.title;

  const description = document.createElement("p");
  description.className = "sitecmd-startup-fallback__description";
  description.textContent = options.description;

  panel.appendChild(title);
  panel.appendChild(description);

  if (options.details) {
    const details = document.createElement("pre");
    details.className = "sitecmd-startup-fallback__details";
    details.textContent = options.details;
    panel.appendChild(details);
  }

  const actionRow = document.createElement("div");
  actionRow.className = "sitecmd-startup-fallback__actions";
  actionRow.appendChild(buildButton("Reload App", () => reloadAppWindow()));
  if (options.showResetAction !== false) {
    actionRow.appendChild(
      buildButton("Reset Saved State", () => {
        resetPersistedWorkspaceState();
        reloadAppWindow();
      }),
    );
  }
  panel.appendChild(actionRow);

  container.appendChild(panel);
  root.appendChild(container);
}

export function startStartupWatchdog(options?: { timeoutMs?: number; onTimeout?: () => void }) {
  const timeoutMs = options?.timeoutMs ?? 8000;
  const timer = window.setTimeout(() => {
    if (readStartupStage() === "mounted") return;
    options?.onTimeout?.();
  }, timeoutMs);

  return () => window.clearTimeout(timer);
}
