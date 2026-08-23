import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  getScanStartLabel,
  getTimeEstimate,
  humanizePath,
  type ScanConfig,
} from "@/components/scan/scan-config-overlay-model";
import { useScanConfigOverlayState } from "@/components/scan/useScanConfigOverlayState";
import {
  Search,
  X,
  CheckSquare,
  Square,
  Loader2,
  Play,
  Clock3,
  ListChecks,
  Database,
} from "lucide-react";
import { cn } from "@/lib/utils";

export type { ScanConfig, ScanConfigPreset, ScanMode } from "./scan-config-overlay-model";

interface ScanConfigOverlayProps {
  siteUrl: string;
  siteId?: number;
  projectId?: number;
  projectPath?: string | null;
  onStart: (config: ScanConfig) => void;
  onCancel: () => void;
  initialScanType?: ScanConfig["scanType"];
  initialAxeEnabled?: boolean;
}

export function ScanConfigOverlay({
  siteUrl,
  siteId,
  projectId,
  projectPath,
  onStart,
  onCancel,
}: ScanConfigOverlayProps) {
  const {
    axeEnabled,
    canUseCodeScan,
    hasSite,
    discovering,
    filtered,
    handleDiscover,
    handleStart,
    hasPages,
    inspectLocalDatabases,
    loading,
    pages,
    scanType,
    scopeError,
    search,
    selectAll,
    selected,
    selectNone,
    setInspectLocalDatabases,
    setSearch,
    togglePage,
  } = useScanConfigOverlayState({
    canUseAccessibilityDeepScan: false,
    initialAxeEnabled: false,
    initialScanType: "full",
    onCancel,
    onStart,
    projectPath,
    projectId,
    siteId,
    siteUrl,
  });

  const selectedPageCount = selected.size || 1;
  const estimatedTime = getTimeEstimate(selectedPageCount, axeEnabled, scanType, canUseCodeScan);

  return (
    <div
      className="overlay-backdrop overlay-backdrop--config"
      role="dialog"
      aria-modal="true"
      aria-label="Scan configuration"
      onClick={onCancel}>
      <div className="modal-panel" onClick={(e) => e.stopPropagation()}>
        <div className="scan-config-header">
          <div className="min-w-0">
            <div className="row">
              <span className="icon-badge icon-badge--sm icon-badge--brand">
                <Search className="icon-md" aria-hidden="true" />
              </span>
              <div className="min-w-0">
                <h2 className="scan-config-title">Run Scan</h2>
                <p className="text-body-muted text-truncate scan-config-sub">{siteUrl}</p>
              </div>
            </div>
          </div>
          <Button
            unstyled
            onClick={onCancel}
            className="icon-btn"
            aria-label="Close scan configuration">
            <X className="icon-md" />
          </Button>
        </div>

        <div className="scan-config-body">
          {!canUseCodeScan ? (
            <p className="text-meta text-relaxed">
              Link a local project folder to add Code Scan to this run.
            </p>
          ) : null}

          {canUseCodeScan ? (
            <section className="scan-config-section">
              <div className="settings-control-row">
                <div className="row-start min-w-0">
                  <span className="icon-badge icon-badge--sm icon-badge--muted">
                    <Database className="icon-md" aria-hidden="true" />
                  </span>
                  <div className="min-w-0">
                    <p className="text-body-muted text-strong">Inspect local database schemas</p>
                    <p className="text-meta text-relaxed">
                      Optional for this run. Reads local dotenv values only to find a database, then
                      reads schema and migration metadata, never application table rows. SQLite
                      files must be inside the linked project, and PostgreSQL must be on this
                      computer (localhost or a local socket).
                    </p>
                  </div>
                </div>
                <Button
                  type="button"
                  unstyled
                  className="toggle-switch"
                  data-on={inspectLocalDatabases ? "true" : "false"}
                  role="switch"
                  aria-checked={inspectLocalDatabases}
                  aria-label="Inspect local database schemas"
                  onClick={() => setInspectLocalDatabases(!inspectLocalDatabases)}>
                  <span className="toggle-switch-thumb" />
                </Button>
              </div>
            </section>
          ) : null}

          {!hasSite ? (
            <p className="text-meta text-relaxed">
              This project has no site URL, so this run covers the linked folder only. Add an
              environment URL in Settings to scan the live site too.
            </p>
          ) : loading ? (
            <section className="scan-config-section">
              <div className="row-between">
                <div>
                  <div className="scan-config-skeleton-heading" />
                  <div className="scan-config-skeleton-copy" />
                </div>
                <div className="scan-config-skeleton-action" />
              </div>
              <div className="scan-config-list">
                {[1, 2, 3, 4].map((i) => (
                  <div key={i} className="scan-config-sk-row">
                    <span className="scan-config-skeleton-icon" />
                    <div className="flex-fill">
                      <span className="scan-config-skeleton-title" />
                      <span className="scan-config-skeleton-detail" />
                    </div>
                  </div>
                ))}
              </div>
            </section>
          ) : hasPages ? (
            <section className="scan-config-section">
              <div className="scan-config-pages-head">
                <div>
                  <p className="card__title">
                    <ListChecks className="card__icon icon-md" aria-hidden="true" />
                    Pages
                  </p>
                  <p className="text-body-muted scan-config-desc">
                    <span className="text-foreground scan-config-count">
                      {selected.size} of {pages.length} page{pages.length !== 1 ? "s" : ""} selected
                    </span>
                    . This list is what the site is watched on: scheduled scans cover it too.
                  </p>
                </div>
                <div className="scan-config-select-actions">
                  <Button variant="ghost" size="sm" className="text-meta" onClick={selectAll}>
                    Select all
                  </Button>
                  <Button variant="ghost" size="sm" className="text-meta" onClick={selectNone}>
                    None
                  </Button>
                </div>
              </div>

              {pages.length > 8 && (
                <div className="scan-config-search">
                  <Search className="scan-config-search-icon" />
                  <Input
                    value={search}
                    onChange={(e) => setSearch(e.target.value)}
                    placeholder="Filter pages..."
                    className="scan-config-search-input bg-card text-body-muted"
                  />
                </div>
              )}

              <div className="scan-config-scroll-list">
                {filtered.map((page) => {
                  const isSelected = selected.has(page.url);
                  return (
                    <Button
                      unstyled
                      key={page.id}
                      onClick={() => togglePage(page.url)}
                      className={cn(
                        "scan-config-page",
                        isSelected && "scan-config-page--selected",
                      )}>
                      {isSelected ? (
                        <CheckSquare className="icon-md text-brand" />
                      ) : (
                        <Square className="icon-md scan-config-check-off" />
                      )}
                      <div className="flex-fill">
                        <span className="text-body-muted text-truncate scan-config-page-title">
                          {page.title || humanizePath(page.path)}
                        </span>
                        <span className="text-meta text-truncate scan-config-page-path">
                          {page.path || "/"}
                        </span>
                      </div>
                    </Button>
                  );
                })}
                {filtered.length === 0 && search && (
                  <div className="text-body-muted scan-config-empty">No pages match "{search}"</div>
                )}
              </div>
            </section>
          ) : (
            <section className="scan-config-section">
              <div>
                <p className="card__title">
                  <ListChecks className="card__icon icon-md" aria-hidden="true" />
                  Pages
                </p>
                <p className="text-body-muted scan-config-desc">
                  Scan starts with the homepage. Discover a sitemap when you want to add more pages
                  to this run.
                </p>
              </div>
              <div className="scan-config-mini-card">
                <div className="row-start">
                  <CheckSquare className="icon-md text-brand scan-config-home-check" />
                  <div className="min-w-0">
                    <p className="text-body-muted text-strong">Homepage</p>
                    <p className="text-meta text-truncate scan-config-page-path">{siteUrl}</p>
                  </div>
                </div>
              </div>
              <div className="scan-config-card-row">
                <div className="min-w-0">
                  <p className="text-body-muted text-strong">Find more pages</p>
                  <p className="text-meta scan-config-hint">
                    Pull URLs from the sitemap before starting if you want wider coverage.
                  </p>
                </div>
                <Button
                  size="sm"
                  variant="outline"
                  className="scan-config-discover-btn"
                  onClick={handleDiscover}
                  disabled={discovering}>
                  {discovering ? (
                    <>
                      <Loader2 className="icon-sm animate-spin" /> Discovering…
                    </>
                  ) : (
                    <>
                      <Search className="icon-sm" /> Discover Pages
                    </>
                  )}
                </Button>
              </div>
            </section>
          )}
        </div>

        {scopeError && (
          <p className="scan-config-scope-error" role="alert">
            {scopeError}
          </p>
        )}

        <div className="scan-config-footer">
          <div className="field-row-header">
            <span className="scan-config-stat text-meta">
              <Clock3 className="icon-sm text-brand-accent" aria-hidden="true" />
              {estimatedTime}
            </span>
            <span className="scan-config-stat text-meta">
              <ListChecks className="icon-sm text-brand-accent" aria-hidden="true" />
              {`${selectedPageCount} page${selectedPageCount !== 1 ? "s" : ""}`}
            </span>
          </div>
          <div className="row no-shrink">
            <Button variant="ghost" size="sm" className="btn--text-xs" onClick={onCancel}>
              Cancel
            </Button>
            <Button
              size="sm"
              className="scan-run-button btn--text-xs"
              onClick={() => void handleStart()}>
              <span className="scan-play-icon-slot" aria-hidden="true">
                <Play className="scan-play-icon" fill="currentColor" strokeWidth={0} />
              </span>
              {getScanStartLabel({
                canUseCodeScan,
                hasPages,
                scanType,
                selectedCount: selected.size,
              })}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
