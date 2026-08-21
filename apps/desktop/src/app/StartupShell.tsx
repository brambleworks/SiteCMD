import { Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { SurfaceState } from "@/components/ui/surface-state";
import { PageSkeleton } from "@/components/ui/page-skeleton";
import { TopBar } from "@/components/layout/TopBar";
import { useTheme } from "@/hooks/useTheme";
import type { ProjectRecord } from "@/hooks/useProject";

type StartupState = "loading" | "error" | "welcome";

interface StartupShellProps {
  state: StartupState;
  projects: ProjectRecord[];
  onAddProject: () => void;
  onOpenSearch: () => void;
  onRetryProjectsLoad: () => void;
}

export function StartupShell({
  state,
  projects,
  onAddProject,
  onOpenSearch,
  onRetryProjectsLoad,
}: StartupShellProps) {
  return (
    <div className="app-shell bg-background">
      <TopBar
        projects={projects}
        activeProject={null}
        activeEnv={null}
        onSelectProject={() => {}}
        onSelectEnv={() => {}}
        onAddProject={() => onAddProject()}
        onOpenSearch={onOpenSearch}
      />
      {state === "loading" && <StartupLoading />}
      {state === "error" && <StartupError onRetryProjectsLoad={onRetryProjectsLoad} />}
      {state === "welcome" && <StartupWelcome onAddProject={onAddProject} />}
    </div>
  );
}

function StartupLoading() {
  return (
    <div className="startup-scroll">
      <div className="startup-container">
        <PageSkeleton label="Loading your workspace" layout="dashboard" />
      </div>
    </div>
  );
}

function StartupError({ onRetryProjectsLoad }: { onRetryProjectsLoad: () => void }) {
  return (
    <div className="startup-scroll">
      <div className="startup-container startup-container--narrow">
        <SurfaceState
          kind="error"
          title="Projects could not load"
          description="SiteCMD could not reconnect to your local project data on launch. Try again to reload your workspace instead of starting from an empty screen."
          primaryAction={{
            label: "Retry",
            onClick: onRetryProjectsLoad,
          }}
        />
      </div>
    </div>
  );
}

function StartupWelcome({ onAddProject }: { onAddProject: StartupShellProps["onAddProject"] }) {
  const { resolved } = useTheme();
  return (
    <div className="startup-welcome-scroll">
      <div className="callout-empty-center startup-welcome-body">
        <h1 className="startup-welcome-title">
          <p className="startup-welcome-lead">Welcome to</p>
          <div>
            <img
              src={resolved === "dark" ? "/images/logo.png" : "/images/logo-dark.png"}
              alt="SiteCMD"
              className="welcome-logo startup-welcome-logo"
              width={285}
              height={76}
            />
          </div>
        </h1>
        <p className="startup-welcome-desc text-relaxed">
          Add a project to start building your local website command center.
        </p>
        <div className="row-wrap startup-welcome-actions">
          <Button onClick={() => onAddProject()} size="lg">
            <Plus className="icon-lg" /> Add Project
          </Button>
        </div>
      </div>
    </div>
  );
}
