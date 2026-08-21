import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";

export function SearchConsoleLoadingState() {
  return (
    <LoadingRegion label="Search loading state" className="page-content">
      <section className="panel panel--flush">
        <div className="search-loading-body">
          <div className="search-loading-head">
            <Skeleton variant="line-lg" width="sm" />
            <Skeleton variant="line-lg" width="md" />
          </div>
          <div className="search-loading-lines">
            <Skeleton variant="stat" width="full" />
            <Skeleton variant="line-lg" width="full" />
            <Skeleton variant="line-lg" width="wide" />
            <Skeleton variant="line" width="md" />
          </div>

          <div className="search-loading-stat-grid">
            {["SEO score", "Open issues", "Search trend"].map((label) => (
              <div key={label} className="tile">
                <div className="tile__rule">
                  <p className="tile__label">{label}</p>
                </div>
                <div className="skeleton-stack">
                  <Skeleton variant="stat" width="sm" />
                  <Skeleton variant="line" width="md" />
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>

      <div className="search-loading-split">
        <section className="panel panel--flush">
          <div className="search-loading-panel-head">
            <Skeleton variant="line-lg" width="sm" />
            <Skeleton variant="line-lg" width="full" />
          </div>
          <div className="search-loading-rows">
            {[0, 1, 2, 3].map((row) => (
              <div key={row} className="search-loading-row">
                <Skeleton variant="line" width="xs" />
                <div className="skeleton-lines">
                  <Skeleton variant="line" width="sm" />
                  <Skeleton variant="line-lg" width="lg" />
                  <Skeleton variant="line" width="full" />
                </div>
                <Skeleton variant="line" width="xs" />
              </div>
            ))}
          </div>
        </section>

        <aside className="panel panel--flush">
          <div className="search-loading-panel-head">
            <Skeleton variant="line-lg" width="md" />
            <Skeleton variant="line-lg" width="md" />
          </div>
          <div className="search-loading-rows">
            {[0, 1, 2].map((row) => (
              <div key={row} className="search-loading-source-row">
                <Skeleton variant="avatar" />
                <div className="skeleton-lines">
                  <Skeleton variant="line" width="sm" />
                  <Skeleton variant="line-lg" width="sm" />
                  <Skeleton variant="line" width="full" />
                </div>
              </div>
            ))}
          </div>
        </aside>
      </div>
    </LoadingRegion>
  );
}
