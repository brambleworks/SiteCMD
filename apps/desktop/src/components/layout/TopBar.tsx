import {
  useState,
  useRef,
  useEffect,
  useId,
  type KeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type RefObject,
} from "react";
import { ChevronDown, Loader2, Play, Plus, Settings2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { getHostname } from "@/lib/utils";
import { SearchTrigger } from "./SearchTrigger";
import type { ProjectRecord, EnvironmentRecord } from "@/hooks/useProject";

function focusMenuItem(itemRefs: RefObject<Array<HTMLButtonElement | null>>, index: number) {
  requestAnimationFrame(() => {
    itemRefs.current[index]?.focus();
  });
}

function moveFocusIndex(length: number, currentIndex: number, delta: number) {
  if (length <= 0) return -1;
  if (currentIndex < 0) {
    return delta > 0 ? 0 : length - 1;
  }
  return (currentIndex + delta + length) % length;
}

function isWindowDragTarget(target: EventTarget | null) {
  if (!(target instanceof Element)) return false;
  return !target.closest(
    [
      "button",
      "a",
      "input",
      "select",
      "textarea",
      "[role='button']",
      "[role='menu']",
      "[role='menuitem']",
      "[role='menuitemradio']",
      "[data-no-window-drag]",
    ].join(","),
  );
}

function handleWindowDragMouseDown(event: ReactMouseEvent<HTMLElement>) {
  if (event.button !== 0 || !isWindowDragTarget(event.target)) return;
  void import("@tauri-apps/api/window")
    .then(({ getCurrentWindow }) => getCurrentWindow().startDragging())
    .catch(() => {});
}

interface TopBarProps {
  projects: ProjectRecord[];
  activeProject: ProjectRecord | null;
  activeEnv: EnvironmentRecord | null;
  onSelectProject: (project: ProjectRecord) => void;
  onOpenProjectSettings?: (project: ProjectRecord) => void;
  onSelectEnv: (env: EnvironmentRecord) => void;
  onAddProject: () => void;
  onOpenSearch?: () => void;
  onRunScan?: () => void;
  onOpenScanConfig?: () => void;
  scanning?: boolean;
}

export function TopBar({
  projects,
  activeProject,
  activeEnv,
  onSelectProject,
  onOpenProjectSettings,
  onSelectEnv,
  onAddProject,
  onOpenSearch,
  onRunScan,
  onOpenScanConfig,
  scanning,
}: TopBarProps) {
  return (
    <header className="app-topbar" onMouseDown={handleWindowDragMouseDown}>
      <div
        aria-hidden="true"
        className="app-topbar-window-slot drag-region"
        data-tauri-drag-region=""
      />

      <div className="topbar-project-slot">
        <ProjectDropdown
          projects={projects}
          active={activeProject}
          onSelect={onSelectProject}
          onOpenProjectSettings={onOpenProjectSettings}
          onAdd={onAddProject}
        />
      </div>

      {activeProject && activeProject.environments.length > 0 && (
        <EnvDropdown
          environments={activeProject.environments}
          active={activeEnv}
          onSelect={onSelectEnv}
        />
      )}

      <div className="app-topbar-search-slot">
        <TopBarDragGap />
        {onOpenSearch && <SearchTrigger onClick={onOpenSearch} />}
        <TopBarDragGap />
      </div>

      {onRunScan ? (
        <div className="topbar-scan-actions">
          <Button
            type="button"
            size="sm"
            className="scan-run-button"
            onClick={onRunScan}
            disabled={scanning}>
            {scanning ? (
              <Loader2 className="spinner-sm" />
            ) : (
              <span className="scan-play-icon-slot" aria-hidden="true">
                <Play className="scan-play-icon" fill="currentColor" strokeWidth={0} />
              </span>
            )}
            {scanning ? "Scanning..." : "Run Scan"}
          </Button>
          {onOpenScanConfig ? (
            <Button
              unstyled
              type="button"
              onClick={onOpenScanConfig}
              disabled={scanning}
              aria-label="Configure scan"
              title="Configure scan"
              className="icon-btn-sm disabled-dim">
              <Settings2 className="icon-sm" aria-hidden="true" />
            </Button>
          ) : null}
        </div>
      ) : null}
    </header>
  );
}

function TopBarDragGap() {
  return (
    <div aria-hidden="true" className="app-topbar-drag-gap drag-region" data-tauri-drag-region="" />
  );
}

function ProjectDropdown({
  projects,
  active,
  onSelect,
  onOpenProjectSettings,
  onAdd,
}: {
  projects: ProjectRecord[];
  active: ProjectRecord | null;
  onSelect: (p: ProjectRecord) => void;
  onOpenProjectSettings?: (project: ProjectRecord) => void;
  onAdd: () => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const menuId = useId();
  const activeIndex = projects.findIndex((project) => project.id === active?.id);

  const closeMenu = (returnFocus = true) => {
    setOpen(false);
    if (returnFocus) {
      requestAnimationFrame(() => {
        triggerRef.current?.focus();
      });
    }
  };

  const openAndFocus = (index: number) => {
    setOpen(true);
    focusMenuItem(itemRefs, index);
  };

  const handleTriggerKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      openAndFocus(activeIndex >= 0 ? activeIndex : 0);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      openAndFocus(activeIndex >= 0 ? activeIndex : projects.length);
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (open) {
        closeMenu(false);
      } else {
        openAndFocus(activeIndex >= 0 ? activeIndex : 0);
      }
    } else if (event.key === "Escape") {
      event.preventDefault();
      closeMenu(false);
    }
  };

  const handleMenuKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const itemCount = projects.length + 1;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      focusMenuItem(itemRefs, moveFocusIndex(itemCount, index, 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      focusMenuItem(itemRefs, moveFocusIndex(itemCount, index, -1));
    } else if (event.key === "Home") {
      event.preventDefault();
      focusMenuItem(itemRefs, 0);
    } else if (event.key === "End") {
      event.preventDefault();
      focusMenuItem(itemRefs, itemCount - 1);
    } else if (event.key === "Escape") {
      event.preventDefault();
      closeMenu();
    } else if (event.key === "Tab") {
      setOpen(false);
    }
  };

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  return (
    <div className="project-dropdown-root" ref={ref}>
      <Button
        unstyled
        ref={triggerRef}
        type="button"
        onClick={() => setOpen((current) => !current)}
        onKeyDown={handleTriggerKeyDown}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        className="project-select-trigger">
        <span className="text-truncate">{active?.name || "Select project"}</span>
        <ChevronDown className="icon-sm text-muted-foreground" />
      </Button>

      {open && (
        <div
          id={menuId}
          role="menu"
          aria-label="Project selector"
          className="dropdown-menu topbar-project-menu">
          {projects.map((p, index) => (
            <div
              key={p.id}
              className={`group topbar-menu-item-row ${p.id === active?.id ? "bg-accent" : ""}`}>
              <Button
                unstyled
                ref={(node) => {
                  itemRefs.current[index] = node;
                }}
                type="button"
                role="menuitemradio"
                aria-checked={p.id === active?.id}
                onClick={() => {
                  onSelect(p);
                  setOpen(false);
                }}
                onKeyDown={(event) => handleMenuKeyDown(event, index)}
                className={`dropdown-item flex-fill ${p.id === active?.id ? "dropdown-item--current" : ""}`}>
                <span className="text-truncate flex-fill">{p.name}</span>
              </Button>
              {onOpenProjectSettings && (
                <Button
                  unstyled
                  type="button"
                  aria-label={`Edit ${p.name} project settings`}
                  title={`Edit ${p.name} project settings`}
                  onClick={(event) => {
                    event.stopPropagation();
                    onOpenProjectSettings(p);
                    setOpen(false);
                  }}
                  className="project-settings-action">
                  <Settings2 className="icon-sm" />
                </Button>
              )}
            </div>
          ))}
          <div className="dropdown-divider" />
          <Button
            unstyled
            ref={(node) => {
              itemRefs.current[projects.length] = node;
            }}
            type="button"
            role="menuitem"
            onClick={() => {
              onAdd();
              setOpen(false);
            }}
            onKeyDown={(event) => handleMenuKeyDown(event, projects.length)}
            className="dropdown-item dropdown-item--add">
            <Plus className="icon-sm" /> Add Project
          </Button>
        </div>
      )}
    </div>
  );
}

