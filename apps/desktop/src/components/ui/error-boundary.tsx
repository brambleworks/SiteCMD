import { Component, type ErrorInfo, type ReactNode } from "react";
import { logger } from "@/lib/logger";
import { recordErrorReport } from "@/lib/observability";
import { reloadAppWindow } from "@/lib/app-reload";
import { clearPersistedShellPage } from "@/lib/app-shell-state";
import { clearStoredProjectSelection } from "@/lib/project-selection-state";
import { Button } from "@/components/ui/button";

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("ErrorBoundary caught:", error, info.componentStack);
    recordErrorReport("react.error_boundary", error, {
      fatal: true,
      meta: { componentStack: info.componentStack ?? "unknown" },
    });
    logger.error(
      `React crash: ${error.message}`,
      `${error.stack ?? ""}\nComponent stack: ${info.componentStack ?? "unknown"}`,
    );
  }

  private resetBoundary() {
    this.setState({ hasError: false, error: null });
  }

  private resetPersistedWorkspaceState() {
    // Require confirmation before clearing persisted workspace selection.
    const confirmed =
      typeof window === "undefined"
        ? true
        : window.confirm(
            "This clears your last-used project and page so the app starts from a clean slate. Your scan history and integrations stay intact. Continue?",
          );
    if (!confirmed) return;
    clearPersistedShellPage();
    clearStoredProjectSelection();
    reloadAppWindow();
  }

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) return this.props.fallback;

      return (
        <div className="error-boundary-body">
          <div className="error-boundary-title text-severity-critical">Something went wrong</div>
          <p className="text-body-muted error-boundary-message">
            {this.state.error?.message || "An unexpected error occurred."}
          </p>
          <p className="text-meta error-boundary-hint">
            If this keeps happening, reload SiteCMD or reset the saved workspace state to get out of
            a bad startup loop.
          </p>
          <div className="error-boundary-actions">
            <Button unstyled onClick={() => this.resetBoundary()} className="error-action-primary">
              Try Again
            </Button>
            <Button unstyled onClick={() => reloadAppWindow()} className="secondary-outline-button">
              Reload App
            </Button>
            <Button
              unstyled
              onClick={() => this.resetPersistedWorkspaceState()}
              className="secondary-outline-button">
              Reset Saved State
            </Button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
