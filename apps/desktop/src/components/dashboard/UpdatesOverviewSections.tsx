import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";
import { Button } from "@/components/ui/button";

export function UpdatesLoadingState() {
  const filterLabels = ["All", "Security", "Major", "Minor"];

  return (
    <LoadingRegion label="Updates loading state" className="page-content stack-hero">
      <div className="updates-toolbar">
        <Button unstyled type="button" disabled className="disabled-chip-button">
          Re-check
        </Button>
        <Button unstyled type="button" disabled className="disabled-chip-button">
          Run audit
        </Button>
      </div>

      <div className="updates-stat-grid">
        {["Active Vulnerabilities", "Packages Tracked", "Last Audit"].map((label) => (
          <div key={label} className="tile">
            <div className="tile__rule">
              <p className="tile__label">{label}</p>
            </div>
            <Skeleton className="updates-skeleton-stat" />
            <Skeleton className="updates-skeleton-statsub" />
          </div>
        ))}
      </div>

      <div className="row">
        {filterLabels.map((label) => (
          <Button unstyled key={label} type="button" disabled className="disabled-chip-button">
            {label}
          </Button>
        ))}
      </div>

      <div className="stack-card">
        {["Security updates", "Major updates", "Routine updates"].map((section) => (
          <div key={section}>
            <div className="updates-section-head">
              <Skeleton className="updates-skeleton-dot" />
              <p className="section-label-mid">{section}</p>
              <Skeleton className="updates-skeleton-rule" />
            </div>
            <div className="panel panel--flush panel--muted">
              {[0, 1, 2].map((row) => (
                <div
                  key={row}
                  className={`updates-skeleton-row ${row > 0 ? "subtle-divider-top" : ""}`}>
                  <Skeleton className="updates-skeleton-badge" />
                  <div className="flex-fill stack-snug">
                    <Skeleton className="updates-skeleton-line" />
                    <Skeleton className="updates-skeleton-subline" />
                  </div>
                  <div className="row no-shrink">
                    <Skeleton className="updates-skeleton-btn" />
                    <Skeleton className="updates-skeleton-btn updates-skeleton-btn--wide" />
                  </div>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </LoadingRegion>
  );
}

export function UpdatesStatCards({
  securityCount,
  packageCount,
  lastAuditLabel,
  loading,
}: {
  securityCount: number;
  packageCount: number;
  lastAuditLabel: string;
  loading: boolean;
}) {
  return (
    <div className="updates-stat-grid">
      <div className="stat-card">
        <p className="stat-label">Active Vulnerabilities</p>
        {loading ? (
          <span className="inline-skeleton-sm" />
        ) : (
          <span
            className={`stat-value ${securityCount > 0 ? "text-severity-critical" : "text-foreground"}`}>
            {securityCount}
          </span>
        )}
      </div>
      <div className="stat-card">
        <p className="stat-label">Packages Tracked</p>
        {loading ? (
          <span className="inline-skeleton-sm" />
        ) : (
          <span className="stat-value text-foreground">{packageCount}</span>
        )}
      </div>
      <div className="stat-card">
        <p className="stat-label">Last Audit</p>
        {loading ? (
          <span className="inline-skeleton-md" />
        ) : (
          <span className="stat-value text-foreground">{lastAuditLabel}</span>
        )}
      </div>
    </div>
  );
}