function EnvDropdown({
  environments,
  active,
  onSelect,
}: {
  environments: EnvironmentRecord[];
  active: EnvironmentRecord | null;
  onSelect: (e: EnvironmentRecord) => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const menuId = useId();
  const activeIndex = environments.findIndex((environment) => environment.id === active?.id);

  const closeMenu = (returnFocus = true) => {
    setOpen(false);
    if (returnFocus) {
      requestAnimationFrame(() => {
        triggerRef.current?.focus();
      });
    }
  };

  const openAndFocus = (index: number) => {
    setOpen(true);
    focusMenuItem(itemRefs, index);
  };

  const handleTriggerKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (environments.length === 0) return;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      openAndFocus(activeIndex >= 0 ? activeIndex : 0);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      openAndFocus(activeIndex >= 0 ? activeIndex : environments.length - 1);
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (open) {
        closeMenu(false);
      } else {
        openAndFocus(activeIndex >= 0 ? activeIndex : 0);
      }
    } else if (event.key === "Escape") {
      event.preventDefault();
      closeMenu(false);
    }
  };

  const handleMenuKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      focusMenuItem(itemRefs, moveFocusIndex(environments.length, index, 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      focusMenuItem(itemRefs, moveFocusIndex(environments.length, index, -1));
    } else if (event.key === "Home") {
      event.preventDefault();
      focusMenuItem(itemRefs, 0);
    } else if (event.key === "End") {
      event.preventDefault();
      focusMenuItem(itemRefs, environments.length - 1);
    } else if (event.key === "Escape") {
      event.preventDefault();
      closeMenu();
    } else if (event.key === "Tab") {
      setOpen(false);
    }
  };

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  if (environments.length <= 1 && active) {
    return (
      <span className="text-body-muted">
        {getHostname(active.url)}
        <span className="topbar-env-suffix">· {active.environment}</span>
      </span>
    );
  }

  return (
    <div className="env-dropdown-root" ref={ref}>
      <Button
        unstyled
        ref={triggerRef}
        type="button"
        onClick={() => setOpen((current) => !current)}
        onKeyDown={handleTriggerKeyDown}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        className="topbar-menu-trigger">
        <span className="text-muted-foreground">
          {getHostname(active?.url || "")}
          <span className="topbar-env-suffix">· {active?.environment || "env"}</span>
        </span>
        <ChevronDown className="icon-sm text-muted-foreground" />
      </Button>

      {open && (
        <div
          id={menuId}
          role="menu"
          aria-label="Environment selector"
          className="dropdown-menu topbar-env-menu">
          {environments.map((env, index) => (
            <Button
              unstyled
              ref={(node) => {
                itemRefs.current[index] = node;
              }}
              type="button"
              role="menuitemradio"
              aria-checked={env.id === active?.id}
              key={env.id}
              onClick={() => {
                onSelect(env);
                setOpen(false);
              }}
              onKeyDown={(event) => handleMenuKeyDown(event, index)}
              className={`dropdown-item ${env.id === active?.id ? "bg-accent dropdown-item--current" : ""}`}>
              <span className="text-capitalize">{env.environment}</span>
              <span className="subtitle-xs text-truncate topbar-env-host">
                {getHostname(env.url)}
              </span>
            </Button>
          ))}
        </div>
      )}
    </div>
  );
}
